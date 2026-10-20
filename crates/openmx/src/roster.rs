//! The garage (P37): what each seat rides and who rides it, picked from what the disc offers, kept in
//! `garage.txt` beside the disc, and handed to the ride as the files the simulation loads.
//!
//! **The bike** is three choices, and the original's own loader (D7) is what joins them up. A bike
//! *model* is a `Bike\<model>[_<livery>].res` archive — 54 on the retail disc, `125sx`, `cr125_faisst`,
//! `yz450f_troy` — and each carries the four `.phy` files of one engine class, `bike\{race,pro}<cc>{a,p}.phy`.
//! Which of those four is ridden is the *set*: [`track::physics_file`] builds the name out of the class,
//! the tier (Race or Pro) and the tuning (A or P), sixteen names in all, and that name is read out of
//! the picked model's archive by [`physics`] — the same lookup the shell has always done for the
//! freeride's `Bike\Race125P.phy`, now with the seat's set in place of the freeride constants. So a
//! pick is not a skin over the stock bike: the `.phy` that answers is the one [`game_sim::phy::Phy`]
//! reads every parameter of the ride from. The class is the model's, not a free choice: the archive
//! only carries its own class's four files, so [`Catalogue::fit`] keeps a set's class on its model's.
//!
//! **The rider** is the disc's rider set, `Rider\<name><1|2>.res`: one body shared by every rider, and
//! each archive a `.sub` repainting its six gear materials (helmet, jersey, pants, boots, gloves,
//! goggles) — the name is whose gear it is, and 1 and 2 are its two colourways. The freestyle copies
//! (`ff_*`), the pack's ghost (`ghost_ai_01`, which names the low-detail AI body) and `custom` (the
//! create-a-rider wardrobe, which has no `.sub` of its own) are not riders a seat can pick.
//!
//! **The shape** is opentennis's character select (`crates/hst/src/play/main_menu/roster.rs`): every
//! seat its own column with its own cursor and its own picks, and left/right on a row walking that
//! row's choices. That is [`Garage`], a pure state machine of one seat's picks fed by direction alone,
//! so it is tested without a window. The picture is the front end's (P36).
//!
//! **Not here yet:** the biplane is not a bike pick. It is the track's own machine, swapped to with the
//! shell's vehicle switch (P19), and a garage row for it waits on Phase 11's career/vehicle select.

use game_render::Disc;
use game_render::disc::Res;
use game_sim::track::{self, Engine, Tuning};
use std::path::Path;

/// How many seats the garage keeps picks for: four players on one screen, as P38's join screen has.
pub const SEATS: usize = 4;

/// The file the picks live in, beside the disc like the front end's own `settings.txt`.
pub const GARAGE_FILE: &str = "garage.txt";

// ---- the sixteen sets ------------------------------------------------------------------------------

/// One of the sixteen `.phy` files a bike can ride on: a class, a tier and a tuning, which is exactly
/// what the original's vehicle loader builds the name from (D7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Set {
    pub engine: Engine,
    /// The Pro files rather than the Race ones.
    pub pro: bool,
    pub tuning: Tuning,
}

/// The four classes in the order the game indexes them.
pub const ENGINES: [Engine; 4] = [Engine::Cc50, Engine::Cc125, Engine::Cc250, Engine::Cc500];

/// The engine size a class is named for.
pub fn cc(engine: Engine) -> u32 {
    match engine {
        Engine::Cc50 => 50,
        Engine::Cc125 => 125,
        Engine::Cc250 => 250,
        Engine::Cc500 => 500,
    }
}

impl Set {
    /// What a freeride boot rides with nothing picked: `Bike\Race125P.phy` (D7).
    pub const FREERIDE: Set = Set {
        engine: track::FREERIDE_ENGINE,
        pro: track::FREERIDE_PRO,
        tuning: Tuning::P,
    };

