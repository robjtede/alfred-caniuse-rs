use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::{FeatureData, VersionData};

/// The caniuse features
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Db {
    #[serde(default)]
    base_url: String,
    versions: HashMap<String, VersionData>,
    features: HashMap<String, FeatureData>,
}

impl Default for Db {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            versions: HashMap::new(),
            features: HashMap::new(),
        }
    }
}

impl Db {
    /// Fetch the database from the given URL.
    pub fn fetch(url: &str) -> eyre::Result<Db> {
        let mut db = ureq::get(&format!("{}/features.json", url))
            .set("user-agent", "alfred-caniuse-rs/0.1")
            .call()?
            .into_json::<Db>()?;

        db.base_url = url.to_owned();

        Ok(db)
    }

    /// Finds a feature given a query string and returns the feature and stabilization version data.
    pub fn lookup<'a>(&'a self, query: &str) -> Option<(&'a FeatureData, Option<&'a VersionData>)> {
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
