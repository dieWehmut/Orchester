use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use orchester_netz::{ServerConfig, StaticAssets};

#[test]
fn server_config_defaults_to_ephemeral_ipv4_loopback() {
    let config = ServerConfig::default();

    assert_eq!(config.bind_ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(config.port(), 0);
    assert_eq!(
        config.socket_addr(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    );
    assert_eq!(config.static_assets(), &StaticAssets::Disabled);
}

#[test]
fn server_config_preserves_explicit_network_and_static_asset_settings() {
    let assets = PathBuf::from("web-dist");
    let config = ServerConfig::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4317)
        .with_static_assets(StaticAssets::Directory(assets.clone()));

    assert_eq!(config.bind_ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(config.port(), 4317);
    assert_eq!(
        config.socket_addr(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4317)
    );
    assert_eq!(config.static_assets(), &StaticAssets::Directory(assets));
}
