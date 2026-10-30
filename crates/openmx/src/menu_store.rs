//! The mod store's screen (P198): Mods' Browse the Store, a picture grid of the store's mods under a
//! tab a kind (B372/B373), each card its name, author, source game, size, Verified, and Installed or
//! Update. Cross installs (or updates) the lit card, Square removes it, Triangle reports it, Start
//! reads the store again, Select downloads every mod on the tab not installed (or out of date), L1/R1
//! change the tab. MD66: on the Tracks tab, Up from the grid's top row lights its category filter
//! (Freestyle, Supercross, …, Other, from each mod's `meta.json`), which Left and Right step and Down
//! leaves. P230: Texture Packs is a kind tab too, and its Pack Order page lists every pack in
//! the mods folder top first: Cross turns the lit one on or off, Left and Right move it up and down,
//! written at once to settings.txt's `texture_packs` / `texture_packs_off`.
//!
//! The menu knows the store only as this plain view, which the shell fills from `modstore.rs` and
//! whose asks it takes ([`Menu::take_store_asks`]), so the menu's tests mount nothing of the store.

use super::*;

/// The grid's tabs: `modstore::TABS`.
pub const STORE_TABS: [&str; 4] = ["Tracks", "Trick Packs", "Music", "Texture Packs"];
/// P230: the Texture Packs tab.
pub const TEXTURES_TAB: usize = 3;
/// The grid's columns.
pub const STORE_COLUMNS: usize = 4;
/// P199: the store's own pages after the kind tabs: Upload my mod (and Upload all), and My public mods;
/// P230: first, the texture packs' order and switches.
pub const STORE_PAGES: [&str; 3] = ["Pack Order", "Upload", "My Mods"];
/// Their tabs.
pub const PACKS_TAB: usize = STORE_TABS.len();
pub const UPLOAD_TAB: usize = STORE_TABS.len() + 1;
pub const MINE_TAB: usize = STORE_TABS.len() + 2;
/// P230: what a texture pack is online, said on its tabs.
pub const PACKS_ONLINE: &str = "Online: only you see a texture pack; the lobby doesn't share it";
/// What a player agrees to before they upload (`plan/P198.md` § P199): shown until agreed to, once ever
/// ([`Settings::store_rules`]).
pub const STORE_RULES: [&str; 4] = [
    "Uploads go on the public openMX mod store, for anyone to download.",
    "You are responsible for what you share: only mods you made or may give away.",
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
    /// MD66: its category (`modstore::Meta::category_word`: Other where its `meta.json` names
    /// none), which the Tracks tab filters by.
    pub category: String,
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
    /// P230: the texture packs in the mods folder, read again (Pack Order opened).
    Packs,
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

/// P230: one texture pack on Pack Order (`textures::Listed`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackRow {
    /// Its folder under `mods/`, as `texture_packs` names it.
    pub folder: String,
    pub name: String,
    pub textures: usize,
    pub on: bool,
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
    /// MD66: the Tracks tab's category filter, a [`Shop::categories`] name; empty for All
    /// Categories.
    pub category: String,
    /// MD66: the cursor is on that filter, above the grid.
    pub filtering: bool,
    /// Whether there is a store at all: `--mods off` has no folder to install into.
    pub off: bool,
    /// P199: the signed-in Hugging Face account.
    pub account: Option<String>,
    /// A pasted token being checked.
    pub signing: bool,
    /// The rules were agreed to ([`Settings::store_rules`], kept in `settings.txt`).
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
    /// P230: the texture packs, top first, as the shell read them for Pack Order.
    pub packs: Vec<PackRow>,
    pub(super) asks: Vec<StoreAsk>,
}

impl Shop {
    /// The cards on the tab, as indices into [`Shop::cards`]: on Tracks, only the lit category's
    /// (MD66).
    pub fn shown(&self) -> Vec<usize> {
        let want = (self.tab == 0 && self.category_group() > 0).then_some(self.category.as_str());
        (0..self.cards.len()).filter(|&i| self.cards[i].tab == self.tab && want.is_none_or(|w| self.card_category(i) == w)).collect()
    }

    /// MD66: a card's category, Other where it has none.
    fn card_category(&self, i: usize) -> &str {
        let c = self.cards[i].category.trim();
        if c.is_empty() { OTHER } else { c }
    }

    /// MD66: the Tracks tab's category filters: All Categories, then each category a track card is
    /// in, in the pick's order ([`CATEGORIES`], any other by name, Other last). All Categories alone
    /// where they are all one.
    pub fn categories(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for i in (0..self.cards.len()).filter(|&i| self.cards[i].tab == 0) {
            let c = self.card_category(i).to_string();
            if !seen.contains(&c) {
                seen.push(c);
            }
        }
        seen.sort_by_cached_key(|c| (c == OTHER, CATEGORIES.iter().position(|k| k == c).unwrap_or(CATEGORIES.len()), c.clone()));
        if seen.len() < 2 {
            return vec![ALL_CATEGORIES.to_string()];
        }
        seen.insert(0, ALL_CATEGORIES.to_string());
        seen
    }

