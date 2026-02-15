use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use tracing::{debug, info, trace};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FortuneConfig {
    pub include_offensive: bool,
    pub equal_files: bool,
    pub seed: Option<u64>,
    pub sources: Vec<PathBuf>,
    pub search_paths: Vec<PathBuf>,
}

impl Default for FortuneConfig {
    fn default() -> Self {
        Self {
            include_offensive: false,
            equal_files: false,
            seed: None,
            sources: Vec::new(),
            search_paths: vec![
                PathBuf::from("testdata/fortunes"),
                PathBuf::from("/usr/share/games/fortunes"),
                PathBuf::from("/usr/share/fortune"),
            ],
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FortuneError {
    #[error("no fortune sources found")]
    NoSources,
    #[error("no fortunes available in sources")]
    NoFortunes,
    #[error("io error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
struct Db {
    path: PathBuf,
    fortunes: Vec<String>,
}

pub fn pick_fortune(config: &FortuneConfig) -> Result<String, FortuneError> {
    info!(
        equal_files = config.equal_files,
        include_offensive = config.include_offensive,
        "selecting internal fortune"
    );

    let sources = resolve_sources(config)?;
    let mut dbs = Vec::new();
    for source in sources {
        let db = load_db(&source)?;
        if !db.fortunes.is_empty() {
            dbs.push(db);
        }
    }

    if dbs.is_empty() {
        return Err(FortuneError::NoFortunes);
    }

    let mut rng = seeded_rng(config.seed);
    let db_index = if config.equal_files {
        rng.random_range(0..dbs.len())
    } else {
        let total: usize = dbs.iter().map(|db| db.fortunes.len()).sum();
        let mut target = rng.random_range(0..total);
        let mut index = dbs.len() - 1;
        for (pos, db) in dbs.iter().enumerate() {
            if target < db.fortunes.len() {
                index = pos;
                break;
            }
            target -= db.fortunes.len();
        }
        index
    };

    let fortune_index = rng.random_range(0..dbs[db_index].fortunes.len());
    debug!(source = %dbs[db_index].path.display(), fortune_index, "selected internal fortune");
    Ok(dbs[db_index].fortunes[fortune_index].clone())
}

fn resolve_sources(config: &FortuneConfig) -> Result<Vec<PathBuf>, FortuneError> {
    let mut found = BTreeSet::new();

    if config.sources.is_empty() {
        for root in &config.search_paths {
            collect(root, config.include_offensive, &mut found);
        }
    } else {
        for source in &config.sources {
            if source.exists() {
                collect(source, config.include_offensive, &mut found);
                continue;
            }

            for root in &config.search_paths {
                let candidate = root.join(source);
                if candidate.exists() {
                    collect(&candidate, config.include_offensive, &mut found);
                }
            }
        }
    }

    if found.is_empty() {
        Err(FortuneError::NoSources)
    } else {
        Ok(found.into_iter().collect())
    }
}

fn collect(path: &Path, include_offensive: bool, out: &mut BTreeSet<PathBuf>) {
    if path.is_file() {
        if is_candidate(path, include_offensive) {
            out.insert(path.to_path_buf());
        }
        return;
    }

    if !path.is_dir() {
        return;
    }

    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() && is_candidate(entry.path(), include_offensive) {
            out.insert(entry.path().to_path_buf());
        }
    }
}

fn is_candidate(path: &Path, include_offensive: bool) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if name.starts_with('.') || name.ends_with(".dat") {
        return false;
    }
    if !include_offensive && name.ends_with("-o") {
        return false;
    }
    true
}

fn load_db(path: &Path) -> Result<Db, FortuneError> {
    let raw = std::fs::read(path).map_err(|source| FortuneError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let text = String::from_utf8_lossy(&raw);
    let fortunes = split_fortunes(&text);
    trace!(path = %path.display(), fortunes = fortunes.len(), "loaded fortune database");

    Ok(Db {
        path: path.to_path_buf(),
        fortunes,
    })
}

fn split_fortunes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line == "%" {
            push_segment(&mut out, &current);
            current.clear();
            continue;
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    push_segment(&mut out, &current);
    out
}

fn push_segment(out: &mut Vec<String>, segment: &str) {
    let trimmed = segment.trim_matches('\n');
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
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

    #[test]
    fn parses_percent_delimited_fortunes() {
        let parsed = split_fortunes("one\n%\ntwo\n%\nthree\n");
        assert_eq!(parsed, vec!["one", "two", "three"]);
    }
}
