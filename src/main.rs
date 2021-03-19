use std::{env, io};

use alfred_caniuse_rs::{exit_alfred_error, Db};

const DB_URL: &str = "https://caniuse.rs/features.json";

fn main() {
    let res = try_main().and_then(|items| Ok(alfred::json::write_items(io::stdout(), &items)?));

    if let Err(err) = res {
        eprintln!("{}", err);
        exit_alfred_error();
    }
}

fn try_main() -> eyre::Result<Vec<alfred::Item<'static>>> {
    let mut args = env::args();
    args.next();

    let query = args
        .next()
        .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "no query"))?;

    let db = Db::fetch(DB_URL)?;
    let (feature, _) = db.lookup(&query).ok_or(io::Error::new(
        io::ErrorKind::InvalidInput,
        "no feature match",
    ))?;

    let item = feature.to_alfred_item();

    Ok(vec![item])
}
