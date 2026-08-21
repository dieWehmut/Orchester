use std::net::{IpAddr, Ipv4Addr};

use orchester_netz::{bind_listener, ServerBindError, ServerConfig};

#[test]
fn binding_ephemeral_port_exposes_an_allocated_loopback_address() {
    let bound = bind_listener(&ServerConfig::default()).expect("loopback listener");
    let address = bound.local_addr().expect("listener address");

    assert!(address.ip().is_loopback());
    assert_ne!(address.port(), 0);
}

#[test]
fn binding_rejects_non_loopback_before_opening_a_socket() {
    let config = ServerConfig::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);

    assert!(matches!(
        bind_listener(&config),
        Err(ServerBindError::InvalidConfig(_))
    ));
}
