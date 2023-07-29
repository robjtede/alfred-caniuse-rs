//! caniuse.rs object structures.
//!
//! Definitions derived from https://github.com/jplatte/caniuse.rs/blob/e9c940047437cccfaf8ff65bcf68f70538877662/build.rs.

use std::{cmp::Ordering, fmt};

use alfred::{Item, ItemBuilder, Modifier};
use serde::{Deserialize, Serialize};
use time::{macros::format_description, Date};

const RUST_BLOG_ROOT: &str = "https://blog.rust-lang.org/";

/// Versions that have been cut are either stable, beta or nightly.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl Default for Channel {
    fn default() -> Self {
        Self::Stable
    }
}

impl PartialOrd for Channel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Channel {
    fn cmp(&self, other: &Self) -> Ordering {
        let numeric = |&chan| -> u8 {
            match chan {
                Channel::Stable => 0,
                Channel::Beta => 1,
                Channel::Nightly => 2,
            }
        };

        numeric(self).cmp(&numeric(other))
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Channel::Stable => f.write_str("stable"),
            Channel::Beta => f.write_str("beta"),
            Channel::Nightly => f.write_str("nightly"),
        }
    }
}

/// Rust compiler version info.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompilerVersionData {
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

impl CompilerVersionData {
    /// Creates an Alfred item from version data.
    pub fn to_alfred_item(&self) -> Item<'static> {
        let mut builder = ItemBuilder::new(format!("v{} ({})", &self.number, &self.channel));

        if let Some(release_date) = self.release_date() {
            // August 16 2019
            let rel_date_str = release_date
                .format(format_description!("[month repr:long] [day], [year]"))
                .unwrap();
            builder.set_subtitle(format!("Released {rel_date_str}"));
        }

        if let Some(blog_post) = self.blog_post_path.as_deref() {
            let blog_post_url = format!("{}{}", RUST_BLOG_ROOT, blog_post.to_owned());

            builder.set_quicklook_url(blog_post_url.clone());

            builder.set_modifier(
                Modifier::Option,
                Some("Press enter to view release post."),
                Some(blog_post_url),
                true,
                None,
            );
        };

        builder.into_item()
    }
}

impl CompilerVersionData {
    fn release_date(&self) -> Option<Date> {
        self.release_date.as_deref().and_then(|date| {
            Date::parse(date, format_description!("[year repr:full]-[month]-[day]")).ok()
        })
    }
}

impl PartialOrd<CompilerVersionData> for CompilerVersionData {
    fn partial_cmp(&self, other: &CompilerVersionData) -> Option<Ordering> {
        self.channel
            .cmp(&other.channel)
            .then_with(|| {
                let self_rel = match self.release_date() {
                    Some(rel) => rel,
                    None => return Ordering::Equal,
                };

                let other_rel = match other.release_date() {
                    Some(rel) => rel,
                    None => return Ordering::Equal,
                };

                self_rel.cmp(&other_rel)
            })
            .into()
    }
}

/// Rust "feature" info for some arbitrary definition of feature.
///
/// Not strictly tied to compiler features.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
    #[serde(rename = "version")]
    pub version_number: Option<String>,

    /// Alternatives to the title
    #[serde(default)]
    pub aliases: Vec<String>,

    /// Unique "feature" name for caniuse links.
    ///
    /// Filled in after fetch.
    #[serde(default)]
    pub slug: String,
}

impl FeatureData {
    /// Creates an Alfred row item from feature data.
    pub fn to_alfred_item(&self, base_url: &str) -> Item<'static> {
        let mut builder = ItemBuilder::new(self.title.clone());

        match self.version_number.as_deref() {
            Some(v) => builder.set_subtitle(format!("since v{v}")),
            None => builder.set_subtitle("unstable"),
        };

        builder.set_arg(format!("{}/features/{}", base_url, &self.slug));
        builder.set_quicklook_url(format!("{}/features/{}", base_url, &self.slug));

        if self.items.is_empty() {
            // seems to prevent large type activation
            builder.set_text_large_type(" ".to_owned());
        } else {
            builder.set_text_large_type(self.items.join("\n"));
        }

        if let Some(ref doc_path) = self.doc_path {
            let doc_url = format!("https://doc.rust-lang.org/{doc_path}");
            builder.set_quicklook_url(doc_url.clone());

            builder.set_modifier(
                Modifier::Option,
                Some("Press enter to see docs."),
                Some(doc_url),
                true,
                None,
            );
        } else {
            builder.set_modifier(
                Modifier::Option,
                Some("No docs available."),
                None::<String>,
                false,
                None,
            );
        }

        builder.into_item()
    }
}
