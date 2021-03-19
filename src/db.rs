use std::collections::HashMap;

use serde::Deserialize;

use crate::models::{FeatureData, VersionData};

#[derive(Debug, Clone, Deserialize)]
pub struct Db {
    versions: HashMap<String, VersionData>,
    features: HashMap<String, FeatureData>,
}

impl Default for Db {
    fn default() -> Self {
        Self {
            versions: HashMap::new(),
            features: HashMap::new(),
        }
    }
}

impl Db {
    pub fn fetch(url: &str) -> eyre::Result<Db> {
        ureq::get(url)
            .set("User-Agent", "alfred-caniuse/0.1")
            .call()?
            .into_json()
            .map_err(Into::into)
    }

    /// Finds feature and if  version.
    pub fn lookup<'a>(
        &'a self,
        query: &str,
    ) -> Option<(&'a FeatureData, Option<&'a VersionData>)> {
        let feature = self.features.get(query)?;

        match feature.version_number.as_deref() {
            Some(v) => {
                let version = self.versions.get(v);
                Some((feature, version))
            }
            None => Some((feature, None)),
        }
    }
}