    /// All sixteen, class by class, Race before Pro and A before P.
    pub fn all() -> Vec<Set> {
        let mut out = Vec::with_capacity(16);
        for engine in ENGINES {
            for pro in [false, true] {
                for tuning in [Tuning::A, Tuning::P] {
                    out.push(Set { engine, pro, tuning });
                }
            }
        }
        out
    }

    /// The name the loader asks the resource system for, `Bike\Race125P.phy`.
    pub fn file(self) -> String {
        track::physics_file(self.engine, self.pro, self.tuning)
    }

    /// The set as `garage.txt` spells it: the file's own name, lowercased, `race125p`.
    pub fn word(self) -> String {
        let file = self.file().to_ascii_lowercase();
        let leaf = file.rsplit('\\').next().unwrap_or(&file);
        leaf.trim_end_matches(".phy").to_string()
    }

    pub fn from_word(w: &str) -> Option<Set> {
        let w = w.trim().to_ascii_lowercase();
        Set::all().into_iter().find(|s| s.word() == w)
    }

    /// The set's three halves as a rider reads them: `125 Race P`.
    pub fn label(self) -> String {
        let tier = if self.pro { "Pro" } else { "Race" };
        let tuning = match self.tuning {
            Tuning::A => 'A',
            Tuning::P => 'P',
        };
        format!("{} {tier} {tuning}", cc(self.engine))
    }
}

/// The `.phy` a picked set rides on, out of the picked model's archive: the bytes, the member that
/// answered, and a line to print when it was not the set's own file.
///
/// The lookup the shell has always made for the freeride (P25): the set's own file first, then a
/// file named after the model, then the archive's first `.phy`. A model archive carries all four of
/// its class's files, so with [`Catalogue::fit`]'s pick the first one answers; the other two are
/// for an archive this catalogue did not read, which `--bike` can still name.
pub fn physics(archive: &Res, set: Set, model: &str) -> Result<(Vec<u8>, String, Option<String>), String> {
    let own = set.file();
    let wanted = [own.clone(), format!("{model}.phy"), ".phy".to_string()];
    let (want, e) = wanted
        .iter()
        .find_map(|w| archive.find_suffix(w).map(|e| (w, e)))
        .ok_or("the bike archive has no .phy")?;
    let name = e.name.clone();
    let note = (*want != own).then(|| format!("the bike archive has no {own}; riding {name} instead"));
    Ok((archive.read_suffix(want)?, name, note))
}

// ---- what the disc offers --------------------------------------------------------------------------

/// One bike model archive: `cr125_faisst` is the `cr125` in Faisst's livery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bike {
    /// The archive's own name, `cr125_faisst`: what `--bike` takes and what `garage.txt` keeps.
    pub archive: String,
    /// The bike the livery is painted on, `cr125`.
    pub model: String,
    /// The paint, `faisst`, or empty for the model's own.
    pub livery: String,
    /// The class of the four `.phy` files the archive carries.
    pub engine: Engine,
}

/// One rider archive: `fox2` is Fox's gear in its second colours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rider {
    /// The archive's own name, `fox2`: what `--rider` takes and what `garage.txt` keeps.
    pub archive: String,
    /// Whose gear, `fox`.
    pub name: String,
    /// Which of its colourways, 1 or 2.
    pub colours: u8,
}

/// Split `cr125_faisst` into its model and its livery at the first `_`.
fn model_and_livery(archive: &str) -> (String, String) {
    match archive.split_once('_') {
        Some((m, l)) => (m.to_string(), l.to_string()),
        None => (archive.to_string(), String::new()),
    }
}

/// `fox2` as `fox` in colours 2, or `None` for an archive a seat cannot pick.
fn rider_of(archive: &str) -> Option<Rider> {
    if archive.starts_with("ff_") || archive.contains('_') || archive == "custom" {
        return None;
    }
    let colours = match archive.as_bytes().last()? {
        b'1' => 1,
        b'2' => 2,
        _ => return None,
    };
    let name = archive[..archive.len() - 1].to_string();
    (!name.is_empty()).then(|| Rider { archive: archive.to_string(), name, colours })
}

