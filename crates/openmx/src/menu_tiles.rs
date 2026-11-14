//! P341's machine tiles: the Garage's Machines tab and the pause's Garage page show a player's
//! machines as a grid of cream tiles under Bikes, Fliers and Quads & cars, each with its picture
//! baked off the Garage's stage. L1 and R1 step the lit tile's livery; on the pause's page Cross
//! switches to it and Triangle, there and on the pause's list, to the last machine ridden.

use super::pages::{Key, Kind, Row};
use super::*;

/// The grid's sections, in order.
pub const SECTIONS: [&str; 3] = ["Bikes", "Fliers", "Quads & cars"];
/// Tiles across the grid, their gap and their height, in stage ems.
pub(super) const TILE_COLS: usize = 4;
pub(super) const TILE_GAP: f32 = 0.5;
pub(super) const TILE_H: f32 = 6.8;

/// What a tile picks.
#[derive(Clone, Debug, PartialEq)]
pub enum TileOf {
    /// A disc bike model in one class: its paints' archives, which the liveries name.
    Bike(Vec<String>),
    /// A flier kind, by its pack word: its skins are the liveries.
    Flier(String),
    /// The quad (MD37d): Off, then the handling files the installed quad pack rides.
    Quad(Vec<String>),
    /// What openmx cannot ride, listed greyed with why.
    Locked,
    /// A machine of the ride under the pause, which Cross switches to.
    Switch(Machine),
}

/// One tile.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    /// Its place in [`SECTIONS`].
    pub section: usize,
    pub name: String,
    /// The line over the name: the class, or the game a pack's comes from.
    pub class: String,
    /// The baked picture's key ([`Picture::key`]), where it has one.
    pub picture: Option<String>,
    /// Its liveries, each with why it cannot be picked where it cannot.
    pub liveries: Vec<(String, Option<String>)>,
    pub livery: usize,
    /// The machine the player rides: tagged RIDING.
    pub riding: bool,
    /// Why it cannot be ridden: greyed, with a lock.
    pub locked: Option<String>,
    pub of: TileOf,
}

/// A tile's picture as the shell bakes it off the Garage's stage.
#[derive(Clone, Debug, PartialEq)]
pub enum Picture {
    /// A bike, ridden by the seat's rider.
    Bike(Pick),
    Flier(roster::Flier),
}

impl Picture {
    /// The picture's key: `bike:<archive>`, `flier:<pack>/<skin>`.
    pub fn key(&self) -> String {
        match self {
            Picture::Bike(p) => bike_key(&p.bike),
            Picture::Flier(f) => flier_key(f),
        }
    }
}

fn bike_key(archive: &str) -> String {
    format!("bike:{}", archive.to_ascii_lowercase())
}

fn flier_key(f: &roster::Flier) -> String {
    format!("flier:{}/{}", f.pack, f.skin)
}

/// Where a pause tile's machine sits: the bike, the fliers, the quad and the cars.
fn section_of(m: Machine) -> usize {
    match m {
        Machine::Bike => 0,
        Machine::Biplane | Machine::Helicopter => 1,
        _ => 2,
    }
}

/// A flier machine's pack word.
fn flier_word(m: Machine) -> Option<&'static str> {
    match m {
        Machine::Biplane => Some("biplane"),
        Machine::Helicopter => Some("kiowa"),
        _ => None,
    }
}

/// Every place the grid lays a row of `rows` in a column `x` to `x + w`, from 0 down, unscrolled:
/// a tile row is a tile in a line of [`TILE_COLS`], any other row a line of its own.
pub(super) fn tile_layout(rows: &[Row], x: f32, w: f32) -> (Vec<(usize, [f32; 4])>, f32) {
    use super::screens::{HEAD_H, ROW_GAP, ROW_H};
    let tw = (w - TILE_GAP * (TILE_COLS - 1) as f32) / TILE_COLS as f32;
    let (mut y, mut col) = (0.0, 0);
    let mut out = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if r.kind == Kind::Tile {
            if col == TILE_COLS {
                (col, y) = (0, y + TILE_H + TILE_GAP);
            }
            out.push((i, [x + col as f32 * (tw + TILE_GAP), y, tw, TILE_H]));
            col += 1;
            continue;
        }
        if col > 0 {
            (col, y) = (0, y + TILE_H + ROW_GAP);
        }
        let h = if r.kind == Kind::Heading { HEAD_H } else { ROW_H };
        out.push((i, [x, y, w, h]));
        y += h + ROW_GAP;
    }
    if col > 0 {
        y += TILE_H + ROW_GAP;
    }
    (out, y)
}

