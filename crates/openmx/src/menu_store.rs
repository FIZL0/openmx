//! The mod store's screen (P198): Mods' Browse the Store, a picture grid of the store's mods under a
//! tab a kind (B372/B373), each card its name, author, source game, size, Verified, and Installed or
//! Update. Cross installs (or updates) the lit card, Square removes it, Triangle reports it, Start
//! reads the store again, L1/R1 change the tab.
//!
//! The menu knows the store only as this plain view, which the shell fills from `modstore.rs` and
//! whose asks it takes ([`Menu::take_store_asks`]), so the menu's tests mount nothing of the store.

use super::*;

/// The grid's tabs: `modstore::TABS`.
pub const STORE_TABS: [&str; 3] = ["Tracks", "Trick Packs", "Music"];
/// The grid's columns.
pub const STORE_COLUMNS: usize = 4;

/// A card's mod as against the mods folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Got {
    #[default]
    No,
    Installed,
    /// Installed from another zip of this mod: Cross updates it.
    Update,
    /// A folder of its id in the mods folder that is not the store's, which Install leaves alone.
    Own,
}

/// One mod on the grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    /// The store's key for it (`<author>/<id>`, B248), which the asks name it by.
    pub key: String,
    /// Which of [`STORE_TABS`] it is on.
    pub tab: usize,
    pub name: String,
    pub author: String,
    pub source: String,
    /// Its download's bytes.
    pub size: u64,
    pub verified: bool,
    pub got: Got,
    /// Its preview's name in the menu's art ([`Art::track_image`]), whether or not it is in yet.
    pub picture: String,
    /// An install under way: how far, 0 to 1, and what to say.
    pub progress: Option<(f32, String)>,
}

/// Where the listing is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Listed {
    #[default]
    Asking,
    Shown,
    Failed(String),
}

/// What the store screen asks the shell for, by [`Card::key`].
#[derive(Clone, Debug, PartialEq)]
pub enum StoreAsk {
    /// The listing; `fresh` past the hour's kept one (Start).
    List { fresh: bool },
    Install(String),
    Remove(String),
    Report(String),
}

/// The store screen's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shop {
    pub cards: Vec<Card>,
    pub listed: Listed,
    /// A line over the grid: the store was busy, or what the last install or removal did.
    pub note: Option<String>,
    pub tab: usize,
    /// Whether there is a store at all: `--mods off` has no folder to install into.
    pub off: bool,
    asks: Vec<StoreAsk>,
}

impl Shop {
    /// The cards on the tab, as indices into [`Shop::cards`].
    pub fn shown(&self) -> Vec<usize> {
        (0..self.cards.len()).filter(|&i| self.cards[i].tab == self.tab).collect()
    }
}

/// "340 MB", "12 KB".
pub fn size_words(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 10 << 20 => format!("{} MB", n >> 20),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n => format!("{} KB", (n >> 10).max(1)),
    }
}

impl Menu {
    /// Mods' Browse the Store: the store screen, its listing asked for (the hour's kept one, if any).
    pub(super) fn open_store(&mut self) {
        self.push(Screen::Store);
        self.cursor[Screen::Store.index()] = 0;
        self.ask_store_listing();
    }

    /// The store's listing asked for (the hour's kept one, if any), as Browse the Store asks it; and
    /// as `--screen store`, which opens the screen without Browse, does.
    pub fn ask_store_listing(&mut self) {
        if self.store.off {
            return;
        }
        if self.store.cards.is_empty() {
            self.store.listed = Listed::Asking;
        }
        self.store.asks.push(StoreAsk::List { fresh: false });
    }

    /// What the store screen asked since this was last asked.
    pub fn take_store_asks(&mut self) -> Vec<StoreAsk> {
        std::mem::take(&mut self.store.asks)
    }

    /// The lit card, as an index into [`Shop::cards`].
    pub fn store_lit(&self) -> Option<usize> {
        let shown = self.store.shown();
        shown.get(self.cursor[Screen::Store.index()].min(shown.len().saturating_sub(1))).copied()
    }

    /// [`Menu::walk`]'s store screen.
    pub(super) fn walk_store(&mut self, p: Press) {
        let at = Screen::Store.index();
        if p.l1 || p.r1 {
            let n = STORE_TABS.len();
            self.store.tab = (self.store.tab + if p.l1 { n - 1 } else { 1 }) % n;
            self.cursor[at] = 0;
            return;
        }
        let lit = self.store_lit();
        // Triangle is also back: Report before it, as Free Ride's Wrecks is
        if p.triangle {
            if let Some(i) = lit {
                self.store.asks.push(StoreAsk::Report(self.store.cards[i].key.clone()));
                self.store.note = Some(format!("Report {}: opened in your browser", self.store.cards[i].name));
            }
            return;
        }
        if p.back {
            self.pop();
            return;
        }
        if p.start && !self.store.off {
            self.store.listed = Listed::Asking;
            self.store.asks.push(StoreAsk::List { fresh: true });
            return;
        }
        let n = self.store.shown().len();
        if n > 0 {
            let c = self.cursor[at].min(n - 1);
            self.cursor[at] = if p.left {
                c.saturating_sub(1)
            } else if p.right {
                (c + 1).min(n - 1)
            } else if p.up {
                c.checked_sub(STORE_COLUMNS).unwrap_or(c)
            } else if p.down {
                if c + STORE_COLUMNS < n { c + STORE_COLUMNS } else { c }
            } else {
                c
            };
        }
        let Some(i) = lit else { return };
        let card = &mut self.store.cards[i];
        if p.ok && card.progress.is_none() && matches!(card.got, Got::No | Got::Update) {
            card.progress = Some((0.0, "Waiting to download".into()));
            self.store.asks.push(StoreAsk::Install(card.key.clone()));
        } else if p.delete && card.progress.is_none() && matches!(card.got, Got::Installed | Got::Update) {
            self.store.asks.push(StoreAsk::Remove(card.key.clone()));
        }
    }
}