/// The class a model archive's `.phy` files are for: the one whose four names are all in it.
pub fn engine_of(archive: &Res) -> Option<Engine> {
    ENGINES.into_iter().find(|&engine| {
        Set::all()
            .into_iter()
            .filter(|s| s.engine == engine)
            .all(|s| archive.find_suffix(&s.file()).is_some())
    })
}

/// The leaf name of `Dir\leaf.res`, lowercased, if the member is a `.res` in `dir`.
fn leaf_in<'a>(member: &'a str, dir: &str) -> Option<String> {
    let lower = member.to_ascii_lowercase();
    let rest = lower.strip_prefix(&format!("{}\\", dir.to_ascii_lowercase()))?;
    let leaf = rest.strip_suffix(".res")?;
    (!leaf.contains('\\')).then(|| leaf.to_string())
}

/// Every bike and rider a seat can pick, read off the disc.
#[derive(Clone, Debug, Default)]
pub struct Catalogue {
    /// Class by class, and in the disc's own order within a class.
    pub bikes: Vec<Bike>,
    /// In the disc's own order: a name's two colourways side by side.
    pub riders: Vec<Rider>,
}

impl Catalogue {
    /// Open every `Bike\*.res` for the class its `.phy` files are, and list every `Rider\*.res` a
    /// seat can ride as. A bike archive with no complete class of `.phy` is left out and said.
    pub fn read(disc: &mut Disc) -> Result<Catalogue, String> {
        let members: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).collect();
        let mut bikes = Vec::new();
        for member in &members {
            let Some(archive) = leaf_in(member, "Bike") else { continue };
            let res = disc.res(member)?;
            match engine_of(&res) {
                Some(engine) => {
                    let (model, livery) = model_and_livery(&archive);
                    bikes.push(Bike { archive, model, livery, engine });
                }
                None => println!("garage: {member} has no whole class of .phy files"),
            }
        }
        bikes.sort_by_key(|b| b.engine as usize);
        let riders = members.iter().filter_map(|m| leaf_in(m, "Rider")).filter_map(|a| rider_of(&a)).collect();
        Ok(Catalogue { bikes, riders })
    }

    pub fn bike(&self, archive: &str) -> Option<&Bike> {
        self.bikes.iter().find(|b| b.archive.eq_ignore_ascii_case(archive))
    }

    pub fn rider(&self, archive: &str) -> Option<&Rider> {
        self.riders.iter().find(|r| r.archive.eq_ignore_ascii_case(archive))
    }

    /// The models of one class, each once, in catalogue order.
    pub fn models(&self, engine: Engine) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for b in self.bikes.iter().filter(|b| b.engine == engine) {
            if !out.contains(&b.model.as_str()) {
                out.push(&b.model);
            }
        }
        out
    }

    /// One model's liveries, as the archives that paint it.
    pub fn liveries(&self, model: &str) -> Vec<&Bike> {
        self.bikes.iter().filter(|b| b.model == model).collect()
    }

    /// The riders' names, each once.
    pub fn names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for r in &self.riders {
            if !out.contains(&r.name.as_str()) {
                out.push(&r.name);
            }
        }
        out
    }

    /// A pick this disc can ride: an unknown bike or rider becomes the stock one (or the first there
    /// is), and the set's class becomes the bike's own, since that is the only class its archive has.
    pub fn fit(&self, pick: &Pick) -> Pick {
        let stock = Pick::default();
        let bike = self
            .bike(&pick.bike)
            .or_else(|| self.bike(&stock.bike))
            .or(self.bikes.first());
        let rider = self
            .rider(&pick.rider)
            .or_else(|| self.rider(&stock.rider))
            .or(self.riders.first());
        let mut set = pick.set;
        if let Some(b) = bike {
            set.engine = b.engine;
        }
        Pick {
            bike: bike.map_or(pick.bike.clone(), |b| b.archive.clone()),
            set,
            rider: rider.map_or(pick.rider.clone(), |r| r.archive.clone()),
        }
    }
}

