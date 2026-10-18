use std::time::Duration;

use alfred_caniuse_rs::Db;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

#[tokio::test]
async fn fetch_times_out_when_response_is_delayed() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/features.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "versions": {}, "features": {} }))
                .set_delay(Duration::from_secs(4)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let url = server.uri();
    let err = tokio::task::spawn_blocking(move || Db::fetch(&url))
        .await
        .unwrap()
        .unwrap_err();

    assert!(
        matches!(
            err.downcast_ref::<ureq::Error>(),
            Some(ureq::Error::Timeout(ureq::Timeout::Global))
        ),
        "expected a database fetch timeout, got {err}"
    );
}

#[tokio::test]
async fn fetch_sends_user_agent_and_decodes_database() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/features.json"))
        .and(header(
            "user-agent",
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "versions": {
                "1.39.0": {
                    "number": "1.39.0",
                    "channel": "stable",
                    "release_date": "2019-11-07"
                }
            },
            "features": {
                "async_await": {
                    "title": "Async/await syntax",
                    "version": "1.39.0",
                    "items": ["async", "await"]
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let url = server.uri();
    let db = tokio::task::spawn_blocking(move || Db::fetch(&url))
        .await
        .unwrap()
        .unwrap();

    let (feature, version) = db.get_feature("async_await").unwrap();

    assert_eq!(feature.title, "Async/await syntax");
    assert_eq!(feature.slug, "async_await");
    assert_eq!(version.unwrap().number, "1.39.0");
}
