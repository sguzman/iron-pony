use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tracing::{debug, trace};
use walkdir::WalkDir;

use crate::PonyError;

#[derive(Debug, Clone, Default)]
pub struct PonyMetadata {
    pub tags: BTreeMap<String, Vec<String>>,
    pub comments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PonyAsset {
    pub path: PathBuf,
    pub metadata: PonyMetadata,
    pub body: String,
}

pub fn load_pony(name: &str, roots: &[PathBuf]) -> Result<PonyAsset, PonyError> {
    for root in roots {
        for candidate in pony_candidates(root, name) {
            if candidate.is_file() {
                let raw = std::fs::read_to_string(&candidate).map_err(|source| PonyError::Io {
                    path: candidate.clone(),
                    source,
                })?;

                let (metadata, body) = parse_metadata_header(&raw);
                debug!(path = %candidate.display(), tags = metadata.tags.len(), "loaded pony asset");

                return Ok(PonyAsset {
                    path: candidate,
                    metadata,
                    body,
                });
            }
        }
    }

    Err(PonyError::PonyNotFound {
        name: name.to_string(),
    })
}

pub fn list_pony_names(root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if !root.exists() {
        return names;
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }
    }

    names.sort();
    names.dedup();
    names
}

pub fn insert_balloon(template: &str, balloon_lines: &[String]) -> String {
    if balloon_lines.is_empty() {
        return template.to_string();
    }

    let mut out = Vec::new();
    let mut inserted = false;

    for line in template.lines() {
        if let Some((prefix, suffix)) = line.split_once("$balloon$") {
            trace!("expanding $balloon$ anchor");
            for (index, balloon_line) in balloon_lines.iter().enumerate() {
                if index == 0 {
                    out.push(format!("{prefix}{balloon_line}{suffix}"));
                } else {
                    out.push(format!(
                        "{}{balloon_line}",
                        " ".repeat(prefix.chars().count())
                    ));
                }
            }
            inserted = true;
        } else {
            out.push(line.to_string());
        }
    }

    if !inserted {
        let mut merged = balloon_lines.to_vec();
        merged.extend(out);
        return merged.join("\n");
    }

    out.join("\n")
}

fn pony_candidates(root: &Path, name: &str) -> [PathBuf; 2] {
    [root.join(name), root.join(format!("{name}.pony"))]
}

fn parse_metadata_header(raw: &str) -> (PonyMetadata, String) {
    let text = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut metadata = PonyMetadata::default();

    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return (metadata, String::new());
    };

    if first.trim_end() != "$$$" {
        return (metadata, text.to_string());
    }

    let mut body_start = 0;
    let mut consumed = first.len() + 1;

    for line in lines {
        if line.trim_end() == "$$$" {
            body_start = consumed + line.len() + 1;
            break;
        }

        if let Some((tag, value)) = line.split_once(':') {
            let tag = tag.trim();
            if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
                metadata
                    .tags
                    .entry(tag.to_string())
                    .or_default()
                    .push(value.trim().to_string());
            } else {
                metadata.comments.push(line.to_string());
            }
        } else {
            metadata.comments.push(line.to_string());
        }

        consumed += line.len() + 1;
    }

    let body = if body_start > 0 && body_start <= text.len() {
        text[body_start..].to_string()
    } else {
        // Unterminated metadata header: keep file as-is for resilience while logging.
        debug!("unterminated pony metadata header; preserving entire body");
        text.to_string()
    };

    (metadata, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_balloon_anchor() {
        let template = "  $balloon$\n   \\\n    (oo)";
        let out = insert_balloon(template, &["< hi >".to_string(), "\\----/".to_string()]);
        assert!(out.contains("< hi >"));
        assert!(out.contains("\\----/"));
    }

    #[test]
    fn parses_metadata_header() {
        let raw = "$$$\nNAME: Twilight\ncomment\n$$$\npony";
        let (meta, body) = parse_metadata_header(raw);
        assert_eq!(meta.tags["NAME"], vec!["Twilight"]);
        assert_eq!(meta.comments, vec!["comment"]);
        assert_eq!(body.trim(), "pony");
    }
}
