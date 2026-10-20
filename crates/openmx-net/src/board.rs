//! Where Find and Quickplay see lobbies (opentennis's `net.rs`, M2f and M2j): the DHT board and the LAN.
//!
//! - **The DHT board**: a host announces itself on the fixed infohash [`TOPIC`] (BEP 5, every mainline node supports
//!   it) with its lobby number as the port, and puts its [`Listing`] as a BEP 44 mutable item under the well-known
//!   [`BOARD`] key, salted with that number and signed again with its own endpoint key. Find = `get_peers(TOPIC)`,
//!   then each lobby number's listing; listings older than [`STALE`] are dropped.
//! - **The LAN**: every endpoint advertises itself by mDNS (service [`MDNS`]), so bare IDs resolve on a shared
//!   network with no internet; a host puts its listing (with its room code) in the mDNS record's user data, and
//!   Find and a code's lookup read those first.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint_info::UserData;
use iroh::{EndpointId, SecretKey, Signature};
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
use n0_future::StreamExt;
use n0_mainline::{Dht, Id, MutableItem, SigningKey};

use crate::codes::STALE;
use crate::room::{Listing, Room, now};

/// The lobby list's infohash (any 20 bytes; these spell it).
const TOPIC: &[u8; 20] = b"openmx-lobby-board/1";
/// The listings' shared BEP 44 key (its secret is public on purpose: anyone may list; the endpoint signature inside
/// says who wrote a listing).
const BOARD: &[u8; 32] = b"mx-unleashed-openmx-lobby-board1";
/// The mDNS service name (`<id>._openmx._udp.local`), so only our endpoints answer.
pub const MDNS: &str = "openmx";
/// The LAN listing's prefix in the mDNS user data.
const LAN_TAG: &str = "omx1";

/// The salt of lobby number `n`.
fn salt(n: u16) -> Vec<u8> {
    format!("omx1/{n}").into_bytes()
}

/// A listing's DHT value: the endpoint's signature over salt + JSON, then the JSON.
pub fn seal(key: &SecretKey, n: u16, l: &Listing) -> Vec<u8> {
    let json = serde_json::to_vec(l).unwrap();
    let sig = key.sign(&[salt(n), json.clone()].concat());
    [sig.to_bytes().to_vec(), json].concat()
}

/// The listing in `value` if its endpoint signed it for lobby number `n`.
pub fn unseal(n: u16, value: &[u8]) -> Option<Listing> {
    let (sig, json) = value.split_at_checked(64)?;
    let l: Listing = serde_json::from_slice(json).ok()?;
    let id: EndpointId = l.id.parse().ok()?;
    id.verify(&[salt(n), json.to_vec()].concat(), &Signature::from_bytes(sig.try_into().ok()?)).ok()?;
    Some(l)
}

/// This host's lobby number, from its (random) endpoint ID. Two hosts on one number overwrite each other's listing
/// (birthday odds, ~1 % at 36 hosts at once), as in opentennis.
pub fn lobby_number(id: EndpointId) -> u16 {
    u16::from_le_bytes([id.as_bytes()[0], id.as_bytes()[1]]).max(1)
}

/// The open lobbies on the DHT board now.
pub async fn find(dht: &Dht) -> Vec<Listing> {
    let mut ports = std::collections::BTreeSet::new();
    if let Ok(mut s) = dht.get_peers(Id::from_bytes(TOPIC).unwrap()).await {
        // the query ends by itself once the closest nodes answered; a cap in case it doesn't
        let _ = tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(batch) = s.next().await {
                ports.extend(batch.iter().map(|a| a.port()));
            }
        })
        .await;
    }
    let board = SigningKey::from_bytes(BOARD).verifying_key().to_bytes();
    let mut gets = tokio::task::JoinSet::new();
    for n in ports {
        let dht = dht.clone();
        gets.spawn(async move { unseal(n, dht.get_mutable_most_recent(&board, Some(&salt(n))).await.ok().flatten()?.value()) });
    }
    let mut list: Vec<Listing> = gets.join_all().await.into_iter().flatten().filter(|l| now().saturating_sub(l.time) <= STALE && l.open() > 0).collect();
    list.sort_by(|a, b| (&a.id, b.time).cmp(&(&b.id, a.time)));
    list.dedup_by(|a, b| a.id == b.id);
    list
}