// ---- a seat's picks ---------------------------------------------------------------------------------

/// What one seat rides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    /// The bike model archive, `125sx`.
    pub bike: String,
    pub set: Set,
    /// The rider archive, `thq1`.
    pub rider: String,
}

impl Default for Pick {
    /// The shell as it was before there was a garage: the `125sx`, on the freeride's set, in the stock
    /// gear.
    fn default() -> Pick {
        Pick { bike: "125sx".into(), set: Set::FREERIDE, rider: "thq1".into() }
    }
}

impl Pick {
    /// The disc members a ride on this pick opens: the bike's archive and the rider's.
    pub fn members(&self) -> (String, String) {
        (format!("Bike\\{}.res", self.bike), format!("Rider\\{}.res", self.rider))
    }
}

/// What a name in `garage.txt` may hold: an archive's own characters.
fn clean(v: &str) -> Option<String> {
    let v = v.trim().to_ascii_lowercase();
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(v)
}

/// Every seat's picks: `garage.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picks(pub [Pick; SEATS]);

impl Default for Picks {
    fn default() -> Picks {
        Picks(std::array::from_fn(|_| Pick::default()))
    }
}

impl Picks {
    /// Read a file's text: `seat<n> = <bike> <set> <rider>` a line, `#` a comment. A seat the file
    /// does not name, or names with a word it cannot read, keeps the stock pick for that word, so an
    /// older or hand-edited file still opens.
    pub fn parse(text: &str) -> Picks {
        let mut picks = Picks::default();
        for line in text.lines() {
            let Some((k, v)) = line.split('#').next().and_then(|l| l.split_once('=')) else { continue };
            let Some(seat) = k.trim().strip_prefix("seat").and_then(|n| n.parse::<usize>().ok()) else {
                continue;
            };
            let Some(pick) = seat.checked_sub(1).and_then(|i| picks.0.get_mut(i)) else { continue };
            let mut words = v.split_whitespace();
            if let Some(bike) = words.next().and_then(clean) {
                pick.bike = bike;
            }
            if let Some(set) = words.next().and_then(Set::from_word) {
                pick.set = set;
            }
            if let Some(rider) = words.next().and_then(clean) {
                pick.rider = rider;
            }
        }
        picks
    }

    pub fn text(&self) -> String {
        let mut out = String::from("# openmx's garage (P37): seat = bike set rider\n");
        for (i, p) in self.0.iter().enumerate() {
            out.push_str(&format!("seat{} = {} {} {}\n", i + 1, p.bike, p.set.word(), p.rider));
        }
        out
    }

    /// `garage.txt` out of `dir`; a missing file is every seat on the stock pick.
    pub fn load(dir: &Path) -> Picks {
        Picks::parse(&std::fs::read_to_string(dir.join(GARAGE_FILE)).unwrap_or_default())
    }

    /// `garage.txt` into `dir`, written whole and then moved into place, so a reader never sees half.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join(GARAGE_FILE);
        let part = dir.join(format!("{GARAGE_FILE}.part"));
        std::fs::write(&part, self.text()).map_err(|e| format!("{}: {e}", part.display()))?;
        std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

// ---- one seat's garage -------------------------------------------------------------------------------

/// The garage's rows, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// 50, 125, 250, 500.
    Class,
    /// The models of the class.
    Model,
    /// The model's paints.
    Livery,
    /// Race or Pro.
    Tier,
    /// A or P.
    Tuning,
    /// Whose gear.
    Rider,
    /// Its colourway.
    Colours,
}

pub const ROWS: [Row; 7] = [Row::Class, Row::Model, Row::Livery, Row::Tier, Row::Tuning, Row::Rider, Row::Colours];

/// Step `i` by `by` through `n` choices, wrapping.
fn wrap(i: usize, by: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    (i as i64 + by as i64).rem_euclid(n as i64) as usize
}

