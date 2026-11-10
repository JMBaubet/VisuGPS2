//! Tests des helpers purs du client HTTP.

use crate::hermes::client::{join_url, map_status, parse_detail, validate_id};
use crate::hermes::types::codes;

#[test]
fn detail_string_is_extracted() {
    assert_eq!(
        parse_detail(r#"{"detail":"communes.geojson absent pour dataset 'paca'"}"#),
        Some("communes.geojson absent pour dataset 'paca'".to_string())
    );
}

#[test]
fn detail_validation_array_is_formatted() {
    let detail = r#"{"detail":[{"loc":["body","points"],"msg":"field required","type":"value_error.missing"}]}"#;
    assert_eq!(
        parse_detail(detail),
        Some("body.points : field required".to_string())
    );
}

#[test]
fn detail_without_location_keeps_message() {
    assert_eq!(
        parse_detail(r#"{"detail":[{"msg":"valeur invalide"}]}"#),
        Some("valeur invalide".to_string())
    );
}

#[test]
fn detail_is_none_on_non_json_body() {
    assert_eq!(parse_detail("<html>Bad Gateway</html>"), None);
    assert_eq!(parse_detail(r#"{"message":"autre format"}"#), None);
}

#[test]
fn statuses_are_mapped_to_machine_codes() {
    assert_eq!(map_status(400), codes::INVALID);
    assert_eq!(map_status(405), codes::INVALID);
    assert_eq!(map_status(422), codes::INVALID);
    assert_eq!(map_status(401), codes::FORBIDDEN);
    assert_eq!(map_status(403), codes::FORBIDDEN);
    assert_eq!(map_status(404), codes::NOT_FOUND);
    assert_eq!(map_status(409), codes::CONFLICT);
    assert_eq!(map_status(500), codes::SERVER);
    assert_eq!(map_status(503), codes::SERVER);
    assert_eq!(map_status(418), codes::SERVER);
}

#[test]
fn identifiers_are_validated() {
    assert!(validate_id("provence-alpes-cote-dazur", "dataset").is_ok());
    assert!(validate_id("ma-zone_nice", "polygone").is_ok());
    assert!(validate_id(&"a".repeat(64), "polygone").is_ok());

    assert!(validate_id("", "dataset").is_err());
    assert!(validate_id("a/b", "dataset").is_err());
    assert!(validate_id("a b", "dataset").is_err());
    assert!(validate_id(&"a".repeat(65), "dataset").is_err());
}

#[test]
fn urls_are_joined_without_double_slash() {
    assert_eq!(
        join_url("http://10.15.58.131:9000/api/v1/", "/ping"),
        "http://10.15.58.131:9000/api/v1/ping"
    );
    assert_eq!(
        join_url("http://10.15.58.131:9000/api/v1", "datasets"),
        "http://10.15.58.131:9000/api/v1/datasets"
    );
}
