//! Caniuse

#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(missing_docs)]

use std::{fmt, io, process};

mod cache;
mod db;
mod models;
mod update;

pub use self::{
    cache::{cache_fetch, cache_put},
    db::Db,
    models::{CompilerVersionData, FeatureData},
    update::self_update_check_item,
};

/// Crate Alfred readable error row.
pub fn alfred_error(err: impl fmt::Display + 'static) -> alfred::Item<'static> {
    alfred::ItemBuilder::new("error")
        .subtitle(format!("{}", err))
        .valid(false)
        .into_item()
}

/// Output Alfred readable error row to stdout and exit.
pub fn exit_alfred_error(err: impl fmt::Display + 'static) -> ! {
    alfred::json::write_items(io::stdout(), &[alfred_error(err)]).unwrap();
    process::exit(1);
}
