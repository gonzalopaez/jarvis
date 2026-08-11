use jarvis_core::validate_bind_address;
use std::net::SocketAddr;

fn address(value: &str) -> SocketAddr {
    value.parse().expect("valid fixture address")
}

#[test]
fn loopback_and_private_addresses_are_allowed() {
    for value in [
        SocketAddr::from(([127, 0, 0, 1], 4100)),
        SocketAddr::from(([10, 20, 30, 40], 4100)),
        SocketAddr::from(([172, 16, 0, 1], 4100)),
        SocketAddr::from(([172, 31, 255, 254], 4100)),
        SocketAddr::from(([192, 168, 50, 2], 4100)),
        address("[::1]:4100"),
        address("[fd00::10]:4100"),
    ] {
        assert!(
            validate_bind_address(value).is_ok(),
            "{value} should be private"
        );
    }
}

#[test]
fn unspecified_and_public_addresses_are_denied() {
    for value in [
        "0.0.0.0:4100",
        "1.1.1.1:4100",
        "172.15.0.1:4100",
        "172.32.0.1:4100",
        "[::]:4100",
        "[2001:4860:4860::8888]:4100",
    ] {
        assert!(
            validate_bind_address(address(value)).is_err(),
            "{value} should be denied"
        );
    }
}
