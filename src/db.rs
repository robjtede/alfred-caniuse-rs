use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};

use crate::models::{CompilerVersionData, FeatureData};

const UA_NAME: &str = env!("CARGO_PKG_NAME");
const UA_VERSION: &str = env!("CARGO_PKG_VERSION");
const FETCH_TIMEOUT: Duration = Duration::from_secs(2);

/// The caniuse features
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Db {
    #[serde(default)]
    base_url: String,
    versions: HashMap<String, CompilerVersionData>,
    features: HashMap<String, FeatureData>,
}

impl Db {
    /// Fetch the database from the given URL.
    ///
    /// The request has a two-second deadline, including response body reads.
    pub fn fetch(url: &str) -> eyre::Result<Db> {
        let client = ureq::Agent::config_builder()
            .user_agent(format!("{UA_NAME}/{UA_VERSION}"))
            .timeout_global(Some(FETCH_TIMEOUT))
            .build()
            .new_agent();

        let mut db = client
            .get(&format!("{url}/features.json"))
            .call()?
            .into_body()
            .read_json::<Db>()?;

        db.base_url = url.to_owned();

        // fill in slugs with map key
        for (slug, feature) in &mut db.features {
            feature.slug = slug.clone()
        }

        Ok(db)
    }

    /// Returns an iterator of the most recent Rust versions in reverse chronological order.
    pub fn versions_preview(&self) -> impl Iterator<Item = CompilerVersionData> {
        let mut versions = self.versions.values().cloned().collect::<Vec<_>>();
        versions.sort_by(|a, b| a.partial_cmp(b).unwrap().reverse());
        versions.into_iter().take(10)
    }

    /// Finds a feature given it's slug and returns the feature and stabilization version data.
    pub fn get_feature<'a>(
        &'a self,
        name: &str,
    ) -> Option<(&'a FeatureData, Option<&'a CompilerVersionData>)> {
        let feature = self.features.get(name)?;

        match feature.version_number.as_deref() {
            Some(v) => {
                let version = self.versions.get(v);
                Some((feature, version))
            }
            None => Some((feature, None)),
        }
    }

    /// Finds features in relevance order.
    ///
    /// Exact slug, flag, or title matches come first, then substring matches, then fuzzy
    /// matches. Features with the same match rank are sorted by slug.
    pub fn lookup<'a>(&'a self, query: &str) -> Vec<&'a FeatureData> {
        let mut feats = vec![];

        'features: for feature in self.features.values() {
            if feature.slug.to_lowercase().contains(query) {
                feats.push(feature);
                continue;
            }

            if feature
                .flag
                .as_deref()
                .map(|flag| flag.to_lowercase().contains(query))
                .unwrap_or(false)
            {
                feats.push(feature);
                continue;
            }

            if feature.title.to_lowercase().contains(query) {
                feats.push(feature);
                continue;
            }

            for item in &feature.items {
                if item.to_lowercase().contains(query) {
                    feats.push(feature);
                    continue 'features;
                }
            }

            for item in &feature.aliases {
                if item.to_lowercase().contains(query) {
                    feats.push(feature);
                    continue 'features;
                }
            }

            if strsim::sorensen_dice(query, &feature.slug.to_lowercase()) > 0.65 {
                feats.push(feature);
                continue;
            }

            if let Some(flag) = feature.flag.as_deref() {
                if strsim::sorensen_dice(query, &flag.to_lowercase()) > 0.65 {
                    feats.push(feature);
                    continue;
                }
            }

            if strsim::sorensen_dice(query, &feature.title.to_lowercase()) > 0.4 {
                feats.push(feature);
                continue;
            }
        }

        feats.sort_by_cached_key(|feature| {
            let slug = feature.slug.to_lowercase();
            let flag = feature.flag.as_deref().map(str::to_lowercase);
            let title = feature.title.to_lowercase();

            let rank = if slug == query || flag.as_deref() == Some(query) || title == query {
                0
            } else if slug.contains(query)
                || flag.as_deref().is_some_and(|flag| flag.contains(query))
                || title.contains(query)
                || feature
                    .items
                    .iter()
                    .any(|item| item.to_lowercase().contains(query))
                || feature
                    .aliases
                    .iter()
                    .any(|alias| alias.to_lowercase().contains(query))
            {
                1
            } else {
                2
            };

            (rank, feature.slug.as_str())
        });

        feats
    }
}
