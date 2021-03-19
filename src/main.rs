#![deny(rust_2018_idioms, nonstandard_style)]

use std::{env, io};

use alfred_caniuse_rs::{cache_fetch, cache_put, exit_alfred_error, self_update_check_item, Db};

const CANIUSE_URL: &str = "https://caniuse.rs";

fn main() {
    let res = try_main().and_then(|items| Ok(alfred::json::write_items(io::stdout(), &items)?));

    if let Err(err) = res {
        exit_alfred_error(err);
    }
}

fn try_main() -> eyre::Result<Vec<alfred::Item<'static>>> {
    let mut items = vec![];

    // check for workflow update and add row if needed
    items.extend(self_update_check_item());

    let mut args = env::args();
    // skip self binary arg
    args.next();

    // TODO: allow empty query to show recent versions

    let query = args
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no query"))?;

    let db = match cache_fetch() {
        Some(db) => db,
        None => {
            let db = Db::fetch(CANIUSE_URL)?;
            cache_put(&db);
            db
        }
    };

    // TODO: fuzzy matching

    let (feature, _) = db
        .lookup(&query)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no feature match"))?;

    let item = feature.to_alfred_item(CANIUSE_URL);
    items.push(item);

    Ok(items)
}
