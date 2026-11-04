//! P293, opentennis's M136, M141 and B601 (`mods_screen/transfers.rs`): the mod store's downloads
//! and uploads under way, shown on every menu screen and not only the store's: a pill at the top
//! middle, a part a direction (the mod going now, its place in the batch, a bar for the whole
//! batch). Once the last download of a batch that installed something is in and no upload is still
//! going, the Reload / Later popup on whichever menu screen is up, or on the next calm one; then,
//! while the new mods wait, a Reload Mods button (Select) in the pill's place, and Free Ride, Local
//! Game and Online ask again (Reload now / Play without them) if the popup was put off. Nothing asks
//! while anything is queued or going.
//!
//! A reload here is the shell reading the mods folder again ([`StoreAsk::Reload`]): the pick's
//! tracks, the music and trick packs. Opentennis restarts for it; openMX read the folder after each
//! install, and now waits for the batch's end and the player's word, as opentennis does.

use super::*;

/// The popup's two rows, and the Play rows' ask's.
pub const RELOAD_ROWS: [&str; 2] = ["Reload", "Later"];
pub const PLAY_ROWS: [&str; 2] = ["Reload now", "Play without them"];
/// What the popup asks.
pub const RELOAD_ASK: &str = "Load the new mods?";
/// What a store card's bar says while it waits its turn (`modstore::Store::install`'s line too).
const WAITING: &str = "Waiting";

/// The batches and the reload's state, kept in [`Shop`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transfers {
    /// The installs asked since none was last going (the strip's "of 5").
    all: usize,
    /// The uploads asked since none was last going.
    up_all: usize,
    /// An install of this batch went in.
    fresh: bool,
    /// A batch that installed something ended: the popup comes up once the screen is calm.
    due: bool,
    /// Mods were installed since the shell last read the folder ([`Menu::mods_reloaded`]).
    changed: bool,
    /// Play without them was chosen since they changed: Play asks no more.
    without: bool,
    /// The popup, while it is up: its row, and the main menu's row that asked it (a Play row's ask).
    ask: Option<(usize, Option<usize>)>,
}

/// One direction's batch: its verb, the mod going now, how many are done, under way (side by side)
/// and in all, and the share of the batch done (the ones going counted by their own bars).
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub verb: &'static str,
    pub now: String,
    pub done: usize,
    pub going: usize,
    pub all: usize,
    pub share: f32,
}

impl Strip {
    /// "Downloading 3/5: Nat 01" (the one going now is the batch's 3rd of 5); "1-3/5: Nat 01 +2"
    /// with three side by side.
    pub fn line(&self) -> String {
        let (from, to) = ((self.done + 1).min(self.all), (self.done + self.going).clamp(1, self.all.max(1)));
        let place = if to > from { format!("{from}-{to}/{}", self.all) } else { format!("{from}/{}", self.all) };
        format!("{} {place}: {}", self.verb, self.now)
    }
}

/// "Nat 01" with one going, "Nat 01 +2" with three.
fn named(first: &str, going: usize) -> String {
    if going > 1 { format!("{first} +{}", going - 1) } else { first.to_string() }
}

impl Shop {
    /// The cards queued or installing, as indices into [`Shop::cards`].
    fn in_flight(&self) -> Vec<usize> {
        (0..self.cards.len()).filter(|&i| self.cards[i].progress.is_some()).collect()
    }

    /// The player's mods queued or going up.
    fn ups_in_flight(&self) -> Vec<usize> {
        (0..self.locals.len()).filter(|&i| matches!(self.locals[i].status, Some((Phase::Busy, _)))).collect()
    }

    /// `n` installs asked (their bars up already): a new batch if they are all there is.
    pub(super) fn queued(&mut self, n: usize) {
        if self.in_flight().len() == n {
            self.t.all = 0;
        }
        self.t.all += n;
    }

    /// `n` uploads asked (their lines up already): a new batch if they are all there is.
    pub(super) fn up_queued(&mut self, n: usize) {
        if self.ups_in_flight().len() == n {
            self.t.up_all = 0;
        }
        self.t.up_all += n;
    }

