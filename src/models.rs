//! caniuse.rs object structures.
//!
//! Definitions derived from https://github.com/jplatte/caniuse.rs/blob/e9c940047437cccfaf8ff65bcf68f70538877662/build.rs.

use serde::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl Default for Channel {
    fn default() -> Self {
        // Not specifying the channel in features.toml is equivalent to specifying "stable".
        Self::Stable
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VersionData {
    /// Rust version number, e.g. "1.0.0"
    pub number: String,

    /// The channel (stable / beta / nightly)
    #[serde(default)]
    pub channel: Channel,

    /// Release date, in format "yyyy-mm-dd"
    pub release_date: Option<String>,

    /// Release notes (https://github.com/rust-lang/rust/blob/master/RELEASES.md#{anchor})
    pub release_notes: Option<String>,

    /// Blog post path (https://blog.rust-lang.org/{path})
    pub blog_post_path: Option<String>,

    /// GitHub milestone id (https://github.com/rust-lang/rust/milestone/{id})
    pub gh_milestone_id: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeatureData {
    /// Short description to identify the feature
    pub title: String,

    /// Feature flag name, for things that were previously or are still Rust
    /// nightly features with such a thing (`#![feature(...)]`)
    pub flag: Option<String>,

    /// RFC ID (https://github.com/rust-lang/rfcs/pull/{id})
    pub rfc_id: Option<u64>,

    /// Implementation PR id (https://github.com/rust-lang/rust/pull/{id})
    ///
    /// Only for small features that were implemented in one PR.
    pub impl_pr_id: Option<u64>,

    /// Tracking issue id (https://github.com/rust-lang/rust/issues/{id})
    pub tracking_issue_id: Option<u64>,

    /// Stabilization PR id (https://github.com/rust-lang/rust/pull/{id})
    pub stabilization_pr_id: Option<u64>,

    /// Documentation path (https://doc.rust-lang.org/{path})
    pub doc_path: Option<String>,

    /// Edition guide path (https://doc.rust-lang.org/edition-guide/{path})
    pub edition_guide_path: Option<String>,

    /// Unstable book path (https://doc.rust-lang.org/unstable-book/{path})
    pub unstable_book_path: Option<String>,

    /// Language items (functions, structs, modules) that are part of this
    /// feature (unless this feature is exactly one item and that item is
    /// already used as the title)
    #[serde(default)]
    pub items: Vec<String>,

    /// The version number at which the feature was stabilized.
    pub version_number: Option<String>,
}

use alfred::{Item, ItemBuilder};

impl FeatureData {
    pub fn to_alfred_item(&self) -> Item<'static> {
        let mut builder = ItemBuilder::new(self.title.clone());

        match self.version_number.as_deref() {
            Some(v) => builder.set_subtitle(format!("since v{}", v)),
            None => builder.set_subtitle("unstable"),
        };

        builder.into_item()
    }
}
