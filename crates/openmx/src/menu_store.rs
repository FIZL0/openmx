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
/// P199: the store's own pages after the kind tabs: Upload my mod (and Upload all), and My public mods.
pub const STORE_PAGES: [&str; 2] = ["Upload", "My Mods"];
/// Their tabs.
pub const UPLOAD_TAB: usize = STORE_TABS.len();
pub const MINE_TAB: usize = STORE_TABS.len() + 1;
/// What a player agrees to before they upload (`plan/P198.md` § P199): shown first, once a session.
pub const STORE_RULES: [&str; 5] = [
    "Uploads go on the public openMX mod store, for anyone to download.",
    "You are responsible for what you share: only mods you made or may give away.",
    "Nothing copied from a game disc: a mod made from your own disc stays with you.",
    "Each goes up as a pull request under your Hugging Face account, listed at once.",
    "Anyone can report a mod, and take-downs go through the store's Community page.",
];

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
    /// P199: the player's own mods and the account, read again (the Upload page opened).
    Locals,
    /// Open Hugging Face's page that makes a write token.
    Tokens,
    /// Sign in with the token on the clipboard.
    Paste,
    SignOut,
    /// Upload these local mods (by id), in turn.
    Upload(Vec<String>),
    /// My Mods' listing.
    Mine,
    /// Take the player's own mod down (by key).
    TakeDown(String),
}

/// How an upload or take-down stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Busy,
    Done,
    Failed,
}

/// One of the player's own mods on the Upload page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Upload {
    pub id: String,
    pub name: String,
    /// Its kind's tab name ([`STORE_TABS`]).
    pub kind: String,
    /// Why it cannot go up (made from a game disc, …), if it cannot.
    pub refused: Option<String>,
    /// Its upload's state and line, once asked.
    pub status: Option<(Phase, String)>,
}

/// One of the player's mods on the store, on My Mods.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Posted {
    /// The store's key ([`Card::key`]).
    pub key: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub verified: bool,
    /// The pull request it is on (None: on `main`).
    pub pr: Option<u32>,
    /// A take-down under way.
    pub busy: bool,
}

/// A row of the Upload page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpRow {
    /// Open Hugging Face's page that makes a token.
    Tokens,
    /// Sign in with the token on the clipboard.
    Paste,
    SignOut,
    All,
    /// [`Shop::locals`]' mod.
    Mod(usize),
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
    /// P199: the signed-in Hugging Face account.
    pub account: Option<String>,
    /// A pasted token being checked.
    pub signing: bool,
    /// The rules were agreed to this session.
    pub agreed: bool,
    /// The player's own mods, for Upload.
    pub locals: Vec<Upload>,
    /// The browser's build: no mods of the player's own to upload.
    pub web: bool,
    /// Their mods on the store (My Mods), and where that listing is.
    pub mine: Vec<Posted>,
    pub mine_listed: Listed,
    /// A take-down asked once, by key: Cross again takes it down.
    pub arm: Option<String>,
    asks: Vec<StoreAsk>,
}

impl Shop {
    /// The cards on the tab, as indices into [`Shop::cards`].
    pub fn shown(&self) -> Vec<usize> {
        (0..self.cards.len()).filter(|&i| self.cards[i].tab == self.tab).collect()
    }

    /// The Upload page's rows (none until the rules are agreed to).
    pub fn upload_rows(&self) -> Vec<UpRow> {
        match (self.agreed, &self.account) {
            (false, _) => Vec::new(),
            (true, None) => vec![UpRow::Tokens, UpRow::Paste],
            (true, Some(_)) => [UpRow::SignOut, UpRow::All].into_iter().chain((0..self.locals.len()).map(UpRow::Mod)).collect(),
        }
    }