    /// The downloads' strip, while any is queued or under way: the first one going named (else the
    /// first queued), the ones waiting their turn not counted as going.
    fn downloading(&self) -> Option<Strip> {
        let flight = self.in_flight();
        let first = *flight.first()?;
        let bar = |i: usize| self.cards[i].progress.as_ref();
        let going: Vec<usize> = flight.iter().copied().filter(|&i| bar(i).is_some_and(|(_, line)| !line.starts_with(WAITING))).collect();
        let all = self.t.all.max(flight.len());
        let done = all - flight.len();
        let share: f32 = flight.iter().filter_map(|&i| bar(i).map(|b| b.0.clamp(0.0, 1.0))).sum();
        let now = &self.cards[going.first().copied().unwrap_or(first)].name;
        Some(Strip { verb: "Downloading", now: named(now, going.len()), done, going: going.len(), all, share: (done as f32 + share) / all.max(1) as f32 })
    }

    /// The uploads' strip, while any is queued or going: one going counts half done (packed or
    /// packing, not sent yet), as opentennis's B601 has it.
    fn uploading(&self) -> Option<Strip> {
        let flight = self.ups_in_flight();
        let first = *flight.first()?;
        let line = |i: usize| self.locals[i].status.as_ref().map_or("", |s| s.1.as_str());
        let going: Vec<usize> = flight.iter().copied().filter(|&i| !line(i).starts_with(WAITING)).collect();
        let all = self.t.up_all.max(flight.len());
        let done = all - flight.len();
        let now = match going.last() {
            Some(&i) => named(&self.locals[i].name, going.len()),
            None if flight.len() == 1 => self.locals[first].name.clone(),
            None => "waiting its turn".into(),
        };
        Some(Strip { verb: "Uploading", now, done, going: going.len(), all, share: (done as f32 + 0.5 * going.len() as f32) / all.max(1) as f32 })
    }

    /// Both directions' strips, downloads first: the pill's.
    pub fn strips(&self) -> Vec<Strip> {
        self.downloading().into_iter().chain(self.uploading()).collect()
    }

    /// No download queued or going and no upload: the mods are as they will stay, so Reload may be
    /// offered.
    pub fn idle(&self) -> bool {
        self.in_flight().is_empty() && self.ups_in_flight().is_empty()
    }
}

impl Menu {
    /// The shell's word that the store's install of `key` ended, `ok` if it went in: the card's bar
    /// goes, and the batch's last one in brings the popup ([`Menu::offer_reload`]).
    pub fn store_installed(&mut self, key: &str, ok: bool) {
        let shop = &mut self.store;
        if let Some(c) = shop.cards.iter_mut().find(|c| c.key == key) {
            c.progress = None;
        }
        if ok {
            (shop.t.fresh, shop.t.changed, shop.t.without) = (true, true, false);
        }
        if shop.in_flight().is_empty() && std::mem::take(&mut shop.t.fresh) {
            shop.t.due = true;
        }
    }

    /// The shell read the mods folder again (a Reload, a removal, the Mods switch): nothing waits.
    pub fn mods_reloaded(&mut self) {
        let t = &mut self.store.t;
        (t.changed, t.due, t.without) = (false, false, false);
        if t.ask.is_some_and(|(_, play)| play.is_none()) {
            t.ask = None;
        }
    }

    /// Whether installed mods wait for a reload.
    pub fn mods_waiting(&self) -> bool {
        self.store.t.changed
    }

    /// A screen the popup may come up over: a front-end menu page with nothing else asking, not a
    /// pick, a lobby, a join, a keyboard, nor anything over a ride.
    fn calm(&self) -> bool {
        use Screen::*;
        let screen = matches!(self.screen(), Some(Main | Online | Settings | Mods | Modifiers | Original | Store));
        screen
            && !self.stack.contains(&Pause)
            && self.loading.is_none()
            && self.over.is_none()
            && self.listing.is_none()
            && self.quit.is_none()
            && self.leaving.is_none()
            && self.store.arm.is_none()
            && self.store.t.ask.is_none()
    }