/// One seat's garage: its cursor and its pick, walked by the d-pad. Up and down choose the row, left
/// and right that row's value. Every value it can reach is one [`Catalogue::fit`] leaves alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Garage {
    pub row: usize,
    pub pick: Pick,
}

impl Garage {
    pub fn new(catalogue: &Catalogue, pick: &Pick) -> Garage {
        Garage { row: 0, pick: catalogue.fit(pick) }
    }

    pub fn up(&mut self) {
        self.row = wrap(self.row, -1, ROWS.len());
    }

    pub fn down(&mut self) {
        self.row = wrap(self.row, 1, ROWS.len());
    }

    /// Left (`-1`) or right (`1`) on the current row.
    pub fn turn(&mut self, c: &Catalogue, by: i32) {
        let p = &mut self.pick;
        let Some(bike) = c.bike(&p.bike).cloned() else { return };
        match ROWS[self.row] {
            Row::Class => {
                let classes: Vec<Engine> = ENGINES.into_iter().filter(|&e| !c.models(e).is_empty()).collect();
                let at = classes.iter().position(|&e| e == bike.engine).unwrap_or(0);
                let engine = classes[wrap(at, by, classes.len())];
                // The class's first model in its first paint.
                if let Some(b) = c.bikes.iter().find(|b| b.engine == engine) {
                    p.bike = b.archive.clone();
                }
            }
            Row::Model => {
                let models = c.models(bike.engine);
                let at = models.iter().position(|m| *m == bike.model).unwrap_or(0);
                let model = models[wrap(at, by, models.len())];
                p.bike = c.liveries(model)[0].archive.clone();
            }
            Row::Livery => {
                let paints = c.liveries(&bike.model);
                let at = paints.iter().position(|b| b.archive == bike.archive).unwrap_or(0);
                p.bike = paints[wrap(at, by, paints.len())].archive.clone();
            }
            Row::Tier => p.set.pro = !p.set.pro,
            Row::Tuning => {
                p.set.tuning = match p.set.tuning {
                    Tuning::A => Tuning::P,
                    Tuning::P => Tuning::A,
                }
            }
            Row::Rider => {
                let names = c.names();
                let Some(r) = c.rider(&p.rider) else { return };
                let at = names.iter().position(|n| *n == r.name).unwrap_or(0);
                let name = names[wrap(at, by, names.len())];
                // The same colourway where the next name has it.
                let next = c
                    .riders
                    .iter()
                    .find(|x| x.name == name && x.colours == r.colours)
                    .or_else(|| c.riders.iter().find(|x| x.name == name));
                if let Some(x) = next {
                    p.rider = x.archive.clone();
                }
            }
            Row::Colours => {
                let Some(r) = c.rider(&p.rider) else { return };
                let ways: Vec<&Rider> = c.riders.iter().filter(|x| x.name == r.name).collect();
                let at = ways.iter().position(|x| x.archive == r.archive).unwrap_or(0);
                p.rider = ways[wrap(at, by, ways.len())].archive.clone();
            }
        }
        *p = c.fit(p);
    }

