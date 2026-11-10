//! Tests du paquet magique Wake-on-LAN.

use crate::hermes::wake::build_magic_packet;

const MAC_BYTES: [u8; 6] = [0x68, 0x5b, 0x35, 0xa8, 0x5c, 0x65];

#[test]
fn builds_the_standard_magic_packet() {
    let packet = build_magic_packet("68:5b:35:a8:5c:65").expect("MAC valide");

    assert_eq!(packet.len(), 102);
    assert_eq!(&packet[..6], &[0xFF; 6]);
    for repetition in 0..16 {
        let start = 6 + repetition * 6;
        assert_eq!(
            &packet[start..start + 6],
            &MAC_BYTES,
            "répétition {} incorrecte",
            repetition
        );
    }
}

#[test]
fn accepts_dashes_and_uppercase() {
    let with_colons = build_magic_packet("68:5B:35:A8:5C:65").expect("MAC majuscule");
    let with_dashes = build_magic_packet("68-5B-35-A8-5C-65").expect("MAC tirets");
    assert_eq!(with_colons, with_dashes);
}

#[test]
fn rejects_invalid_mac_addresses() {
    assert!(build_magic_packet("68:5b:35:a8:5c").is_err());
    assert!(build_magic_packet("68:5b:35:a8:5c:65:00").is_err());
    assert!(build_magic_packet("zz:5b:35:a8:5c:65").is_err());
    assert!(build_magic_packet("").is_err());
}