    /// MD66: which of [`Shop::categories`] the filter is, All Categories where no card is in it.
    pub fn category_group(&self) -> usize {
        if self.category.is_empty() {
            return 0;
        }
        self.categories().iter().position(|c| *c == self.category).unwrap_or(0)
    }

    /// MD66: whether the tab has the category filter: Tracks, with cards of two categories or more.
    pub fn filtered(&self) -> bool {
        self.tab == 0 && self.categories().len() > 1
    }

    /// The cards on the tab Download All installs: not installed or out of date, and not going now.
    pub fn downloadable(&self) -> Vec<usize> {
        self.shown().into_iter().filter(|&i| self.cards[i].progress.is_none() && matches!(self.cards[i].got, Got::No | Got::Update)).collect()
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
        (0..self.locals.len()).filter(|&i| !matches!(self.locals[i].status, Some((Phase::Busy, _)))).collect()
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
        self.store.filtering = false;
        self.cursor[Screen::Store.index()] = 0;
        if self.store.off {
            return;
        }
        if tab == PACKS_TAB {
            self.store.asks.push(StoreAsk::Packs);
        } else if tab == UPLOAD_TAB {
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
        if self.store.filtering && self.store.filtered() {
            return None;
        }
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
        if self.store.tab >= PACKS_TAB {
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
        if p.select && !self.store.off {
            let all = self.store.downloadable();
            self.store.note = Some(match all.len() {
                0 => format!("Every one of the {} is installed already", STORE_TABS[self.store.tab].to_lowercase()),
                n => format!("Downloading {n} of the {}", STORE_TABS[self.store.tab].to_lowercase()),
            });
            for i in all {
                let card = &mut self.store.cards[i];
                card.progress = Some((0.0, "Waiting to download".into()));
                self.store.asks.push(StoreAsk::Install(card.key.clone()));
            }
            return;
        }
        // MD66: the category filter, above the grid: Left and Right step it, Down goes back to the
        // grid's first card.
        if self.store.filtering && self.store.filtered() {
            if p.left || p.right {
                let cats = self.store.categories();
                let (n, c) = (cats.len(), self.store.category_group());
                let name = &cats[if p.left { (c + n - 1) % n } else { (c + 1) % n }];
                self.store.category = if name == ALL_CATEGORIES { String::new() } else { name.clone() };
                self.cursor[at] = 0;
            } else if p.down {
                self.store.filtering = false;
            }
            return;
        }
        self.store.filtering = false;
        let n = self.store.shown().len();
        if (p.up && self.cursor[at].min(n.saturating_sub(1)) < STORE_COLUMNS || n == 0 && p.up) && self.store.filtered() {
            self.store.filtering = true;
            return;
        }
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
        if self.store.tab == PACKS_TAB {
            self.walk_packs(p);
            return;
        }
        let shop = &mut self.store;
        if shop.tab == UPLOAD_TAB {
            if !shop.agreed {
                if p.ok {
                    shop.agreed = true;
                    self.settings.store_rules = true;
                    self.changed = true;
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
                    if !matches!(l.status, Some((Phase::Busy, _))) {
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

impl Menu {
    /// P230: Pack Order: Up and Down light a pack, Cross turns it on or off, Left and Right move it up
    /// and down the stack (the cursor with it), Start reads the folder again. A change is written to
    /// the settings at once ([`Menu::save_packs`]) and drawn from the next ride.
    fn walk_packs(&mut self, p: Press) {
        let at = Screen::Store.index();
        if p.start {
            self.store.asks.push(StoreAsk::Packs);
            return;
        }
        let n = self.store.packs.len();
        let c = step_row(self.cursor[at], n, p);
        self.cursor[at] = c;
        if n == 0 {
            return;
        }
        let packs = &mut self.store.packs;
        if p.ok {
            packs[c].on = !packs[c].on;
            self.store.note = Some(format!("{} is {}: from the next ride", packs[c].name, if packs[c].on { "on" } else { "off" }));
        } else if p.left && c > 0 {
            packs.swap(c, c - 1);
            self.cursor[at] = c - 1;
        } else if p.right && c + 1 < n {
            packs.swap(c, c + 1);
            self.cursor[at] = c + 1;
        } else {
            return;
        }
        self.save_packs();
    }

    /// P230: Pack Order's list as settings.txt's lines: every pack in `texture_packs`, top first (so a
    /// pack installed later, in neither, goes on top), and the ones off in `texture_packs_off`.
    pub fn save_packs(&mut self) {
        self.settings.texture_packs = self.store.packs.iter().map(|r| r.folder.clone()).collect();
        self.settings.texture_packs_off = self.store.packs.iter().filter(|r| !r.on).map(|r| r.folder.clone()).collect();
        self.changed = true;
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
