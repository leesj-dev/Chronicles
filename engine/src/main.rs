use chronicles::{collector::Collector, copilot, paths, report, sync, usage, web};
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand};
use std::{
    error::Error,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

#[derive(Parser)]
#[command(version, about, args_conflicts_with_subcommands = true)]
struct Cli {
    #[arg(long)]
    list_clients: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the local dashboard
    Serve {
        #[arg(long, default_value_t = 8768)]
        port: u16,
        #[arg(long)]
        demo: bool,
        #[arg(long)]
        offline: bool,
        /// Load the allowlisted web assets from this directory
        #[arg(long)]
        web_dir: Option<PathBuf>,
    },
    /// Write a self-contained HTML snapshot
    Export {
        path: PathBuf,
        #[arg(long)]
        demo: bool,
        #[arg(long)]
        offline: bool,
    },
    /// Import usage from another Mac over SSH
    Sync {
        #[arg(long, default_value = "macmini")]
        host: String,
        #[arg(long)]
        offline: bool,
    },
    /// Print the raw tokscale-derived report as JSON
    Report(report::Options),
}

fn main() -> ExitCode {
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    if let Err(error) = run(cli, matches.subcommand_matches("report")) {
        eprintln!("chronicles: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn print_json(value: impl serde::Serialize) -> io::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, &value)?;
    writeln!(out)
}

fn run(cli: Cli, report_args: Option<&ArgMatches>) -> Result<(), Box<dyn Error>> {
    let command = match cli.command {
        _ if cli.list_clients => return Ok(print_json(report::clients())?),
        Some(Command::Report(options)) if options.list_clients => {
            return Ok(print_json(report::clients())?)
        }
        Some(command) => command,
        None => {
            return Ok(Cli::command().print_help()?);
        }
    };
    if let Command::Export {
        path, demo: true, ..
    } = &command
    {
        web::export(path, &web::demo_report())?;
        println!("Exported snapshot: {}", path.display());
        return Ok(());
    }

    let root = paths::root()?;
    let config = if matches!(command, Command::Sync { .. }) {
        Ok(())
    } else {
        paths::configure_engine(&root)
    };
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async move {
        match command {
            Command::Serve {
                port,
                demo,
                offline,
                web_dir,
            } => {
                let collector = Collector::new(&root, offline, usage::timezone()?, config)?;
                web::serve(port, demo, web_dir, Some(collector)).await?;
            }
            Command::Export { path, offline, .. } => {
                let report = Collector::new(&root, offline, usage::timezone()?, config)?
                    .collect()
                    .await;
                web::export(&path, &report)?;
                println!("Exported snapshot: {}", path.display());
            }
            Command::Sync { host, offline } => {
                sync::sync(
                    &root,
                    &std::env::current_exe()?,
                    &host,
                    offline,
                    usage::timezone()?,
                )
                .await
                .map_err(|error| format!("sync: {error}"))?;
            }
            Command::Report(options) => {
                config?;
                // Read the private fields of the shared report options from Clap.
                let args = report_args.expect("report subcommand arguments");
                let home = args.get_one::<String>("home").cloned();
                let exclude = args.get_one::<String>("exclude").map(String::as_str);
                print_json(copilot::scan(&options, home, exclude).await?)?;
            }
        }
        Ok(())
    });
    runtime.shutdown_background();
    result
}