    /// The mods Upload all sends: every one that may go up and is not going up now.
    pub fn uploadable(&self) -> Vec<usize> {
        (0..self.locals.len()).filter(|&i| self.locals[i].refused.is_none() && !matches!(self.locals[i].status, Some((Phase::Busy, _)))).collect()
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

    /// P199: Mods' Upload to the Store: the store screen on its Upload page.
    pub(super) fn open_store_upload(&mut self) {
        self.push(Screen::Store);
        self.store_tab(UPLOAD_TAB);
    }

    /// The store screen on `tab`, the cursor home: the Upload page reads the player's mods again, and
    /// My Mods their mods on the store.
    pub fn store_tab(&mut self, tab: usize) {
        self.store.tab = tab;
        self.store.arm = None;
        self.cursor[Screen::Store.index()] = 0;
        if self.store.off {
            return;
        }
        if tab == UPLOAD_TAB {
            self.store.asks.push(StoreAsk::Locals);
        } else if tab == MINE_TAB && self.store.account.is_some() {
            self.store.mine_listed = Listed::Asking;
            self.store.asks.push(StoreAsk::Mine);
        }
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
            let n = STORE_TABS.len() + STORE_PAGES.len();
            let tab = (self.store.tab + if p.l1 { n - 1 } else { 1 }) % n;
            self.store_tab(tab);
            return;
        }
        if self.store.tab >= UPLOAD_TAB {
            self.walk_store_page(p);
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

    /// [`Menu::walk_store`]'s Upload and My Mods pages.
    fn walk_store_page(&mut self, p: Press) {
        let at = Screen::Store.index();
        // Triangle is also back, and the grid's Report: here it does neither
        if p.triangle {
            return;
        }
        if p.back {
            self.pop();
            return;
        }
        if self.store.off {
            return;
        }
        let shop = &mut self.store;
        if shop.tab == UPLOAD_TAB {
            if !shop.agreed {
                if p.ok {
                    shop.agreed = true;
                    self.cursor[at] = 0;
                }
                return;
            }
            if p.start {
                shop.asks.push(StoreAsk::Locals);
                return;
            }
            let rows = shop.upload_rows();
            let c = step_row(self.cursor[at], rows.len(), p);
            self.cursor[at] = c;
            let shop = &mut self.store;
            if !p.ok {
                return;
            }
            match rows.get(c) {
                Some(UpRow::Tokens) => {
                    shop.asks.push(StoreAsk::Tokens);
                    shop.note = Some("Make a token with write access, copy it, then Paste token".into());
                }
                Some(UpRow::Paste) if !shop.signing => {
                    shop.signing = true;
                    shop.note = Some("Checking the token with Hugging Face…".into());
                    shop.asks.push(StoreAsk::Paste);
                }
                Some(UpRow::SignOut) => {
                    shop.account = None;
                    shop.mine.clear();
                    shop.asks.push(StoreAsk::SignOut);
                    shop.note = Some("Signed out: the token is forgotten".into());
                    self.cursor[at] = 0;
                }
                Some(UpRow::All) => {
                    let all = shop.uploadable();
                    if all.is_empty() {
                        shop.note = Some("Nothing to upload: none of your mods can go up now".into());
                        return;
                    }
                    for &i in &all {
                        shop.locals[i].status = Some((Phase::Busy, "Waiting to upload".into()));
                    }
                    shop.asks.push(StoreAsk::Upload(all.iter().map(|&i| shop.locals[i].id.clone()).collect()));
                }
                Some(UpRow::Mod(i)) => {
                    let l = &mut shop.locals[*i];
                    if let Some(why) = &l.refused {
                        shop.note = Some(format!("{} can't go up: {why}", l.name));
                    } else if !matches!(l.status, Some((Phase::Busy, _))) {
                        l.status = Some((Phase::Busy, "Waiting to upload".into()));
                        shop.asks.push(StoreAsk::Upload(vec![l.id.clone()]));
                    }
                }
                _ => {}
            }
            return;
        }
        // My Mods
        if shop.account.is_none() {
            return;
        }
        if p.start {
            shop.mine_listed = Listed::Asking;
            shop.arm = None;
            shop.asks.push(StoreAsk::Mine);
            return;
        }
        let before = self.cursor[at];
        let c = step_row(before, self.store.mine.len(), p);
        self.cursor[at] = c;
        let shop = &mut self.store;
        if c != before {
            shop.arm = None;
        }
        let Some(m) = shop.mine.get_mut(c) else { return };
        if !p.ok || m.busy {
            return;
        }
        if shop.arm.as_deref() == Some(m.key.as_str()) {
            m.busy = true;
            shop.arm = None;
            shop.note = Some(format!("Taking {} down…", m.name));
            shop.asks.push(StoreAsk::TakeDown(m.key.clone()));
        } else {
            shop.arm = Some(m.key.clone());
            shop.note = Some(format!("Press Cross again to take {} off the store", m.name));
        }
    }
}

/// A list's cursor `c` of `n` rows after Up or Down.
fn step_row(c: usize, n: usize, p: Press) -> usize {
    let c = c.min(n.saturating_sub(1));
    if p.up {
        c.saturating_sub(1)
    } else if p.down {
        (c + 1).min(n.saturating_sub(1))
    } else {
        c
    }
}