    /// Each frame: a batch that installed something ended and nothing is going: the popup comes up
    /// once the screen is calm.
    pub(super) fn offer_reload(&mut self) {
        if self.store.t.due && self.store.idle() && self.calm() {
            self.store.t.due = false;
            self.store.t.ask = Some((0, None));
        }
    }

    /// The Reload Mods button is up: everything done, mods waiting, the popup down, a calm screen
    /// but the store's, whose Select is Download All.
    pub fn reload_button(&self) -> bool {
        self.store.t.changed && self.store.idle() && self.calm() && self.screen() != Some(Screen::Store)
    }

    /// The popup, if it is up: its question, its rows, and the row it is on.
    pub fn reload_asking(&self) -> Option<(&'static str, [&'static str; 2], usize)> {
        let (row, play) = self.store.t.ask?;
        self.screen()?;
        Some((RELOAD_ASK, if play.is_some() { PLAY_ROWS } else { RELOAD_ROWS }, row))
    }

    /// The new mods read in: the shell is asked to read the folder again.
    fn reload_mods(&mut self) {
        self.store.asks.push(StoreAsk::Reload);
        self.mods_reloaded();
        self.note = Some("Mods reloaded".into());
    }

    /// The main menu's Free Ride, Local Game or Online (`row`) with new mods waiting and nothing
    /// going: unless Play without them was chosen since they changed, the popup asks first. Whether
    /// it asked.
    pub(super) fn play_ask(&mut self, row: usize) -> bool {
        let t = &self.store.t;
        if !t.changed || t.without || t.ask.is_some() || !self.store.idle() {
            return false;
        }
        self.store.t.ask = Some((0, Some(row)));
        true
    }

    /// One press on the popup: any direction moves between its rows, Cross on Reload reads the mods
    /// in, Cross on Later (or Circle) puts it off. A Play row's ask then carries on to that row's
    /// screen, but for Circle, which stays.
    pub(super) fn answer_reload(&mut self, p: Press) -> Act {
        let Some((row, play)) = self.store.t.ask else { return Act::Stay };
        if p.up || p.down || p.left || p.right {
            self.store.t.ask = Some((1 - row, play));
            return Act::Stay;
        }
        if !(p.ok || p.back) {
            return Act::Stay;
        }
        self.store.t.ask = None;
        if p.back {
            return Act::Stay;
        }
        if row == 0 {
            self.reload_mods();
        } else if play.is_some() {
            self.store.t.without = true;
        }
        match play {
            Some(at) if self.screen() == Some(Screen::Main) => {
                self.cursor[Screen::Main.index()] = at;
                self.walk(Press { ok: true, ..Press::default() })
            }
            _ => Act::Stay,
        }
    }

    /// [`Menu::walk`]'s part before the screens': the popup takes every press while it is up, Select
    /// presses the Reload Mods button, and a finished batch's popup comes up. `Some` when it took
    /// the press.
    pub(super) fn walk_transfers(&mut self, p: Press) -> Option<Act> {
        if self.store.t.ask.is_some() {
            return Some(self.answer_reload(p));
        }
        if p.select && self.reload_button() {
            self.reload_mods();
            return Some(Act::Stay);
        }
        self.offer_reload();
        None
    }

    /// `OPENMX_TRANSFERS` (for `--shot`): `1` a made-up batch of each under way, over whatever
    /// `--screen`; `ask` the batch done and the popup up; `done` it put off (the Reload Mods button);
    /// `play` Free Ride's ask.
    pub fn transfers_sample(&mut self) {
        let Some(how) = std::env::var_os("OPENMX_TRANSFERS") else { return };
        let t = &mut self.store.t;
        match how.to_str() {
            Some("ask") => (t.changed, t.ask) = (true, Some((0, None))),
            Some("done") => t.changed = true,
            Some("play") => (t.changed, t.ask) = (true, Some((0, Some(MAIN_FREE)))),
            _ => {
                let card = |key: &str, name: &str, progress: Option<(f32, &str)>| Card {
                    key: key.into(),
                    name: name.into(),
                    progress: progress.map(|(s, l)| (s, l.to_string())),
                    ..Card::default()
                };
                self.store.cards.extend([
                    card("sample/nat01", "Nat 01", Some((0.4, "Downloading 12 / 30 MB"))),
                    card("sample/nat02", "Nat 02", Some((0.0, "Waiting to download Nat 02"))),
                    card("sample/sx03", "SX 03", Some((0.0, "Waiting to download SX 03"))),
                ]);
                self.store.t.all = 5;
                let up = |id: &str, line: &str| Upload { id: id.into(), name: id.into(), kind: "Tracks".into(), status: Some((Phase::Busy, line.into())) };
                self.store.locals.extend([up("my_track", "Packing my_track"), up("my_tricks", "Waiting to upload")]);
                self.store.t.up_all = 3;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str) -> Card {
        Card { key: key.into(), name: key.to_uppercase(), ..Card::default() }
    }

    fn ask(m: &mut Menu, keys: &[&str]) {
        for k in keys {
            if !m.store.cards.iter().any(|c| c.key == *k) {
                m.store.cards.push(card(k));
            }
            m.store.cards.iter_mut().find(|c| c.key == *k).unwrap().progress = Some((0.0, "Waiting to download".into()));
        }
        m.store.queued(keys.len());
    }

    fn going(m: &mut Menu, key: &str, share: f32) {
        m.store.cards.iter_mut().find(|c| c.key == key).unwrap().progress = Some((share, "Downloading 1 / 2 MB".into()));
    }

    #[test]
    fn a_download_batch_counts_down() {
        let mut m = Menu::new(Settings::default(), Controls::default());
        assert!(m.store.strips().is_empty());
        ask(&mut m, &["x", "y", "z"]);
        going(&mut m, "x", 0.5);
        let s = &m.store.strips()[0];
        assert_eq!((s.line().as_str(), s.done, s.all), ("Downloading 1/3: X", 0, 3));
        assert!((s.share - 0.5 / 3.0).abs() < 1e-6, "{}", s.share);
        m.store_installed("x", true);
        going(&mut m, "y", 0.0);
        let s = &m.store.strips()[0];
        assert_eq!(s.line(), "Downloading 2/3: Y");
        // side by side
        going(&mut m, "z", 0.1);
        assert_eq!(m.store.strips()[0].line(), "Downloading 2-3/3: Y +1");
        // one more asked mid-batch joins it
        ask(&mut m, &["w"]);
        assert_eq!(m.store.strips()[0].all, 4);
        for k in ["y", "z", "w"] {
            m.store_installed(k, true);
        }
        assert!(m.store.strips().is_empty() && m.store.idle());
        // the next batch counts from its own start
        ask(&mut m, &["v"]);
        assert_eq!(m.store.strips()[0].line(), "Downloading 1/1: V");
    }

    #[test]
    fn an_upload_batch_counts_down() {
        let mut m = Menu::new(Settings::default(), Controls::default());
        let up = |id: &str, line: &str| Upload { id: id.into(), name: id.into(), kind: "Tracks".into(), status: Some((Phase::Busy, line.into())) };
        m.store.locals = vec![up("a", "Packing a"), up("b", "Waiting to upload")];
        m.store.up_queued(2);
        let s = &m.store.strips()[0];
        assert_eq!((s.line().as_str(), s.share), ("Uploading 1/2: a", 0.25));
        m.store.locals[0].status = Some((Phase::Done, "Uploaded a".into()));
        m.store.locals[1].status = Some((Phase::Busy, "Packing b".into()));
        assert_eq!(m.store.strips()[0].line(), "Uploading 2/2: b");
        m.store.locals[1].status = Some((Phase::Failed, "no".into()));
        assert!(m.store.strips().is_empty());
    }

    #[test]
    fn a_failed_batch_does_not_ask() {
        let mut m = Menu::new(Settings::default(), Controls::default());
        m.open(Screen::Main);
        ask(&mut m, &["x"]);
        m.store_installed("x", false);
        m.offer_reload();
        assert_eq!((m.reload_asking(), m.mods_waiting()), (None, false));
    }
}