/// The row a grid press moves to from `at` (P341): left and right the tile before or after it,
/// up and down the nearest row in the line above or below, the one most under it. `None` where
/// the press is not the grid's: left and right off a tile, or up and down past the grid's ends.
pub(super) fn grid_step(rows: &[Row], at: usize, p: Press) -> Option<usize> {
    let (spots, _) = tile_layout(rows, 0.0, 36.0);
    let here = spots.iter().find(|s| s.0 == at)?.1;
    let on_tile = rows.get(at).is_some_and(|r| r.kind == Kind::Tile);
    if p.left || p.right {
        if !on_tile {
            return None;
        }
        let tiles: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].kind == Kind::Tile).collect();
        let k = tiles.iter().position(|&i| i == at)?;
        let to = if p.left { k.checked_sub(1) } else { Some(k + 1).filter(|&k| k < tiles.len()) };
        return Some(to.map_or(at, |k| tiles[k]));
    }
    if !(p.up || p.down) {
        return None;
    }
    let mid = |b: &[f32; 4]| (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
    let (hx, hy) = mid(&here);
    let ahead: Vec<&(usize, [f32; 4])> = spots
        .iter()
        .filter(|(i, b)| rows[*i].selectable() && if p.up { mid(b).1 < hy - 0.1 } else { mid(b).1 > hy + 0.1 })
        .collect();
    let line = ahead.iter().map(|(_, b)| mid(b).1).min_by(|a, b| (a - hy).abs().total_cmp(&(b - hy).abs()))?;
    ahead
        .iter()
        .filter(|(_, b)| (mid(b).1 - line).abs() < 0.1)
        .min_by(|a, b| (mid(&a.1).0 - hx).abs().total_cmp(&(mid(&b.1).0 - hx).abs()))
        .map(|s| s.0)
}

/// Tiles as page rows: each section's heading and its tiles (none for an empty section).
pub(super) fn tile_rows(tiles: &[Tile]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (s, name) in SECTIONS.iter().enumerate() {
        let mut heading = true;
        for (i, t) in tiles.iter().enumerate().filter(|(_, t)| t.section == s) {
            if std::mem::take(&mut heading) {
                rows.push(Row::heading(name));
            }
            let mut r = Row::new(Key::Tile(i), Kind::Tile, &t.name, "");
            r.choices = t.liveries.iter().map(|(n, _)| n.clone()).collect();
            r.now = Some(t.livery);
            r.tips = t.liveries.iter().map(|(_, why)| why.clone().unwrap_or_default()).collect();
            r.value = r.choices.get(t.livery).cloned().unwrap_or_default();
            rows.push(r.off_if(t.locked.is_some(), t.locked.as_deref().unwrap_or_default()));
        }
    }
    rows
}

