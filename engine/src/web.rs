use crate::{collector::Collector, usage};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use http_body_util::Full;
use hyper::{
    body::Bytes, header, server::conn::http1, service::service_fn, Method, Request, Response,
    StatusCode,
};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use std::{
    borrow::Cow,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{net::TcpListener, sync::Mutex};

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'";

macro_rules! asset {
    ($file:literal, $mime:literal) => {
        Some(($mime, include_bytes!(concat!("../../src/web/", $file))))
    };
}

fn embedded(file: &str) -> Option<(&'static str, &'static [u8])> {
    match file {
        "index.html" => asset!("index.html", "text/html"),
        "app.js" => asset!("app.js", "text/javascript"),
        "combobox.js" => asset!("combobox.js", "text/javascript"),
        "calendar.js" => asset!("calendar.js", "text/javascript"),
        "analytics.mjs" => asset!("analytics.mjs", "text/javascript"),
        "style.css" => asset!("style.css", "text/css"),
        "shadcn.css" => asset!("shadcn.css", "text/css"),
        "favicon.svg" => asset!("favicon.svg", "image/svg+xml"),
        "Geist.woff2" => asset!("Geist.woff2", "font/woff2"),
        _ => None,
    }
}

fn embedded_text(file: &str) -> &'static str {
    std::str::from_utf8(embedded(file).expect("known web asset").1).expect("UTF-8 web asset")
}

pub fn demo_report() -> Value {
    let mut report: Value =
        serde_json::from_str(include_str!("../../examples/demo.json")).expect("valid demo");
    let fields = [
        "input",
        "output",
        "cacheRead",
        "cacheWrite",
        "requests",
        "costRequests",
    ];
    let mut hourly = Vec::new();
    for row in report["rows"].as_array().expect("demo rows") {
        let requests = row["requests"].as_u64().expect("demo requests");
        let count = requests.min(4);
        let mut spent = 0.0;
        for (index, hour) in [8, 11, 15, 20].into_iter().take(count as usize).enumerate() {
            let mut bucket = row.clone();
            for field in fields {
                if let Some(value) = row[field].as_u64() {
                    bucket[field] =
                        json!(value / count + u64::from((index as u64) < value % count));
                }
            }
            bucket["total"] = json!(fields[..4]
                .iter()
                .map(|field| bucket[field].as_u64().unwrap())
                .sum::<u64>());
            if let Some(cost) = row["cost"].as_f64() {
                let portion = if index as u64 == count - 1 {
                    cost - spent
                } else {
                    cost * bucket["requests"].as_f64().unwrap() / requests as f64
                };
                let portion = usage::round_cost(portion);
                spent += portion;
                bucket["cost"] = json!(portion);
            }
            let date = row["date"].as_str().unwrap();
            bucket["hour"] = json!(format!("{date}T{hour:02}:00:00+00:00"));
            hourly.push(bucket);
        }
    }
    report["hourlyRows"] = json!(hourly);
    report
}

struct Site {
    collector: Option<Mutex<Collector>>,
    demo: bool,
    web_dir: Option<PathBuf>,
}

pub async fn serve(
    port: u16,
    demo: bool,
    web_dir: Option<PathBuf>,
    collector: Option<Collector>,
) -> io::Result<()> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("Chronicles: http://{}", listener.local_addr()?);
    let site = Arc::new(Site {
        collector: collector.map(Mutex::new),
        demo,
        web_dir,
    });
    loop {
        let (stream, _) = listener.accept().await?;
        let site = site.clone();
        tokio::spawn(async move {
            let handler = service_fn(move |request| {
                let site = site.clone();
                async move { site.respond(request).await }
            });
            let _ = http1::Builder::new()
                .keep_alive(false)
                .serve_connection(TokioIo::new(stream), handler)
                .await;
        });
    }
}

impl Site {
    async fn respond<B>(&self, request: Request<B>) -> io::Result<Response<Full<Bytes>>> {
        if request.method() != Method::GET {
            let method = request.method().as_str();
            return Ok(error(
                StatusCode::NOT_IMPLEMENTED,
                &format!("Unsupported method ('{method}')"),
                "Server does not support this operation",
            ));
        }
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(':').next());
        if !matches!(host, Some("localhost" | "127.0.0.1")) {
            return Ok(error(
                StatusCode::FORBIDDEN,
                "Forbidden",
                "Request forbidden -- authorization will not help",
            ));
        }
        let path = request.uri().path();
        let path = if path.starts_with("//") {
            Cow::Owned(format!("/{}", path.trim_start_matches('/')))
        } else {
            Cow::Borrowed(path)
        };
        let path = if request.uri().scheme().is_some() {
            ""
        } else {
            path.as_ref()
        };
        let (content, mime) = match path {
            "/api/demo" | "/api/usage" | "/api/local" => {
                let report = if path == "/api/demo" || (self.demo && path != "/api/local") {
                    demo_report()
                } else if let Some(collector) = &self.collector {
                    collector.lock().await.collect().await
                } else {
                    return Ok(error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Usage scan failed",
                        "Server got itself in trouble",
                    ));
                };
                (
                    Bytes::from(serde_json::to_vec(&report)?),
                    "application/json",
                )
            }
            _ => {
                let file = if path == "/" {
                    "index.html"
                } else {
                    path.strip_prefix('/').unwrap_or("")
                };
                let Some((mime, bytes)) = embedded(file) else {
                    return Ok(error(
                        StatusCode::NOT_FOUND,
                        "Not Found",
                        "Nothing matches the given URI",
                    ));
                };
                let content = if let Some(dir) = &self.web_dir {
                    Bytes::from(std::fs::read(dir.join(file))?)
                } else {
                    Bytes::from_static(bytes)
                };
                (content, mime)
            }
        };
        Ok(Response::builder()
            .header(header::CONTENT_TYPE, format!("{mime}; charset=utf-8"))
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(header::CONTENT_SECURITY_POLICY, CSP)
            .header(header::CONTENT_LENGTH, content.len())
            .body(Full::new(content))
            .expect("valid response"))
    }
}

