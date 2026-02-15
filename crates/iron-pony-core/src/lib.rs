mod balloon;
mod fortune;
mod pony;

use std::collections::BTreeSet;
use std::path::PathBuf;

use thiserror::Error;
use tracing::{debug, info};

pub use balloon::{BalloonMode, BalloonStyle};
pub use fortune::FortuneConfig;
pub use pony::{PonyAsset, PonyMetadata};

#[derive(Debug, Error)]
pub enum PonyError {
    #[error("no message was provided (message arg, stdin, or --fortune)")]
    NoMessage,
    #[error("pony '{name}' was not found")]
    PonyNotFound { name: String },
    #[error("balloon style '{name}' was not found")]
    BalloonNotFound { name: String },
    #[error("io error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),
    #[error("fortune selection failed: {0}")]
    Fortune(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Say,
    Think,
}

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub message: String,
    pub pony: String,
    pub pony_paths: Vec<PathBuf>,
    pub balloon: Option<String>,
    pub balloon_paths: Vec<PathBuf>,
    pub mode: Mode,
    pub wrap_width: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            message: String::new(),
            pony: "default".to_string(),
            pony_paths: default_pony_paths(),
            balloon: None,
            balloon_paths: default_balloon_paths(),
            mode: Mode::Say,
            wrap_width: 40,
        }
    }
}

pub fn default_pony_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("testdata/ponies"),
        PathBuf::from("/usr/share/ponysay/ponies"),
        PathBuf::from("/usr/local/share/ponysay/ponies"),
    ]
}

pub fn default_balloon_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("testdata/balloons"),
        PathBuf::from("/usr/share/ponysay/balloons"),
        PathBuf::from("/usr/local/share/ponysay/balloons"),
    ]
}

pub fn list_ponies(pony_paths: &[PathBuf]) -> Vec<String> {
    let mut names = BTreeSet::new();
    for path in pony_paths {
        for name in pony::list_pony_names(path) {
            names.insert(name);
        }
    }
    names.into_iter().collect()
}

pub fn list_balloons(balloon_paths: &[PathBuf]) -> Vec<String> {
    let mut names = BTreeSet::new();
    for path in balloon_paths {
        for name in balloon::list_balloon_names(path) {
            names.insert(name);
        }
    }
    names.into_iter().collect()
}

pub fn pick_fortune(config: &FortuneConfig) -> Result<String, PonyError> {
    fortune::pick_fortune(config).map_err(|error| PonyError::Fortune(error.to_string()))
}

pub fn render(config: &RenderConfig) -> Result<String, PonyError> {
    if config.message.trim().is_empty() {
        return Err(PonyError::NoMessage);
    }

    info!(
        pony = %config.pony,
        balloon = config.balloon.as_deref().unwrap_or("<default>"),
        width = config.wrap_width,
        mode = ?config.mode,
        "rendering ponysay output"
    );

    let pony = pony::load_pony(&config.pony, &config.pony_paths)?;
    let mode = match config.mode {
        Mode::Say => BalloonMode::Say,
        Mode::Think => BalloonMode::Think,
    };

    let style = balloon::load_style(config.balloon.as_deref(), &config.balloon_paths, mode)
        .ok_or_else(|| PonyError::BalloonNotFound {
            name: config
                .balloon
                .clone()
                .unwrap_or_else(|| "<default>".to_string()),
        })?;

    debug!(pony_path = %pony.path.display(), "loaded pony template");

    let bubble = balloon::render_balloon(&config.message, config.wrap_width, &style);
    let rendered = pony::insert_balloon(&pony.body, &bubble);
    Ok(format!("\u{1b}[0m{rendered}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_inserts_balloon() {
        let mut config = RenderConfig::default();
        config.message = "hello world".to_string();
        let testdata_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
        config.pony = "simple_say".to_string();
        config.balloon = Some("ascii".to_string());
        config.pony_paths = vec![testdata_root.join("ponies")];
        config.balloon_paths = vec![testdata_root.join("balloons")];

        let out = render(&config).expect("rendered");
        assert!(out.contains("hello world"));
        assert!(out.contains("\\"));
    }
}
