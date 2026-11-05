//! Where Find and Quickplay see lobbies (opentennis's `net.rs`, M2f and M2j): the DHT board and the LAN.
//!
//! - **The DHT board**: a host announces itself on the fixed infohash [`TOPIC`] (BEP 5, every mainline node supports
//!   it) with its lobby number as the port, and puts its [`Listing`] as a BEP 44 mutable item under the well-known
//!   [`BOARD`] key, salted with that number and signed again with its own endpoint key. Find = `get_peers(TOPIC)`,
//!   then each lobby number's listing; listings older than [`STALE`] are dropped.
//! - **The LAN**: every endpoint advertises itself by mDNS (service [`MDNS`]), so bare IDs resolve on a shared
//!   network with no internet; a host puts its listing (with its room code) in the mDNS record's user data, and
//!   Find and a code's lookup read those first.

// In a browser (N4) there is no DHT and no mDNS, so their halves go unused there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports, unused_variables, unreachable_code))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint_info::UserData;
use iroh::{EndpointId, SecretKey, Signature};
#[cfg(not(target_arch = "wasm32"))]
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
#[cfg(not(target_arch = "wasm32"))]
use n0_future::StreamExt;
#[cfg(not(target_arch = "wasm32"))]
use n0_mainline::{Dht, Id, MutableItem, SigningKey};

#[cfg(target_arch = "wasm32")]
use crate::codes::Dht;
/// A browser has no multicast (N4): no LAN lobbies there. A stand-in nobody can make.
#[cfg(target_arch = "wasm32")]
pub enum MdnsAddressLookup {}

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
#[cfg(target_arch = "wasm32")]
pub async fn find(dht: &Dht) -> Vec<Listing> {
    match *dht {}
}

/// The open lobbies on the DHT board now.
#[cfg(not(target_arch = "wasm32"))]
pub async fn find(dht: &Dht) -> Vec<Listing> {
    let mut ports = std::collections::BTreeSet::new();
    if let Ok(mut s) = dht.get_peers(Id::from_bytes(TOPIC).unwrap()).await {
        // the query ends by itself once the closest nodes answered; a cap in case it doesn't
        let _ = n0_future::time::timeout(Duration::from_secs(30), async {
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
#[cfg(target_arch = "wasm32")]
pub async fn announce(dht: &Dht, _: &SecretKey, _: &Room) -> Result<(), String> {
    match *dht {}
}

/// Puts `room`'s listing on the DHT board (as above).
#[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(target_arch = "wasm32")]
    match mdns {}
    #[cfg(not(target_arch = "wasm32"))]
    watch(mdns, lan.clone());
    lan
}

#[cfg(not(target_arch = "wasm32"))]
fn watch(mdns: MdnsAddressLookup, l: Lan) {
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
}

/// The mDNS service a host lists its room code `code` on beside [`MDNS`] (`_openmx-k7qx._udp.local`), so a code's
/// lookup asks that host alone (K42). On [`MDNS`] every endpoint of the network answers the same question, and
/// swarm-discovery lets only the first ⌈τφ⌉ = 2 answers out per round (τ 0.7 s, φ 2.5, its interactive cadence)
/// and holds the rest back: with N endpoints listening, a host is heard about 2/N of the rounds, and a guest
/// waiting for one particular listing can go [`LAN_CODE_WAIT`](crate::net) without it.
pub fn code_service(code: &str) -> String {
    format!("{MDNS}-{}", code.to_ascii_lowercase())
}

/// What the endpoint publishes (its addresses and listing), put out again on the room code's own service
/// ([`code_service`]) while its host has one; registered on the endpoint as an address lookup that resolves
/// nothing.
#[derive(Clone, Default)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct CodeListing(Arc<Mutex<Coded>>);

#[derive(Default)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
struct Coded {
    /// The code listed on, and its service's mDNS.
    code: Option<(String, MdnsAddressLookup)>,
    /// What the endpoint last published.
    data: Option<iroh::address_lookup::EndpointData>,
}

impl CodeListing {
    /// Lists endpoint `id` on `code`'s service from now on (`None`: on none); the same code again changes nothing.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn list(&self, id: EndpointId, code: Option<&str>) {
        use iroh::address_lookup::AddressLookup;
        let mut s = self.0.lock().unwrap();
        if s.code.as_ref().map(|c| c.0.as_str()) == code {
            return;
        }
        s.code = code.and_then(|c| {
            let m = MdnsAddressLookup::builder().service_name(code_service(c)).build(id).inspect_err(|e| eprintln!("mDNS {c}: {e}")).ok()?;
            Some((c.to_string(), m))
        });
        if let (Some((_, m)), Some(d)) = (&s.code, &s.data) {
            m.publish(d);
        }
    }

    /// Lists nothing: a browser has no mDNS.
    #[cfg(target_arch = "wasm32")]
    pub fn list(&self, _: EndpointId, _: Option<&str>) {}
}

#[cfg(not(target_arch = "wasm32"))]
impl iroh::address_lookup::AddressLookup for CodeListing {
    fn publish(&self, data: &iroh::address_lookup::EndpointData) {
        let mut s = self.0.lock().unwrap();
        s.data = Some(data.clone());
        if let Some((_, m)) = &s.code {
            m.publish(data);
        }
    }
}

/// The host listing room code `code` on the LAN, as it answers on [`code_service`]: its address, for as long as the
/// receiver is kept (none in a browser).
pub fn watch_code(me: EndpointId, code: &str) -> tokio::sync::mpsc::UnboundedReceiver<iroh::EndpointAddr> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Ok(mdns) = MdnsAddressLookup::builder().service_name(code_service(code)).advertise(false).build(me) else { return rx };
        let code = code.to_string();
        tokio::spawn(async move {
            let mut events = mdns.subscribe().await;
            loop {
                tokio::select! {
                    _ = tx.closed() => return,
                    e = events.next() => match e {
                        Some(DiscoveryEvent::Discovered { endpoint_info: i, .. }) => {
                            if lan_listing(i.endpoint_id, i.data.user_data()).is_some_and(|l| l.code == code) && tx.send(i.to_endpoint_addr()).is_err() {
                                return;
                            }
                        }
                        Some(_) => {}
                        None => return,
                    },
                }
            }
        });
    }
    rx
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
    fn a_room_code_s_service_is_a_short_mdns_name_of_its_own() {
        // DNS-SD's service label is at most 15 bytes; names compare without case
        for code in ["K7QX", "2222", "ZZZZ"] {
            let s = code_service(code);
            assert!(s.len() <= 15 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "{s}");
            assert_eq!(code_service(&code.to_ascii_lowercase()), s);
        }
        assert_ne!(code_service("K7QX"), code_service("K7QY"));
        assert_ne!(code_service("K7QX"), MDNS);
    }

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
