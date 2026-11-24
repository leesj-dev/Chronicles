use std::{
    env,
    path::{Path, PathBuf},
};

fn checkout(path: &Path) -> Option<&Path> {
    path.ancestors().find(|dir| {
        dir.join("engine/Cargo.toml").is_file() && dir.join("src/web/index.html").is_file()
    })
}

pub fn root() -> std::io::Result<PathBuf> {
    if let Some(root) = env::current_dir().ok().as_deref().and_then(checkout) {
        return Ok(root.to_owned());
    }
    if let Some(root) = env::current_exe().ok().as_deref().and_then(checkout) {
        return Ok(root.to_owned());
    }
    dirs::data_local_dir()
        .map(|dir| dir.join("Chronicles"))
        .ok_or_else(|| std::io::Error::other("could not locate a Chronicles data directory"))
}

pub fn import_path(root: &Path) -> PathBuf {
    root.join(".local/imports/macmini.json")
}

pub fn configure_engine(root: &Path) -> std::io::Result<()> {
    let config = env::var_os("TOKSCALE_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map_or_else(|| root.join(".local/engine"), PathBuf::from);
    std::fs::create_dir_all(&config)?;
    // Called once from main, before starting Tokio or running tokscale.
    env::set_var("TOKSCALE_CONFIG_DIR", config);
    Ok(())
}
