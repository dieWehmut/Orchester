use orchester_netz::{health_response, HealthDto};

#[tokio::test]
async fn health_response_contains_only_stable_service_metadata() {
    let response: HealthDto = health_response();

    assert_eq!(response.status, "ok");
    assert_eq!(response.service, "orchester");
    assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(response.schema_version, 1);
}