impl Menu {
    /// The Machines tab's tiles for the Garage's seat (P341): every disc bike model by its class
    /// and every machine pack's bike greyed, each flier kind with its skins, the quad, and the
    /// kinds openmx cannot ride greyed. The cars are the worlds', so not here.
    pub fn garage_tiles(&self) -> Vec<Tile> {
        let Some(g) = self.garages.get(self.garage_seat()) else { return Vec::new() };
        let (c, offer, fliers) = (&self.catalogue, &self.machine_offer, self.garage_fliers());
        let mut tiles = Vec::new();
        let ridden = c.bike(&g.pick.bike);
        for engine in roster::ENGINES {
            for model in c.models(engine) {
                let paints: Vec<&roster::Bike> = c.liveries(model).into_iter().filter(|b| b.engine == engine).collect();
                let riding = ridden.is_some_and(|b| b.model == model && b.engine == engine);
                let livery = if riding { paints.iter().position(|b| b.archive == g.pick.bike).unwrap_or(0) } else { 0 };
                tiles.push(Tile {
                    section: 0,
                    name: c.model_name(model),
                    class: roster::class_label(&roster::cc(engine).to_string()),
                    picture: paints.get(livery).map(|b| bike_key(&b.archive)),
                    liveries: paints.iter().map(|b| (c.paint_name(&b.archive), None)).collect(),
                    livery,
                    riding,
                    locked: None,
                    of: TileOf::Bike(paints.iter().map(|b| b.archive.clone()).collect()),
                });
            }
        }
        for m in offer.iter().filter(|m| m.machine == roster::BIKE_KIND) {
            tiles.push(Tile {
                section: 0,
                name: m.name.clone(),
                class: format!("{} {}", m.game, roster::class_label(&m.class)).trim().to_string(),
                picture: None,
                liveries: vec![(m.name.clone(), m.why.clone())],
                livery: 0,
                riding: false,
                locked: Some(m.why.clone().unwrap_or_else(|| "openmx rides the disc's bikes".into())),
                of: TileOf::Locked,
            });
        }
        for kind in roster::flier_kinds(fliers) {
            let choices = roster::flier_choices(fliers, &kind);
            let shown = roster::flier_of(fliers, &g.pick, &kind);
            let why = choices.iter().find_map(|f| f.why.clone()).unwrap_or_default();
            tiles.push(Tile {
                section: 1,
                name: roster::flier_label(&kind),
                class: shown.map_or_else(String::new, |f| f.game.clone()),
                picture: shown.or(choices.first().copied()).map(flier_key),
                liveries: choices.iter().map(|f| (f.name.clone(), f.why.clone())).collect(),
                livery: shown.and_then(|s| choices.iter().position(|c| std::ptr::eq(*c, s))).unwrap_or(0),
                riding: false,
                locked: shown.is_none().then_some(why),
                of: TileOf::Flier(kind),
            });
        }
        for kind in roster::other_kinds(offer, fliers) {
            let of: Vec<&roster::PackMachine> = offer.iter().filter(|m| m.machine == kind).collect();
            let game = of.first().map(|m| m.game.clone()).unwrap_or_default();
            let picks: Vec<&roster::PackMachine> = of.iter().copied().filter(|m| m.why.is_none()).collect();
            if kind == roster::ATV_KIND && !picks.is_empty() {
                let files: Vec<String> = picks.iter().map(|m| m.skin.clone()).collect();
                let livery = files.iter().position(|f| *f == self.settings.atv).map_or(0, |k| k + 1);
                tiles.push(Tile {
                    section: 2,
                    name: roster::machine_label(&kind),
                    class: game,
                    picture: None,
                    liveries: std::iter::once(("Off".to_string(), None)).chain(picks.iter().map(|m| (m.name.clone(), None))).collect(),
                    livery,
                    riding: false,
                    locked: None,
                    of: TileOf::Quad(files),
                });
                continue;
            }
            let why = of.iter().find_map(|m| m.why.clone()).unwrap_or_default();
            tiles.push(Tile {
                section: 2,
                name: roster::machine_label(&kind),
                class: game,
                picture: None,
                liveries: of.iter().map(|m| (m.name.clone(), m.why.clone())).collect(),
                livery: 0,
                riding: false,
                locked: Some(why),
                of: TileOf::Locked,
            });
        }
        tiles
    }

    /// The tile under the Machines tab's cursor, if it is on one.
    pub(super) fn garage_tile(&self) -> Option<Tile> {
        if !self.on_machines() {
            return None;
        }
        let page = self.page_of(Screen::Garage);
        let Some(Key::Tile(i)) = page.get(self.cursor(Screen::Garage)).map(|r| r.key) else { return None };
        self.garage_tiles().into_iter().nth(i)
    }

    /// The flier kind the lit tile is, where it is a flier's.
    pub(super) fn garage_tile_flier(&self) -> Option<String> {
        match self.garage_tile()?.of {
            TileOf::Flier(kind) => Some(kind),
            _ => None,
        }
    }

    /// The pick the Garage's stage shows (P341): on a bike's tile that bike in its lit paint, the
    /// seat's own rider on it; else the seat's pick.
    pub fn garage_shown(&self) -> Option<Pick> {
        let pick = self.garage_pick()?.clone();
        match self.garage_tile().map(|t| (t.of, t.livery)) {
            Some((TileOf::Bike(paints), at)) => {
                let bike = paints.get(at)?.clone();
                Some(self.catalogue.fit(&Pick { bike, ..pick }))
            }
            _ => Some(pick),
        }
    }

    /// The lit bike tile's paints either side of the shown one, for the stage to dress ahead.
    pub(super) fn tile_neighbours(&self) -> Option<Vec<Pick>> {
        let t = self.garage_tile()?;
        let TileOf::Bike(paints) = &t.of else { return None };
        let pick = self.garage_pick()?;
        let n = paints.len();
        Some(
            [1, n.saturating_sub(1)]
                .into_iter()
                .filter(|&d| n > 1 && d % n != 0)
                .map(|d| self.catalogue.fit(&Pick { bike: paints[(t.livery + d) % n].clone(), ..pick.clone() }))
                .collect(),
        )
    }

