use alfred_caniuse_rs::Db;
use serde_json::json;

#[test]
fn lookup_returns_each_matching_feature_once() {
    let db = serde_json::from_value::<Db>(json!({
        "versions": {},
        "features": {
            "const_fn": {
                "slug": "const_fn",
                "title": "Constant functions",
                "items": ["constfn()", "CONSTFN(u8)"],
                "aliases": ["constfn", "ConstFn"]
            },
            "aliases_only": {
                "slug": "aliases_only",
                "title": "Alias entries",
                "aliases": ["constfn expression", "CONSTFN item"]
            },
            "item_only": {
                "slug": "item_only",
                "title": "Array lengths",
                "items": ["constfn expression"]
            },
            "const_fns": {
                "slug": "const_fns",
                "title": "Build-time evaluation"
            },
            "flag_only": {
                "slug": "flag_only",
                "title": "Build-time calls",
                "flag": "const_fn"
            },
            "title_only": {
                "slug": "title_only",
                "title": "Const fn"
            },
            "unrelated": {
                "slug": "unrelated",
                "title": "Vector allocation"
            }
        }
    }))
    .unwrap();

    let mut slugs = db
        .lookup("constfn")
        .into_iter()
        .map(|feature| feature.slug.as_str())
        .collect::<Vec<_>>();

    slugs.sort_unstable();

    assert_eq!(
        slugs,
        [
            "aliases_only",
            "const_fn",
            "const_fns",
            "flag_only",
            "item_only",
            "title_only",
        ]
    );
}