/// Puts `room`'s listing on the DHT board under this host's lobby number (a private lobby lists as full, which
/// Find and Quickplay skip).
pub async fn announce(dht: &Dht, key: &SecretKey, room: &Room) -> Result<(), String> {
    let n = lobby_number(key.public());
    let value = seal(key, n, &Listing::of(&key.public().to_string(), room));
    // (µs: a private / public a second apart still overwrite each other)
    let item = MutableItem::new(&SigningKey::from_bytes(BOARD), &value, crate::codes::unix_micros(), Some(&salt(n)));
    dht.put_mutable(item, None).await.map_err(|e| format!("lobby listing: {e}"))?;
    dht.announce_peer(Id::from_bytes(TOPIC).unwrap(), Some(n)).await.map_err(|e| format!("lobby announce: {e}"))?;
    Ok(())
}

/// A listing as mDNS user data (≤ 245 bytes: the ID is the record's own, the time the finder's clock).
pub fn lan_data(l: &Listing) -> Option<UserData> {
    let json = serde_json::to_string(&Listing { id: String::new(), time: 0, ..l.clone() }).ok()?;
    format!("{LAN_TAG}{json}").parse().ok()
}

/// The listing in endpoint `id`'s mDNS user data, if it's hosting.
pub fn lan_listing(id: EndpointId, data: Option<&UserData>) -> Option<Listing> {
    let l: Listing = serde_json::from_str(data?.as_ref().strip_prefix(LAN_TAG)?).ok()?;
    Some(Listing { id: id.to_string(), time: now(), ..l })
}

/// The lobbies heard on the LAN, kept current by mDNS.
pub type Lan = Arc<Mutex<BTreeMap<EndpointId, Listing>>>;

/// Keeps `lan` to the hosts mDNS hears now (a host's record updates when its seats change, and goes when it stops);
/// none without mDNS.
pub fn watch_lan(mdns: Option<MdnsAddressLookup>) -> Lan {
    let lan = Lan::default();
    let Some(mdns) = mdns else { return lan };
    let l = lan.clone();
    tokio::spawn(async move {
        let mut events = mdns.subscribe().await;
        while let Some(e) = events.next().await {
            let mut l = l.lock().unwrap();
            match e {
                DiscoveryEvent::Discovered { endpoint_info: i, .. } => match lan_listing(i.endpoint_id, i.data.user_data()) {
                    Some(listing) => l.insert(i.endpoint_id, listing),
                    None => l.remove(&i.endpoint_id),
                },
                DiscoveryEvent::Expired { endpoint_id } => l.remove(&endpoint_id),
                _ => None,
            };
        }
    });
    lan
}

/// The open lobbies on the LAN.
pub fn lan_list(lan: &Lan) -> Vec<Listing> {
    lan.lock().unwrap().values().filter(|l| l.open() > 0).cloned().collect()
}

/// The LAN lobby with room code `code` (open or not: its host answers a full one with why).
pub fn lan_code(lan: &Lan, code: &str) -> Option<Listing> {
    lan.lock().unwrap().values().find(|l| l.code == code).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::{HOST, Pick};

    #[test]
    fn a_listing_is_bound_to_its_lobby_number_and_fits_mdns() {
        let key = SecretKey::generate();
        let mut r = Room::new(&"🏍".repeat(24), "free06", 8);
        r.take(HOST, &[Pick::new("thq1", "race125p")]);
        r.short = "K7QX".into();
        let l = Listing::of(&key.public().to_string(), &r);
        let v = seal(&key, 7, &l);
        assert_eq!(unseal(7, &v), Some(l.clone()));
        assert_eq!(unseal(8, &v), None, "another lobby number's salt");
        let other = SecretKey::generate();
        assert_eq!(unseal(7, &seal(&other, 7, &Listing { id: key.public().to_string(), ..l.clone() })), None, "signed by someone else");
        let d = lan_data(&l).expect("a full lobby name still fits the mDNS record");
        let back = lan_listing(key.public(), Some(&d)).unwrap();
        assert_eq!((back.code.as_str(), back.open(), back.track.as_str()), ("K7QX", 7, "free06"));
    }
}
