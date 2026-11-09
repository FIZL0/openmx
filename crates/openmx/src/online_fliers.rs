//! **Fliers online** (P318): which flier packs' planes a seat can pick in an online ride, and the
//! byte its pad picks one by.
//!
//! Every machine runs every seat's physics, so a plane a seat flies needs its files on every machine.
//! The ride's flier packs are one machine's packs of the machine packs' kind ([`KIND`], the one kind
//! whose reader flies anything) — the host's in a lobby's ride, the first seat's machine's on
//! `--peer`s — in key order, so every machine numbers them alike without a list of known packs: a
//! pack is its slot, and a plane skin is its place among that pack's plane skins, which are the same
//! on every machine holding the same files ([`openmx_net::Pack::hash`]). A pick is offered where
//! every seat's machine holds those files, and greyed with who lacks them where one does not
//! (`needs MX vs. ATV Unleashed biplane and Kiowa, missing for 2P`). No files go between
//! players: each installs their own.
//!
//! A seat's pick is its pad's `net::FLIER` byte ([`byte`]), which every machine reads at the tick it
//! lands on, as N17's switch: its next switch onto the plane takes a copy of that skin's plane
//! (`net::rides::Flier`), built off the pack's files at the ride's start on every machine.

use openmx_net::Pack;

/// The `mod.json` kind of the packs a flier comes in: the machine packs' (`machine_packs`).
pub const KIND: &str = "machine";

/// How many flier packs a ride numbers, and plane skins in each: what a byte holds ([`byte`]).
pub const SLOTS: usize = 15;
pub const SKINS: usize = 16;

/// The pad byte of the `skin`th plane skin of the ride's `slot`th flier pack: the slot plus one in
/// the high four bits, the skin in the low four. 0, which no flier is, is the ride's own biplane.
pub fn byte(slot: usize, skin: usize) -> Option<u8> {
    (slot < SLOTS && skin < SKINS).then(|| ((slot as u8 + 1) << 4) | skin as u8)
}

/// [`byte`] read back: the pack's slot and the skin's place, `None` for the ride's own biplane.
pub fn of_byte(byte: u8) -> Option<(usize, usize)> {
    (byte >= 16).then(|| ((byte >> 4) as usize - 1, (byte & 15) as usize))
}

/// One plane skin this machine holds: its pack's id, its model's name (`biplane3`, what a pick
/// saves) and what the picker calls it. In the pack's manifest order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skin {
    pub pack: String,
    pub skin: String,
    pub name: String,
}

/// One row of the ride's offer, as this machine lists it: the byte a pick of it sends, its pack and
/// skin (the skin empty for a pack this machine lacks, which it cannot list the skins of), what it
/// is called, and why it cannot be picked — empty where every seat's machine holds its pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offered {
    pub byte: u8,
    pub pack: String,
    pub skin: String,
    pub name: String,
    pub why: String,
}

impl Offered {
    pub fn open(&self) -> bool {
        self.why.is_empty()
    }
}

/// The ride's flier packs: `reference`'s packs of [`KIND`], in key order, the first [`SLOTS`].
pub fn ride_packs(reference: &[Pack]) -> Vec<Pack> {
    let mut packs: Vec<Pack> = reference.iter().filter(|p| p.kind == KIND).cloned().collect();
    packs.sort_by_key(Pack::key);
    packs.truncate(SLOTS);
    packs
}

/// The seats (`2P`) whose machine lacks `pack` or holds other files under its name, among `seats`:
/// each seat's place and its machine's packs (`None`, a machine that said none, lacks them all).
pub fn lacking(pack: &Pack, seats: &[(usize, Option<&[Pack]>)]) -> Vec<String> {
    let holds = |ps: Option<&[Pack]>| ps.is_some_and(|ps| ps.iter().any(|p| p.key() == pack.key() && p.hash == pack.hash));
    seats.iter().filter(|(_, ps)| !holds(*ps)).map(|(n, _)| format!("{}P", n + 1)).collect()
}