    /// The row's value as the garage prints it.
    pub fn value(&self, c: &Catalogue, row: Row) -> String {
        let p = &self.pick;
        let bike = c.bike(&p.bike);
        let rider = c.rider(&p.rider);
        match row {
            Row::Class => format!("{}cc", cc(p.set.engine)),
            Row::Model => bike.map_or(p.bike.clone(), |b| b.model.to_ascii_uppercase()),
            Row::Livery => match bike {
                Some(b) if !b.livery.is_empty() => b.livery.replace('_', " "),
                _ => "stock".into(),
            },
            Row::Tier => (if p.set.pro { "Pro" } else { "Race" }).into(),
            Row::Tuning => (match p.set.tuning {
                Tuning::A => "A",
                Tuning::P => "P",
            })
            .into(),
            Row::Rider => rider.map_or(p.rider.clone(), |r| r.name.clone()),
            Row::Colours => rider.map_or(String::new(), |r| r.colours.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Catalogue {
        let bike = |archive: &str, engine| {
            let (model, livery) = model_and_livery(archive);
            Bike { archive: archive.into(), model, livery, engine }
        };
        Catalogue {
            bikes: vec![
                bike("fast50_01", Engine::Cc50),
                bike("125sx", Engine::Cc125),
                bike("cr125_faisst", Engine::Cc125),
                bike("cr125_motoxxx", Engine::Cc125),
                bike("cr250_faisst", Engine::Cc250),
            ],
            riders: ["thq1", "thq2", "fox1", "fox2"].iter().filter_map(|a| rider_of(a)).collect(),
        }
    }

    #[test]
    fn the_sixteen_sets_are_the_loaders_sixteen_names() {
        let all = Set::all();
        assert_eq!(all.len(), 16);
        let mut words: Vec<String> = all.iter().map(|s| s.word()).collect();
        words.dedup();
        assert_eq!(words.len(), 16);
        for s in all {
            assert_eq!(Set::from_word(&s.word()), Some(s));
            assert_eq!(s.file().to_ascii_lowercase(), format!("bike\\{}.phy", s.word()));
        }
        assert_eq!(Set::FREERIDE.file(), "Bike\\Race125P.phy");
    }

    #[test]
    fn a_rider_archive_is_a_name_and_its_colours() {
        assert_eq!(rider_of("fox2"), Some(Rider { archive: "fox2".into(), name: "fox".into(), colours: 2 }));
        for not in ["ff_fox2", "ghost_ai_01", "custom"] {
            assert_eq!(rider_of(not), None, "{not}");
        }
        assert_eq!(model_and_livery("yz450f_troy"), ("yz450f".into(), "troy".into()));
        assert_eq!(leaf_in("Bike\\CR125_faisst.res", "Bike").as_deref(), Some("cr125_faisst"));
        assert_eq!(leaf_in("Bike\\anim\\x.res", "Bike"), None);
    }

    #[test]
    fn a_garage_walks_every_row_and_keeps_the_class_on_the_bike() {
        let c = tiny();
        let mut g = Garage::new(&c, &Pick::default());
        assert_eq!(g.pick, Pick::default());
        // Class right: 125 → 250, and the set follows the bike.
        g.turn(&c, 1);
        assert_eq!((g.pick.bike.as_str(), g.pick.set.engine), ("cr250_faisst", Engine::Cc250));
        g.turn(&c, -1);
        g.turn(&c, -1);
        assert_eq!((g.pick.bike.as_str(), g.pick.set.engine), ("fast50_01", Engine::Cc50));
        g.turn(&c, 1);
        g.down(); // model
        g.turn(&c, 1);
        assert_eq!(g.pick.bike, "cr125_faisst");
        g.down(); // livery
        g.turn(&c, 1);
        assert_eq!(g.pick.bike, "cr125_motoxxx");
        assert_eq!(g.value(&c, Row::Livery), "motoxxx");
        g.down(); // tier
        g.turn(&c, 1);
        g.down(); // tuning
        g.turn(&c, 1);
        assert_eq!(g.pick.set.word(), "pro125a");
        g.down(); // rider
        g.turn(&c, 1);
        assert_eq!(g.pick.rider, "fox1");
        g.down(); // colours
        g.turn(&c, 1);
        assert_eq!(g.pick.rider, "fox2");
        g.down();
        assert_eq!(g.row, 0, "the rows wrap");
        assert_eq!(c.fit(&g.pick), g.pick);
    }

    #[test]
    fn fit_mends_a_pick_the_disc_cannot_ride() {
        let c = tiny();
        let odd = Pick { bike: "cr250_faisst".into(), set: Set::FREERIDE, rider: "nobody1".into() };
        let fit = c.fit(&odd);
        assert_eq!(fit.set.engine, Engine::Cc250);
        assert_eq!(fit.rider, "thq1");
        assert_eq!(c.fit(&Pick { bike: "gone".into(), ..Pick::default() }).bike, "125sx");
    }
}