fn error(status: StatusCode, message: &str, explanation: &str) -> Response<Full<Bytes>> {
    let code = status.as_u16();
    let page = format!("<!DOCTYPE HTML>\n<html lang=\"en\">\n    <head>\n        <meta charset=\"utf-8\">\n        <title>Error response</title>\n    </head>\n    <body>\n        <h1>Error response</h1>\n        <p>Error code: {code}</p>\n        <p>Message: {message}.</p>\n        <p>Error code explanation: {code} - {explanation}.</p>\n    </body>\n</html>\n");
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html;charset=utf-8")
        .header(header::CONTENT_LENGTH, page.len())
        .body(Full::new(Bytes::from(page)))
        .expect("valid error response")
}

fn inline_styles(page: String, file: &str, styles: &str) -> String {
    page.replace(
        &format!("<link rel=\"stylesheet\" href=\"/{file}\" />"),
        &format!("<style>{styles}</style>"),
    )
}

fn without_import(module: &str) -> &str {
    if let Some(import) = module.strip_prefix("import ") {
        if let Some((_, body)) = import.split_once(";\n") {
            return body;
        }
    }
    module
}

pub fn export(path: &Path, report: &Value) -> io::Result<()> {
    let font = STANDARD.encode(embedded("Geist.woff2").unwrap().1);
    let styles = embedded_text("style.css")
        .replace("/Geist.woff2", &format!("data:font/woff2;base64,{font}"));
    let mut page = inline_styles(
        embedded_text("index.html").to_owned(),
        "shadcn.css",
        embedded_text("shadcn.css"),
    );
    page = inline_styles(page, "style.css", &styles);
    for file in ["combobox.js", "calendar.js"] {
        page = page.replace(
            &format!("<script src=\"/{file}\"></script>"),
            &format!("<script>{}</script>", embedded_text(file)),
        );
    }
    let analytics = without_import(embedded_text("analytics.mjs")).replace("export ", "");
    let app = without_import(embedded_text("app.js"));
    let json = report.to_string().replace('<', "\\u003c");
    page = page.replace("<script type=\"module\" src=\"/app.js\"></script>",
        &format!("<script>window.CHRONICLES_DATA={json};</script><script type=\"module\">{analytics}\n{app}</script>"));
    let favicon = url_quote(embedded_text("favicon.svg"));
    page = page
        .replace("href=\"/\"", "href=\"#\"")
        .replace("/favicon.svg", &format!("data:image/svg+xml,{favicon}"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, page)
}

fn url_quote(text: &str) -> Cow<'_, str> {
    const ESCAPE: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~')
        .remove(b'/');
    percent_encoding::utf8_percent_encode(text, ESCAPE).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_assets_are_read_from_disk_on_each_request() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("index.html");
        let site = Site {
            collector: None,
            demo: true,
            web_dir: Some(dir.path().into()),
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for version in ["first", "revised"] {
                std::fs::write(&file, version).unwrap();
                let request = Request::get("/")
                    .header(header::HOST, "localhost")
                    .body(())
                    .unwrap();
                let response = site.respond(request).await.unwrap();
                let body = http_body_util::BodyExt::collect(response.into_body())
                    .await
                    .unwrap()
                    .to_bytes();
                assert_eq!(body, version);
            }
            std::fs::write(dir.path().join("private.json"), "{}").unwrap();
            let request = Request::get("/private.json")
                .header(header::HOST, "localhost")
                .body(())
                .unwrap();
            assert_eq!(site.respond(request).await.unwrap().status(), 404);
        });
    }

    #[test]
    fn routes_check_host_and_serve_only_allowlisted_assets() {
        let site = Site {
            collector: None,
            demo: true,
            web_dir: None,
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let request = |path, host| {
            Request::get(path)
                .header(header::HOST, host)
                .body(())
                .unwrap()
        };
        runtime.block_on(async {
            assert_eq!(
                site.respond(request("/", "invalid"))
                    .await
                    .unwrap()
                    .status(),
                403
            );
            assert_eq!(
                site.respond(request("/pricing.mjs", "localhost"))
                    .await
                    .unwrap()
                    .status(),
                404
            );
            assert_eq!(
                site.respond(request("/api/local", "localhost"))
                    .await
                    .unwrap()
                    .status(),
                500
            );
            for path in [
                "/",
                "/index.html",
                "/app.js",
                "/combobox.js",
                "/calendar.js",
                "/analytics.mjs",
                "/style.css",
                "/shadcn.css",
                "/favicon.svg",
                "/Geist.woff2",
                "/api/demo",
                "/api/usage?refresh=1",
            ] {
                let response = site.respond(request(path, "127.0.0.1:8768")).await.unwrap();
                assert_eq!(response.status(), 200);
                assert_eq!(response.headers()[header::CONTENT_SECURITY_POLICY], CSP);
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                assert_eq!(
                    response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                    "nosniff"
                );
                assert!(response.headers()[header::CONTENT_TYPE]
                    .to_str()
                    .unwrap()
                    .ends_with("; charset=utf-8"));
            }
        });
    }
}
