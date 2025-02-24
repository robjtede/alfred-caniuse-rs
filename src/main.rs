#![deny(rust_2018_idioms, nonstandard_style, future_incompatible)]
#![warn(clippy::uninlined_format_args)]

use std::{env, io};

use alfred_caniuse_rs::{Db, cache_fetch, cache_put, exit_alfred_error, self_update_check_item};
use eyre::eyre;

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

    let db = match cache_fetch() {
        Some(db) => db,
        None => {
            let db = Db::fetch(CANIUSE_URL)?;
            cache_put(&db);
            db
        }
    };

    match args.next() {
        None => show_recent_versions(&db, &mut items),
        Some(query) if query.is_empty() => show_recent_versions(&db, &mut items),

        Some(query) => match_query(&db, &query.to_lowercase(), &mut items),
    }?;

    Ok(items)
}

fn show_recent_versions(db: &Db, items: &mut Vec<alfred::Item<'static>>) -> eyre::Result<()> {
    let versions = db.versions_preview().map(|v| v.to_alfred_item());
    items.extend(versions);

    Ok(())
}

fn match_query(db: &Db, query: &str, items: &mut Vec<alfred::Item<'static>>) -> eyre::Result<()> {
    let features = db.lookup(query);

    if features.is_empty() {
        return Err(eyre!("no feature match"));
    }

    // let (feature, _) = db
    //     .get_feature(&query)
    //     .ok_or_else(|| )?;

    // let item = feature.to_alfred_item(CANIUSE_URL);
    // items.push(item);

    items.extend(
        features
            .into_iter()
            .map(|feat| feat.to_alfred_item(CANIUSE_URL)),
    );

    Ok(())
}
