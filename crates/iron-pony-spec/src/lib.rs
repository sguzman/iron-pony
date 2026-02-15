use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, info};

#[derive(Debug, Clone, Deserialize)]
pub struct RequirementSpec {
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub feature_map: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub description: String,
    pub weight: f64,
}

impl RequirementSpec {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        info!(path = %path.display(), "loading requirement specification");
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read spec file {}", path.display()))?;
        let spec = serde_yaml::from_str::<Self>(&raw)
            .with_context(|| format!("failed to parse spec file {}", path.display()))?;
        debug!(
            requirements = spec.requirements.len(),
            feature_map = spec.feature_map.len(),
            "loaded requirement specification"
        );
        Ok(spec)
    }

    pub fn requirement_weights(&self) -> BTreeMap<String, f64> {
        self.requirements
            .iter()
            .map(|req| (req.id.clone(), req.weight))
            .collect()
    }

    pub fn mapped_requirements<'a>(&'a self, features: &'a [String]) -> BTreeSet<&'a str> {
        let mut out = BTreeSet::new();
        for feature in features {
            if let Some(ids) = self.feature_map.get(feature) {
                for id in ids {
                    out.insert(id.as_str());
                }
            } else {
                out.insert(feature.as_str());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_features_to_requirements() {
        let spec = RequirementSpec {
            requirements: vec![Requirement {
                id: "cli.flag.a".to_string(),
                description: "-a flag support".to_string(),
                weight: 1.0,
            }],
            feature_map: BTreeMap::from([(
                "include_offensive".to_string(),
                vec!["cli.flag.a".to_string()],
            )]),
        };

        let features = vec!["include_offensive".to_string()];
        let mapped = spec.mapped_requirements(&features);
        assert!(mapped.contains("cli.flag.a"));
    }
}