    /// L1 or R1 on the Machines tab (P341), `by` -1 or 1: the lit tile's next livery. A bike's
    /// is that bike in its next paint, picked; a flier's its next skin that can fly; the quad's
    /// its next handling, or Off. Whether anything changed.
    pub(super) fn turn_tile(&mut self, by: i32) -> bool {
        let Some(t) = self.garage_tile().filter(|t| t.locked.is_none()) else { return false };
        let seat = self.garage_seat();
        match t.of {
            TileOf::Bike(paints) if !paints.is_empty() => {
                let to = (t.livery as i64 + by as i64).rem_euclid(paints.len() as i64) as usize;
                self.pick_bike(seat, &paints[to])
            }
            TileOf::Flier(_) => self.step_garage_flier(by),
            TileOf::Quad(files) => {
                let n = files.len() + 1;
                let to = (t.livery as i64 + by as i64).rem_euclid(n as i64) as usize;
                self.settings.atv = if to == 0 { String::new() } else { files[to - 1].clone() };
                self.changed = true;
                true
            }
            _ => false,
        }
    }

    /// Cross on the Machines tab's tile: a bike's is picked in its lit paint, the quad's steps on.
    pub(super) fn take_tile(&mut self) {
        let Some(t) = self.garage_tile().filter(|t| t.locked.is_none()) else { return };
        match t.of {
            TileOf::Bike(paints) => {
                if let Some(bike) = paints.get(t.livery) {
                    self.pick_bike(self.garage_seat(), &bike.clone());
                }
            }
            TileOf::Quad(_) => {
                self.turn_tile(1);
            }
            _ => {}
        }
    }

    /// The Garage's seat rides `archive`. Whether it was another.
    fn pick_bike(&mut self, seat: usize, archive: &str) -> bool {
        let c = std::sync::Arc::clone(&self.catalogue);
        let Some(g) = self.garages.get_mut(seat) else { return false };
        let was = g.pick.clone();
        g.pick.bike = archive.to_string();
        g.pick = c.fit(&g.pick);
        let moved = g.pick != was;
        self.changed |= moved;
        moved
    }

