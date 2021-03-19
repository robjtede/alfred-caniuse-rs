#![deny(rust_2018_idioms)]

use std::{io, process};

mod db;
mod models;

pub use db::Db;
pub use models::{FeatureData, VersionData};

pub fn alfred_error() -> alfred::Item<'static> {
    alfred::ItemBuilder::new("error").into_item()
}

pub fn exit_alfred_error() -> ! {
    alfred::json::write_items(io::stdout(), &[alfred_error()]).unwrap();
    process::exit(1);
}
