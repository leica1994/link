use crate::error::{Result, StemError};
use std::{env, path::PathBuf};

const CACHE_DIR_ENV: &str = "STEM_SPLITTER_CORE_CACHE_DIR";

pub fn models_cache_dir() -> Result<PathBuf> {
    let mut p = cache_dir()?;
    p.push("models");
    Ok(p)
}

pub fn ep_cache_file() -> Result<PathBuf> {
    let mut p = cache_dir()?;
    p.push("ep_health_v1.json");
    Ok(p)
}

pub fn ep_probe_cache_file() -> Result<PathBuf> {
    let mut p = cache_dir()?;
    p.push("ep_probe_success_v1.json");
    Ok(p)
}

pub fn coreml_cache_dir() -> Result<PathBuf> {
    let mut p = cache_dir()?;
    p.push("coreml");
    Ok(p)
}

fn cache_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os(CACHE_DIR_ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    Err(StemError::CacheDirUnavailable)
}