    /// Whether the pauser picks the skin of flier `m` on the pause's Garage (P315g): the first two
    /// seats offline; online the first, where the shell said the ride's fliers (P318).
    fn own_flier(&self, m: Machine) -> Option<&'static str> {
        let online = self.session || self.opening == Opening::Open;
        if (online && (self.flier_online.is_none() || self.pauser > 0)) || self.pauser >= OWN_FLIERS {
            return None;
        }
        flier_word(m).filter(|k| !roster::flier_choices(self.fliers_here(), k).is_empty())
    }

    /// The pause's Garage tiles (P341): the machine the pauser rides and every one they can
    /// switch to ([`Menu::choices`]), by section. The bike in its paint and the cars and the quad
    /// as the ride built them; a flier's skins for a seat that picks them, greyed ones with why.
    pub fn pause_tiles(&self) -> Vec<Tile> {
        let seat = self.pauser;
        let up = self.up.get(seat).copied().unwrap_or_default();
        let mut machines = vec![up];
        machines.extend(self.choices(seat));
        machines.sort_by_key(|m| (section_of(*m), Machine::SWITCH.iter().position(|s| s == m)));
        let pick = self.picks.0.get(seat).cloned().unwrap_or_default();
        let c = &self.catalogue;
        machines
            .into_iter()
            .map(|m| {
                let mut t = Tile {
                    section: section_of(m),
                    name: m.label().to_string(),
                    class: String::new(),
                    picture: None,
                    liveries: Vec::new(),
                    livery: 0,
                    riding: m == up,
                    locked: None,
                    of: TileOf::Switch(m),
                };
                if m == Machine::Bike
                    && let Some(b) = c.bike(&pick.bike)
                {
                    t.name = c.model_name(&b.model);
                    t.class = roster::class_label(&roster::cc(b.engine).to_string());
                    t.picture = Some(bike_key(&b.archive));
                    t.liveries = vec![(c.paint_name(&b.archive), None)];
                }
                if let Some(kind) = flier_word(m) {
                    let offer = self.fliers_here();
                    let shown = roster::flier_of(offer, &pick, kind);
                    t.picture = shown.map(flier_key);
                    t.class = shown.map_or_else(String::new, |f| f.game.clone());
                    match self.own_flier(m) {
                        Some(_) => {
                            let choices = roster::flier_choices(offer, kind);
                            t.liveries = choices.iter().map(|f| (f.name.clone(), f.why.clone())).collect();
                            t.livery = shown.and_then(|s| choices.iter().position(|c| std::ptr::eq(*c, s))).unwrap_or(0);
                        }
                        None => t.liveries = shown.map(|f| vec![(f.name.clone(), None)]).unwrap_or_default(),
                    }
                }
                t
            })
            .collect()
    }

    /// The pause's Garage as page rows: its sections' headings and its tiles.
    pub fn pause_garage_rows(&self) -> Vec<Row> {
        tile_rows(&self.pause_tiles())
    }

    /// The pause's LAST MACHINE widget's word (P341): the machine Triangle swaps to, by its tile's name.
    pub fn last_machine_name(&self) -> Option<String> {
        let m = self.last_machine()?;
        let tile = self.pause_tiles().into_iter().find(|t| matches!(t.of, TileOf::Switch(x) if x == m));
        Some(tile.map_or_else(|| m.label().to_string(), |t| t.name))
    }

    /// The pause's Garage tile under the cursor.
    pub fn pause_tile(&self) -> Option<Tile> {
        let rows = self.pause_garage_rows();
        let Some(Key::Tile(i)) = rows.get(self.cursor(Screen::PauseGarage)).map(|r| r.key) else { return None };
        self.pause_tiles().into_iter().nth(i)
    }

    /// The pause's Garage row (P341): its page of tiles over the ride, the cursor on the ridden one.
    pub(super) fn open_pause_garage(&mut self) {
        let rows = self.pause_garage_rows();
        let tiles = self.pause_tiles();
        let at = rows.iter().position(|r| matches!(r.key, Key::Tile(i) if tiles[i].riding)).unwrap_or_else(|| Menu::first_row(&rows));
        self.push(Screen::PauseGarage);
        self.cursor[Screen::PauseGarage.index()] = at;
    }

    /// Triangle on the pause or its Garage (P341): the last machine ridden, where there is one the
    /// pauser can still switch to; else nothing.
    pub(super) fn quick_switch(&mut self) -> Act {
        match self.last_machine() {
            Some(to) => {
                self.stack.clear();
                Act::Switch(self.pauser, to)
            }
            None => Act::Stay,
        }
    }

    /// A press on the pause's Garage (P341): the grid walked, L1 and R1 a flier's skin, Cross the
    /// switch (the ridden machine's tile resumes), Circle the pause again and Start the ride.
    pub(super) fn walk_pause_garage(&mut self, p: Press) -> Act {
        if p.triangle {
            return self.quick_switch();
        }
        if p.start {
            self.stack.clear();
            return Act::Resume;
        }
        if p.back {
            self.pop();
            return Act::Stay;
        }
        let rows = self.pause_garage_rows();
        if rows.is_empty() {
            return Act::Stay;
        }
        let i = Screen::PauseGarage.index();
        let mut at = self.cursor[i].min(rows.len() - 1);
        if !rows[at].selectable() {
            at = Menu::first_row(&rows);
        }
        self.cursor[i] = at;
        if let Some(to) = grid_step(&rows, at, p) {
            self.cursor[i] = to;
            return Act::Stay;
        }
        let Some(t) = self.pause_tile() else { return Act::Stay };
        let TileOf::Switch(m) = t.of else { return Act::Stay };
        if p.l1 || p.r1 {
            if let Some(kind) = self.own_flier(m) {
                let offer = self.fliers_here().to_vec();
                if let Some(pick) = self.picks.0.get_mut(self.pauser) {
                    let was = pick.clone();
                    roster::step_flier(&offer, pick, kind, if p.l1 { -1 } else { 1 });
                    self.picks_changed |= *pick != was;
                }
            }
            return Act::Stay;
        }
        if p.ok {
            self.stack.clear();
            return if t.riding { Act::Resume } else { Act::Switch(self.pauser, m) };
        }
        Act::Stay
    }

    /// The pictures the tiles want baked (P341), by key, each once: every disc bike's paints on
    /// the first seat's rider, and every flier skin on offer.
    pub fn tile_pictures(&self) -> Vec<(String, Picture)> {
        let pick = self.picks.0.first().cloned().unwrap_or_default();
        let mut out: Vec<(String, Picture)> = Vec::new();
        let mut add = |p: Picture| {
            let key = p.key();
            if !out.iter().any(|(k, _)| *k == key) {
                out.push((key, p));
            }
        };
        for engine in roster::ENGINES {
            for model in self.catalogue.models(engine) {
                for b in self.catalogue.liveries(model).into_iter().filter(|b| b.engine == engine) {
                    add(Picture::Bike(self.catalogue.fit(&Pick { bike: b.archive.clone(), ..pick.clone() })));
                }
            }
        }
        for f in self.flier_offer.iter().filter(|f| f.why.is_none()) {
            add(Picture::Flier(f.clone()));
        }
        out
    }
}
