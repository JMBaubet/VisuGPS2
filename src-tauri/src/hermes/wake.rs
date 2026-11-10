//! Réveil Wake-on-LAN du serveur `hermes`.
//!
//! Le paquet magique est construit sans dépendance réseau (`build_magic_packet`),
//! puis envoyé trois fois en UDP. Le client ne boucle pas sur le ping : la vue
//! Hermes interroge `hermes_ping` toutes les 3 s jusqu'à la disponibilité du
//! serveur, avec le budget `Hermes.Reveil.delaiMaxSec`.

use std::net::UdpSocket;

/// Construit le paquet magique WoL : six octets `0xFF` suivis de seize
/// répétitions de l'adresse MAC (6 octets), soit 102 octets.
pub(crate) fn build_magic_packet(mac: &str) -> Result<[u8; 102], String> {
    let bytes = parse_mac(mac)?;
    let mut packet = [0u8; 102];
    for byte in packet.iter_mut().take(6) {
        *byte = 0xFF;
    }
    for repetition in 0..16 {
        let start = 6 + repetition * 6;
        packet[start..start + 6].copy_from_slice(&bytes);
    }
    Ok(packet)
}

/// Envoie le paquet magique trois fois vers `address:port`.
pub(crate) fn send_magic_packet(address: &str, port: u16, mac: &str) -> Result<(), String> {
    let packet = build_magic_packet(mac)?;
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|error| format!("Ouverture du socket UDP impossible : {}", error))?;
    socket
        .set_broadcast(true)
        .map_err(|error| format!("Activation du broadcast impossible : {}", error))?;

    let target = format!("{}:{}", address, port);
    for _ in 0..3 {
        socket
            .send_to(&packet, &target)
            .map_err(|error| format!("Envoi du paquet WoL vers {} impossible : {}", target, error))?;
    }
    Ok(())
}

/// Analyse une MAC `AA:BB:CC:DD:EE:FF` ou `AA-BB-CC-DD-EE-FF`.
fn parse_mac(mac: &str) -> Result<[u8; 6], String> {
    let cleaned: String = mac
        .chars()
        .filter(|character| !matches!(character, ':' | '-' | ' '))
        .collect();
    if cleaned.len() != 12 {
        return Err(format!(
            "Adresse MAC invalide « {} » : 12 chiffres hexadécimaux attendus.",
            mac
        ));
    }

    let mut bytes = [0u8; 6];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&cleaned[start..start + 2], 16).map_err(|_| {
            format!(
                "Adresse MAC invalide « {} » : chiffre hexadécimal attendu.",
                mac
            )
        })?;
    }
    Ok(bytes)
}
