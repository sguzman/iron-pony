mod balloon;
mod fortune;
mod pony;

use std::collections::BTreeSet;
use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use thiserror::Error;
use tracing::{debug, info, trace};

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
            pony: String::new(),
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
        PathBuf::from("/usr/share/ponysay/ponies"),
        PathBuf::from("/usr/share/ponysay/extraponies"),
        PathBuf::from("/usr/share/ponysay/ttyponies"),
        PathBuf::from("/usr/local/share/ponysay/ponies"),
        PathBuf::from("/usr/local/share/ponysay/extraponies"),
        PathBuf::from("/usr/local/share/ponysay/ttyponies"),
    ]
}

pub fn default_balloon_paths() -> Vec<PathBuf> {
    vec![
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

pub fn select_pony(
    requested: Option<&str>,
    pony_paths: &[PathBuf],
    seed: Option<u64>,
) -> Result<String, PonyError> {
    if let Some(name) = requested {
        return Ok(name.to_string());
    }

    if let Some(best_path) = find_best_pony(pony_paths) {
        info!(path = %best_path.display(), "auto-selected best.pony");
        return Ok(best_path.to_string_lossy().to_string());
    }

    let names = list_ponies(pony_paths);
    if names.is_empty() {
        return Err(PonyError::PonyNotFound {
            name: "<auto>".to_string(),
        });
    }

    let mut rng = seeded_rng(seed);
    let index = rng.random_range(0..names.len());
    let selected = names[index].clone();
    info!(
        pony = %selected,
        choices = names.len(),
        "auto-selected random installed pony"
    );
    Ok(selected)
}

pub fn pick_fortune(config: &FortuneConfig) -> Result<String, PonyError> {
    fortune::pick_fortune(config).map_err(|error| PonyError::Fortune(error.to_string()))
}

pub fn render(config: &RenderConfig) -> Result<String, PonyError> {
    if config.message.trim().is_empty() {
        return Err(PonyError::NoMessage);
    }

    let requested_pony = if config.pony.trim().is_empty() {
        None
    } else {
        Some(config.pony.as_str())
    };
    let pony_name = select_pony(requested_pony, &config.pony_paths, None)?;

    info!(
        pony = %pony_name,
        balloon = config.balloon.as_deref().unwrap_or("<default>"),
        width = config.wrap_width,
        mode = ?config.mode,
        "rendering ponysay output"
    );

    let pony = pony::load_pony(&pony_name, &config.pony_paths)?;
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
    let rendered = pony::insert_balloon(&pony.body, &bubble, &style);
    Ok(format!("\u{1b}[0m{rendered}"))
}

fn find_best_pony(pony_paths: &[PathBuf]) -> Option<PathBuf> {
    for root in pony_paths {
        let candidate = root.join("best.pony");
        if !candidate.is_file() {
            trace!(path = %candidate.display(), "best.pony candidate not present");
            continue;
        }

        let resolved = std::fs::canonicalize(&candidate).unwrap_or(candidate);
        return Some(resolved);
    }
    None
}

fn seeded_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(value) => StdRng::seed_from_u64(value),
        None => {
            let mut entropy = rand::rng();
            StdRng::from_rng(&mut entropy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn render_inserts_balloon() {
        let mut config = RenderConfig::default();
        config.message = "hello world".to_string();
        let tmp = tempfile::tempdir().expect("tempdir");
        let pony_dir = tmp.path().join("ponies");
        fs::create_dir_all(&pony_dir).expect("pony dir");
        fs::write(
            pony_dir.join("default.pony"),
            "$$$\nNAME: Test Pony\n$$$\n$balloon$\n  \\\n   pony\n",
        )
        .expect("write pony");

        config.pony = "default".to_string();
        config.balloon = None;
        config.pony_paths = vec![pony_dir];
        config.balloon_paths = vec![];

        let out = render(&config).expect("rendered");
        assert!(out.contains("hello world"));
        assert!(out.contains("\\"));
    }

    #[test]
    fn select_pony_prefers_best_pony() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pony_dir = tmp.path().join("ponies");
        fs::create_dir_all(&pony_dir).expect("pony dir");
        fs::write(pony_dir.join("best.pony"), "$$$\n$$$\nbest\n").expect("write best");
        fs::write(pony_dir.join("other.pony"), "$$$\n$$$\nother\n").expect("write other");

        let selected = select_pony(None, &[pony_dir], Some(7)).expect("selected pony");
        assert!(selected.ends_with("best.pony"));
    }

    #[test]
    fn select_pony_random_is_seeded() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pony_dir = tmp.path().join("ponies");
        fs::create_dir_all(&pony_dir).expect("pony dir");
        fs::write(pony_dir.join("alpha.pony"), "$$$\n$$$\na\n").expect("write alpha");
        fs::write(pony_dir.join("beta.pony"), "$$$\n$$$\nb\n").expect("write beta");

        let first = select_pony(None, &[pony_dir.clone()], Some(42)).expect("first");
        let second = select_pony(None, &[pony_dir], Some(42)).expect("second");
        assert_eq!(first, second);
        assert!(first == "alpha" || first == "beta");
    }
}
