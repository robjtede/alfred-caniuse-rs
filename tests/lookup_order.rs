use std::collections::HashMap;

use alfred_caniuse_rs::{Db, FeatureData};
use serde_json::json;

fn database(features: &[FeatureData]) -> Db {
    let features = features
        .iter()
        .map(|feature| (&feature.slug, feature))
        .collect::<HashMap<_, _>>();

    serde_json::from_value(json!({ "versions": {}, "features": features })).unwrap()
}

#[test]
fn lookup_puts_exact_slug_first() {
    let features = ["async", "xasync"].map(|slug| FeatureData {
        slug: slug.to_owned(),
        ..FeatureData::default()
    });

    let db = database(&features);

    for query in ["async", "xasync"] {
        assert_eq!(db.lookup(query)[0].slug, query);
    }
}

#[test]
fn lookup_puts_exact_flag_and_title_before_slug_substrings() {
    for use_flag in [false, true] {
        let features = ["async", "xasync"].map(|query| {
            let mut feature = FeatureData {
                slug: format!("{query}_feature"),
                ..FeatureData::default()
            };

            if use_flag {
                feature.flag = Some(query.to_uppercase());
            } else {
                feature.title = query.to_uppercase();
            }

            feature
        });

        let db = database(&features);

        for query in ["async", "xasync"] {
            assert_eq!(db.lookup(query)[0].slug, format!("{query}_feature"));
        }
    }
}

#[test]
fn lookup_orders_match_ranks_and_slug_ties_across_database_loads() {
    let features = [
        FeatureData {
            slug: "z-exact-title".to_owned(),
            title: "Async".to_owned(),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "y-exact-flag".to_owned(),
            flag: Some("ASYNC".to_owned()),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "async".to_owned(),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "a-substring-alias".to_owned(),
            aliases: vec!["async operations".to_owned()],
            ..FeatureData::default()
        },
        FeatureData {
            slug: "b-substring-item".to_owned(),
            items: vec!["async operations".to_owned()],
            ..FeatureData::default()
        },
        FeatureData {
            slug: "c-substring-async".to_owned(),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "d-substring-flag".to_owned(),
            flag: Some("async_functions".to_owned()),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "e-substring-title".to_owned(),
            title: "Async functions".to_owned(),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "a-fuzzy-title".to_owned(),
            title: "Asyn".to_owned(),
            ..FeatureData::default()
        },
        FeatureData {
            slug: "b-fuzzy-flag".to_owned(),
            flag: Some("xasyn".to_owned()),
            ..FeatureData::default()
        },
    ];

    for _ in 0..10 {
        let db = database(&features);
        let slugs = db
            .lookup("async")
            .into_iter()
            .map(|feature| feature.slug.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            slugs,
            [
                "async",
                "y-exact-flag",
                "z-exact-title",
                "a-substring-alias",
                "b-substring-item",
                "c-substring-async",
                "d-substring-flag",
                "e-substring-title",
                "a-fuzzy-title",
                "b-fuzzy-flag",
            ]
        );
    }
}