/// Why `pack` is greyed, in words, as the room says it ([`openmx_net::Offered::why`]).
fn why(pack: &Pack, lacking: &[String]) -> String {
    if lacking.is_empty() { String::new() } else { format!("needs {}, missing for {}", pack.said(), lacking.join(", ")) }
}

/// The ride's offer as this machine lists it: each of the ride's packs (`ride`, [`ride_packs`]) in
/// turn, as this machine's plane skins of it (`skins`, in manifest order) where it holds the same
/// files (`mine`), else one row for the pack, greyed; each greyed with the seats that lack it. A pack
/// held here with no plane skin has no row, though it keeps its slot.
pub fn offer(ride: &[Pack], seats: &[(usize, Option<&[Pack]>)], mine: &[Pack], skins: &[Skin]) -> Vec<Offered> {
    let mut out = Vec::new();
    for (slot, pack) in ride.iter().enumerate() {
        let why = why(pack, &lacking(pack, seats));
        let held = mine.iter().any(|p| p.key() == pack.key() && p.hash == pack.hash);
        let own: Vec<&Skin> = skins.iter().filter(|s| held && s.pack == pack.id).collect();
        if own.is_empty() {
            // a pack held here with no plane (the quads) has nothing to pick; one this machine
            // lacks may have, and is listed once, greyed
            if held {
                continue;
            }
            let why = if why.is_empty() { format!("{} is not here", pack.said()) } else { why };
            out.push(Offered { byte: byte(slot, 0).unwrap_or(0), pack: pack.id.clone(), skin: String::new(), name: pack.said().to_string(), why });
            continue;
        }
        for (n, s) in own.into_iter().enumerate().take(SKINS) {
            let byte = byte(slot, n).unwrap_or(0);
            out.push(Offered { byte, pack: pack.id.clone(), skin: s.skin.clone(), name: s.name.clone(), why: why.clone() });
        }
    }
    out
}

/// The byte a pick of `pack`'s `skin` sends in `offer`: `Some(0)` for MX Unleashed's own (`pack`
/// the roster's original), `None` where the offer greys it or has it not.
pub fn byte_of(offer: &[Offered], pack: &str, skin: &str, original: &str) -> Option<u8> {
    if pack == original {
        return Some(0);
    }
    offer.iter().find(|o| o.pack == pack && o.skin == skin && o.open()).map(|o| o.byte)
}

/// The pack's key a flier byte rides (`machine/mxvatv_fliers`), for what the ride says it rides
/// (`openmx_net::Cmd::InUse`): `None` for the ride's own biplane or a byte the offer has not.
pub fn key_of(offer: &[Offered], byte: u8) -> Option<String> {
    let o = offer.iter().find(|o| byte != 0 && o.byte == byte)?;
    Some(openmx_net::handshake::pack_key(KIND, &o.pack))
}

/// A `--peer` machine's saved markers and its packs in one piece (P318), what its datagrams offer
/// every peer before the ride ticks (`net::udp`'s marker pieces): [`WRAP`], the markers' length and
/// bytes, then the packs as JSON.
pub fn wrap(marks: &[u8], packs: &[Pack]) -> Vec<u8> {
    let mut out = WRAP.to_vec();
    out.extend((marks.len() as u32).to_le_bytes());
    out.extend(marks);
    out.extend(serde_json::to_vec(packs).unwrap_or_default());
    out
}

/// [`wrap`]'s first bytes.
pub const WRAP: &[u8; 4] = b"P318";

/// [`wrap`] read back: the markers and the packs. Bytes not wrapped are markers alone, and no packs.
pub fn unwrap(bytes: &[u8]) -> (&[u8], Vec<Pack>) {
    let Some(rest) = bytes.strip_prefix(&WRAP[..]) else { return (bytes, Vec::new()) };
    let Some((len, rest)) = rest.split_first_chunk::<4>() else { return (bytes, Vec::new()) };
    let Some((marks, packs)) = rest.split_at_checked(u32::from_le_bytes(*len) as usize) else { return (bytes, Vec::new()) };
    (marks, serde_json::from_slice(packs).unwrap_or_default())
}
