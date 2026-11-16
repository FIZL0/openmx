//! A `--peer` ride's quad picks, checked against every machine's packs (MD152, MD174).
//!
//! `--peer`s have no room and no MD37e handshake, so nothing else says whether every machine holds
//! a seat's quad pack, and the same files under it. Each machine sends a [`Card`] wrapped around its
//! P318 blob: its own quad pick and every quad pack it holds, each with its files' hash. Every
//! machine hears every seat's card, so every machine takes the same picks and refuses the same ones
//! ([`refused`]): a pick rides only where every machine holds its pack with the picker's hash.

/// A machine's quad card: its pick (`pack/stem`, empty for none), and every quad pack it holds by
/// id with the hash of the files under its folder (as the lobby hashes a pack, MD37e).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    pub pick: String,
    pub held: Vec<(String, u64)>,
}

/// [`wrap`]'s first bytes.
pub const WRAP: &[u8; 4] = b"Q152";

/// A machine's `card` wrapped around its P318 blob (`inner`): [`WRAP`], the pick's length and the
/// pick, the count of packs held, each pack's id's length, id and hash (little end first), then the
/// rest. A card with no pick and no packs adds nothing; a pick or id over 255 bytes is left out.
pub fn wrap(card: &Card, inner: Vec<u8>) -> Vec<u8> {
    let pick = if card.pick.len() > 255 { "" } else { card.pick.as_str() };
    let held: Vec<&(String, u64)> = card.held.iter().filter(|(id, _)| id.len() <= 255).take(255).collect();
    if pick.is_empty() && held.is_empty() {
        return inner;
    }
    let mut out = WRAP.to_vec();
    out.push(pick.len() as u8);
    out.extend(pick.as_bytes());
    out.push(held.len() as u8);
    for (id, hash) in held {
        out.push(id.len() as u8);
        out.extend(id.as_bytes());
        out.extend(hash.to_le_bytes());
    }
    out.extend(inner);
    out
}

/// [`wrap`] read back: the card and the blob inside. Bytes not wrapped (or cut short) have an empty
/// card: no pick and no packs.
pub fn unwrap(bytes: &[u8]) -> (Card, &[u8]) {
    fn read(bytes: &[u8]) -> Option<(Card, &[u8])> {
        let rest = bytes.strip_prefix(&WRAP[..])?;
        let (&len, rest) = rest.split_first()?;
        let (pick, rest) = rest.split_at_checked(len as usize)?;
        let (&count, mut rest) = rest.split_first()?;
        let mut held = Vec::new();
        for _ in 0..count {
            let (&len, after) = rest.split_first()?;
            let (id, after) = after.split_at_checked(len as usize)?;
            let (hash, after) = after.split_first_chunk::<8>()?;
            held.push((String::from_utf8_lossy(id).into_owned(), u64::from_le_bytes(*hash)));
            rest = after;
        }
        Some((Card { pick: String::from_utf8_lossy(pick).into_owned(), held }, rest))
    }
    read(bytes).unwrap_or((Card::default(), bytes))
}

/// Why seat `s`'s quad pick is refused, every seat's card (`cards`, by seat) heard: `None` where
/// it has no pick, or where every seat's machine holds the pick's pack with the hash the picker's
/// machine holds it with. Every machine reads the same cards, so every machine says the same.
pub fn refused(cards: &[Card], s: usize) -> Option<String> {
    let pick = &cards.get(s)?.pick;
    if pick.is_empty() {
        return None;
    }
    let Some((id, _)) = pick.rsplit_once('/') else { return Some(format!("{pick} names no pack")) };
    let hash = |c: &Card| c.held.iter().find(|(held, _)| held == id).map(|&(_, h)| h);
    let Some(want) = hash(&cards[s]) else { return Some(format!("seat {}'s machine sent no hash of {id}", s + 1)) };
    cards.iter().enumerate().find_map(|(other, card)| match hash(card) {
        None => Some(format!("seat {}'s machine has no quad pack {id}", other + 1)),
        Some(h) if h != want => Some(format!("seat {}'s {id} differs from seat {}'s", other + 1, s + 1)),
        Some(_) => None,
    })
}
