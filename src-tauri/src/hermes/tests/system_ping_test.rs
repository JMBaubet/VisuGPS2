//! Tests de l'extraction de l'hôte du ping système.

use crate::hermes::system_ping::host_from_base_url;

#[test]
fn extracts_host_from_the_default_url() {
    assert_eq!(
        host_from_base_url("http://100.118.62.10:9000/api/v1").as_deref(),
        Some("100.118.62.10")
    );
}

#[test]
fn extracts_hostname_without_port() {
    assert_eq!(
        host_from_base_url("https://hermes.local/api/v1").as_deref(),
        Some("hermes.local")
    );
}

#[test]
fn extracts_ipv6_host_and_strips_brackets() {
    assert_eq!(
        host_from_base_url("http://[fd00::1]:9000/api/v1").as_deref(),
        Some("fd00::1")
    );
}

#[test]
fn rejects_urls_without_host() {
    assert_eq!(host_from_base_url("not-a-url"), None);
    assert_eq!(host_from_base_url("http://:9000/api/v1"), None);
    assert_eq!(host_from_base_url("http:///api/v1"), None);
}

#[test]
fn rejects_options_and_unexpected_characters() {
    assert_eq!(host_from_base_url("http://-oops/api/v1"), None);
    assert_eq!(host_from_base_url("http://host name/api/v1"), None);
}
