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
    /// Its `.sub`'s repaint of the shared body, base material to its own (P86): which helmet,
    /// goggles, gloves, boots, jersey and pants the set wears. Empty until [`Catalogue::read`] reads it.
    pub suit: Vec<(String, String)>,
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
    (!name.is_empty()).then(|| Rider { archive: archive.to_string(), name, colours, suit: Vec::new() })
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    /// Class by class, and in the disc's own order within a class.
    pub bikes: Vec<Bike>,
    /// In the disc's own order: a name's two colourways side by side.
    pub riders: Vec<Rider>,
    /// What a rider can wear over their gear set (P86).
    pub wardrobe: Wardrobe,
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
        let mut riders: Vec<Rider> = Vec::new();
        for member in &members {
            let Some(mut rider) = leaf_in(member, "Rider").and_then(|a| rider_of(&a)) else { continue };
            // The set's own pieces, for the garage to start from (P86).
            if let Ok(sub) = disc.res(member).and_then(|r| r.read_suffix(&format!("{}.sub", rider.archive))) {
                rider.suit = game_render::rider::Suit::parse(&game_render::ini::text(&sub)).materials;
            }
            riders.push(rider);
        }
        let wardrobe = Wardrobe::read(disc).unwrap_or_else(|e| {
            println!("garage: no wardrobe: {e}");
            Wardrobe::default()
        });
        Ok(Catalogue { bikes, riders, wardrobe })
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
            look: self.wardrobe.fit(&pick.look),
            preset: pick.preset.clone(),
        }
    }

    /// The material the rider's gear set wears for a piece (`helmet`, `jerseysleeves`, …): what its
    /// `.sub` repaints that piece's base material with, or the base material when it does not.
    pub fn own(&self, rider: &str, piece: &str) -> Option<String> {
        let r = self.rider(rider)?;
        r.suit.iter().find(|(from, _)| is_piece(from, piece)).map(|(_, to)| to.clone())
    }
}

// ---- a seat's picks ---------------------------------------------------------------------------------

/// What one seat rides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    /// The bike model archive, `125sx`.
    pub bike: String,
    pub set: Set,
    /// The rider archive, `thq1`: the gear set.
    pub rider: String,
    /// What the rider wears over the gear set, and their name and number (P86).
    pub look: Look,
    /// The saved look this seat last loaded or saved, by [`Presets::key`], or empty.
    pub preset: String,
}

impl Default for Pick {
    /// The shell as it was before there was a garage: the `125sx`, on the freeride's set, in the stock
    /// gear.
    fn default() -> Pick {
        Pick { bike: "125sx".into(), set: Set::FREERIDE, rider: "thq1".into(), look: Look::default(), preset: String::new() }
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
    ///
    /// A seat's look and the preset it last took are lines of their own, `seat<n>.look = …` in
    /// [`Look::text`]'s words and `seat<n>.preset = <name> <number>`, written only when the seat has
    /// them, so a file from before P86 reads as it did.
    pub fn parse(text: &str) -> Picks {
        let mut picks = Picks::default();
        for line in text.lines() {
            let Some((k, v)) = line.split('#').next().and_then(|l| l.split_once('=')) else { continue };
            let (k, field) = k.trim().split_once('.').unwrap_or((k.trim(), ""));
            let Some(seat) = k.strip_prefix("seat").and_then(|n| n.parse::<usize>().ok()) else {
                continue;
            };
            let Some(pick) = seat.checked_sub(1).and_then(|i| picks.0.get_mut(i)) else { continue };
            match field.trim() {
                "" => {}
                "look" => {
                    pick.look = Look::parse(v);
                    continue;
                }
                "preset" => {
                    // A key is a name and a number: longer than a name alone.
                    let ok = |c: char| c.is_ascii_alphanumeric() || " -'.&".contains(c);
                    pick.preset = v.trim().chars().filter(|&c| ok(c)).take(RIDER_NAME_MAX + 1 + NUMBER_MAX).collect();
                    continue;
                }
                _ => continue,
            }
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
            if p.look != Look::default() {
                out.push_str(&format!("seat{}.look = {}\n", i + 1, p.look.text()));
            }
            if !p.preset.is_empty() {
                out.push_str(&format!("seat{}.preset = {}\n", i + 1, p.preset));
            }
        }
        out
    }

    /// `garage.txt` out of `dir`; a missing file is every seat on the stock pick.
    pub fn load(dir: &Path) -> Picks {
        Picks::parse(&crate::files::read_to_string(dir.join(GARAGE_FILE)).unwrap_or_default())
    }

    /// `garage.txt` into `dir`, written whole and then moved into place, so a reader never sees half.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join(GARAGE_FILE);
        let part = dir.join(format!("{GARAGE_FILE}.part"));
        crate::files::write(&part, self.text()).map_err(|e| format!("{}: {e}", part.display()))?;
        crate::files::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

// ---- the look (P86) ----------------------------------------------------------------------------------

/// The file the saved looks live in, beside `settings.txt`.
pub const PRESETS_FILE: &str = "riders.txt";

/// The longest rider name the garage types, as the front end's own Name (P36).
pub const RIDER_NAME_MAX: usize = 12;

/// The most digits a rider number has: three fit the original's number plate font across a plate.
pub const NUMBER_MAX: usize = 3;

/// The archive the original's create-a-rider screen dresses from: the body, and every piece the
/// wardrobe lists painted (its `.psm` and `.pst`), every brand's in one place.
pub const WARDROBE_ARCHIVE: &str = "custom.res";

/// The fonts the original's gear reader falls back on when an outfit names none, and the plate font
/// every bike on the disc names but the black ones.
pub const NAME_FONT: &str = "name1";
pub const NUMBER_FONT: &str = "number1";
pub const PLATE_FONT: &str = "plate3";

/// The four pieces worn over the jersey and pants, in the order of the original's custom rider screen
/// (`ui\singleplayerbikeriderselect.lyt`: Helmet, Goggle, Glove, Boot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Helmet,
    Goggles,
    Gloves,
    Boots,
}

impl Piece {
    pub const ALL: [Piece; 4] = [Piece::Helmet, Piece::Goggles, Piece::Gloves, Piece::Boots];

    /// The word in its materials' names, `Fox_Helmet_02`, and its list's in `rider\gear.dat`.
    pub fn word(self) -> &'static str {
        match self {
            Piece::Helmet => "helmet",
            Piece::Goggles => "goggles",
            Piece::Gloves => "gloves",
            Piece::Boots => "boots",
        }
    }

    fn list(self) -> &'static str {
        match self {
            Piece::Helmet => "Helmets",
            Piece::Goggles => "Goggles",
            Piece::Gloves => "Gloves",
            Piece::Boots => "Boots",
        }
    }
}

/// Is `material` one of a piece's: `THQ_Helmet_01` is a `helmet`'s, `Fox_JerseySleeves_02` a
/// `jerseysleeves`'.
pub fn is_piece(material: &str, word: &str) -> bool {
    material.to_ascii_lowercase().contains(&format!("_{word}_"))
}

/// A piece's material as a rider reads it: `O'neal_Helmet_02` is `O'neal 2`.
pub fn piece_label(material: &str) -> String {
    let brand = material.split('_').next().unwrap_or(material);
    let n = material.rsplit('_').next().unwrap_or("").trim_start_matches('0');
    match n.is_empty() || brand == material {
        true => material.to_string(),
        false => format!("{brand} {n}"),
    }
}

/// What a seat wears over its gear set, and who it is: the original's custom rider screen's rows
/// below Gear Set. A piece left empty is the gear set's own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Look {
    /// A `[JerseyPants]` entry of `rider\gear.dat`, lowercased, `fox2`: the jersey and pants together,
    /// as the original pairs them.
    pub outfit: String,
    /// The helmet's, goggles', gloves' and boots' materials, in [`Piece::ALL`]'s order, `Fox_Helmet_02`.
    pub pieces: [String; 4],
    /// Drawn across the jersey's back.
    pub name: String,
    /// Drawn on the jersey under the name and on the bike's number plates.
    pub number: String,
}

/// What a name may hold: what the garage's keys type, trimmed and cut to [`RIDER_NAME_MAX`].
pub fn clean_rider_name(v: &str) -> String {
    let ok = |c: char| c.is_ascii_alphanumeric() || " -'.&".contains(c);
    let kept: String = v.trim().chars().filter(|&c| ok(c)).take(RIDER_NAME_MAX).collect();
    kept.trim().to_string()
}

/// A number's digits, at most [`NUMBER_MAX`].
pub fn clean_number(v: &str) -> String {
    v.trim().chars().filter(|c| c.is_ascii_digit()).take(NUMBER_MAX).collect()
}

/// A material name as `rider\gear.dat` writes them: `O'neal_Helmet_01`.
fn clean_material(v: &str) -> String {
    let v = v.trim();
    match v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\'') {
        true => v.to_string(),
        false => String::new(),
    }
}

impl Look {
    pub fn piece(&self, p: Piece) -> &str {
        &self.pieces[p as usize]
    }

    /// The look as one line: `outfit, helmet, goggles, gloves, boots, number, name`, an empty field
    /// the gear set's own. The name is last, so nothing it holds can move the others. The gear set
    /// as it comes is the empty line.
    pub fn text(&self) -> String {
        if *self == Look::default() {
            return String::new();
        }
        let [h, g, gl, b] = &self.pieces;
        format!("{}, {h}, {g}, {gl}, {b}, {}, {}", self.outfit, self.number, self.name)
    }

    pub fn parse(text: &str) -> Look {
        let f: Vec<&str> = text.splitn(7, ',').collect();
        let at = |i: usize| f.get(i).copied().unwrap_or("");
        Look {
            outfit: clean(at(0)).unwrap_or_default(),
            pieces: std::array::from_fn(|i| clean_material(at(1 + i))),
            number: clean_number(at(5)),
            name: clean_rider_name(at(6)),
        }
    }
}

/// One `[JerseyPants]` entry: a jersey and pants that go together, and the fonts the name and number
/// are drawn in on that jersey (black on the light ones).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outfit {
    /// Lowercased, `thq1`, which is the stock rider archive's name too.
    pub id: String,
    /// As the file writes it, `THQ1`.
    pub label: String,
    pub jersey: String,
    pub pants: String,
    pub name_font: String,
    pub number_font: String,
}

/// What the original's create-a-rider screen offers, from `appstart.res`'s `rider\gear.dat` and
/// `bike\bikes.dat`, kept to what [`WARDROBE_ARCHIVE`] has the paint for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wardrobe {
    pub outfits: Vec<Outfit>,
    /// Each [`Piece`]'s materials, in the file's order.
    pub pieces: [Vec<String>; 4],
    /// A bike `.sub`'s name, lowercased, and the font its number plates are drawn in.
    pub plates: Vec<(String, String)>,
}

/// An entry of an `item=` list as written, its quotes off.
fn item(v: &str) -> String {
    v.trim().trim_matches('"').trim().to_string()
}

impl Wardrobe {
    /// The two files' text, and whether the wardrobe archive has a material's `.psm`.
    pub fn parse(gear: &str, bikes: &str, has: &dyn Fn(&str) -> bool) -> Wardrobe {
        let gear = game_render::ini::parse(gear);
        let section = |name: &str, key: &str| {
            gear.iter().find(|s| s.name.eq_ignore_ascii_case(name) && s.get(key).is_some())
        };
        let items = |list: &str| -> Vec<String> {
            gear.iter()
                .filter(|s| s.name.eq_ignore_ascii_case(list))
                .flat_map(|s| s.keys.iter())
                .filter(|(k, _)| k.eq_ignore_ascii_case("item"))
                .map(|(_, v)| item(v))
                .filter(|v| !v.is_empty())
                .collect()
        };
        let mut out = Wardrobe::default();
        for id in items("JerseyPants") {
            let Some(s) = section(&id, "JerseyMaterial") else { continue };
            let (jersey, pants) = (item(s.get("JerseyMaterial").unwrap_or("")), item(s.get("PantMaterial").unwrap_or("")));
            if !has(&jersey) || !has(&pants) {
                continue;
            }
            out.outfits.push(Outfit {
                id: id.to_ascii_lowercase(),
                label: id.clone(),
                jersey,
                pants,
                name_font: s.get("NameFont").map_or(NAME_FONT.into(), item),
                number_font: s.get("NumberFont").map_or(NUMBER_FONT.into(), item),
            });
        }
        for p in Piece::ALL {
            out.pieces[p as usize] = items(p.list()).into_iter().filter(|m| has(m)).collect();
        }
        for s in game_render::ini::parse(bikes) {
            if let Some(font) = s.get("NumberPlateFont") {
                out.plates.push((s.name.to_ascii_lowercase(), item(font)));
            }
        }
        out
    }

    pub fn read(disc: &mut Disc) -> Result<Wardrobe, String> {
        let app = disc.res("appstart.res")?;
        let gear = game_render::ini::text(&app.read_suffix("rider\\gear.dat")?);
        let bikes = game_render::ini::text(&app.read_suffix("bike\\bikes.dat")?);
        let member = disc
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .find(|n| leaf_in(n, "Rider").is_some_and(|l| format!("{l}.res") == WARDROBE_ARCHIVE))
            .ok_or(format!("the disc has no Rider\\{WARDROBE_ARCHIVE}"))?;
        let custom = disc.res(&member)?;
        Ok(Wardrobe::parse(&gear, &bikes, &|m| custom.find_suffix(&format!("\\{m}.psm")).is_some()))
    }

    pub fn outfit(&self, id: &str) -> Option<&Outfit> {
        self.outfits.iter().find(|o| o.id.eq_ignore_ascii_case(id))
    }

    /// The name and number fonts on a jersey: the look's outfit's, else the gear set's own entry
    /// (a stock rider archive is named as its `[JerseyPants]` entry is), else the reader's defaults.
    pub fn fonts(&self, rider: &str, look: &Look) -> (String, String) {
        let id = if look.outfit.is_empty() { rider } else { &look.outfit };
        match self.outfit(id) {
            Some(o) => (o.name_font.clone(), o.number_font.clone()),
            None => (NAME_FONT.into(), NUMBER_FONT.into()),
        }
    }

    /// The font a bike's number plates are drawn in.
    pub fn plate_font(&self, bike: &str) -> String {
        self.plates
            .iter()
            .find(|(b, _)| b.eq_ignore_ascii_case(bike))
            .map_or(PLATE_FONT.into(), |(_, f)| f.clone())
    }

    /// A look with what this wardrobe does not have taken off (an empty wardrobe, a disc not read,
    /// keeps everything).
    pub fn fit(&self, look: &Look) -> Look {
        if self.outfits.is_empty() {
            return look.clone();
        }
        let mut out = look.clone();
        if self.outfit(&out.outfit).is_none() {
            out.outfit.clear();
        }
        for p in Piece::ALL {
            let i = p as usize;
            if !self.pieces[i].iter().any(|m| m.eq_ignore_ascii_case(&out.pieces[i])) {
                out.pieces[i].clear();
            }
        }
        out
    }

    /// The repaint a look puts over its gear set's: `(base, material)` pairs for every piece the look
    /// changes, the base the body's own name for it (`THQ_Helmet_01`), so they go after the `.sub`'s.
    pub fn materials(&self, look: &Look, base: &[String]) -> Vec<(String, String)> {
        let mut wants: Vec<(&str, String)> = Vec::new();
        if let Some(o) = self.outfit(&look.outfit).filter(|_| !look.outfit.is_empty()) {
            wants.push(("jerseysleeves", o.jersey.clone()));
            wants.push(("pants", o.pants.clone()));
        }
        for p in Piece::ALL {
            if !look.piece(p).is_empty() {
                wants.push((p.word(), look.piece(p).to_string()));
            }
        }
        let mut out = Vec::new();
        for (word, to) in wants {
            for from in base.iter().filter(|b| is_piece(b, word)) {
                out.push((from.clone(), to.clone()));
            }
        }
        out
    }
}

/// The saved looks: `riders.txt`, beside `settings.txt`. Each is a whole [`Pick`] — the bike and its
/// set, the gear set and everything worn over it — saved under its rider's name and number.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presets(pub Vec<Pick>);

impl Presets {
    /// What a look is saved and loaded under: `Ryan 7`. Empty without a name.
    pub fn key(pick: &Pick) -> String {
        let name = clean_rider_name(&pick.look.name);
        match name.is_empty() {
            true => String::new(),
            false => format!("{name} {}", pick.look.number).trim().to_string(),
        }
    }

    /// `rider = <bike> <set> <gear set> | <look>` a line, [`Look::text`]'s words after the bar.
    pub fn parse(text: &str) -> Presets {
        let mut out = Presets::default();
        for line in text.lines() {
            let Some((k, v)) = line.split('#').next().and_then(|l| l.split_once('=')) else { continue };
            if k.trim() != "rider" {
                continue;
            }
            let (pick, look) = v.split_once('|').unwrap_or((v, ""));
            let mut p = Picks::parse(&format!("seat1 = {pick}")).0[0].clone();
            p.look = Look::parse(look);
            p.preset = Presets::key(&p);
            if !p.preset.is_empty() {
                out.put(&p);
            }
        }
        out
    }

    pub fn text(&self) -> String {
        let mut out = String::from("# openmx's riders (P86): rider = bike set gearset | outfit, helmet, goggles, gloves, boots, number, name\n");
        for p in &self.0 {
            out.push_str(&format!("rider = {} {} {} | {}\n", p.bike, p.set.word(), p.rider, p.look.text()));
        }
        out
    }

    pub fn load(dir: &Path) -> Presets {
        Presets::parse(&crate::files::read_to_string(dir.join(PRESETS_FILE)).unwrap_or_default())
    }

    /// Written whole and moved into place, as [`Picks::save`].
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join(PRESETS_FILE);
        let part = dir.join(format!("{PRESETS_FILE}.part"));
        crate::files::write(&part, self.text()).map_err(|e| format!("{}: {e}", part.display()))?;
        crate::files::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn find(&self, key: &str) -> Option<&Pick> {
        self.0.iter().find(|p| p.preset.eq_ignore_ascii_case(key.trim()))
    }

    /// Save a pick under its name and number, over the one saved there before. `None` when the pick
    /// has no name to save it under.
    pub fn put(&mut self, pick: &Pick) -> Option<String> {
        let key = Presets::key(pick);
        if key.is_empty() {
            return None;
        }
        let mut p = pick.clone();
        p.preset = key.clone();
        match self.0.iter_mut().find(|q| q.preset.eq_ignore_ascii_case(&key)) {
            Some(q) => *q = p,
            None => self.0.push(p),
        }
        Some(key)
    }

    /// Every preset's key, in the file's order.
    pub fn keys(&self) -> Vec<&str> {
        self.0.iter().map(|p| p.preset.as_str()).collect()
    }
}

/// The keys a name is typed on in a seat's column: the front end's keyboard's (P36).
pub const NAME_KEYS: [&str; 4] = ["abcdefghij", "klmnopqrst", "uvwxyz0123", "456789-'.&"];
pub const NAME_COMMANDS: [&str; 3] = ["Space", "Delete", "Done"];
/// And a number's, as the original's number pad (`mxfrontendnumentry.lyr`).
pub const NUMBER_KEYS: [&str; 2] = ["12345", "67890"];
pub const NUMBER_COMMANDS: [&str; 2] = ["Delete", "Done"];

/// A name or number being typed in one seat's column: the text so far and the key under the cursor,
/// the row past the keys being the commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub number: bool,
    pub text: String,
    pub key: (usize, usize),
}

impl Entry {
    pub fn keys(&self) -> &'static [&'static str] {
        if self.number { &NUMBER_KEYS } else { &NAME_KEYS }
    }

    pub fn commands(&self) -> &'static [&'static str] {
        if self.number { &NUMBER_COMMANDS } else { &NAME_COMMANDS }
    }

    fn width(&self, row: usize) -> usize {
        match self.keys().get(row) {
            Some(k) => k.len(),
            None => self.commands().len(),
        }
    }

    fn max(&self) -> usize {
        if self.number { NUMBER_MAX } else { RIDER_NAME_MAX }
    }

    /// Move the cursor: rows wrap through the command row, and into and out of it the column
    /// scales, as the front end's keyboard.
    pub fn walk(&mut self, rows: i32, cols: i32) {
        let (row, col) = self.key;
        let n = self.keys().len() + 1;
        if rows != 0 {
            let to = wrap(row, rows, n);
            let col = col * self.width(to) / self.width(row).max(1);
            self.key = (to, col.min(self.width(to) - 1));
        } else if cols != 0 {
            self.key.1 = wrap(col, cols, self.width(row));
        }
    }

    pub fn delete(&mut self) {
        self.text.pop();
    }

    /// Cross on the key under the cursor: type it, or do the command. True when it was Done.
    pub fn press(&mut self) -> bool {
        let (row, col) = self.key;
        let full = self.text.chars().count() >= self.max();
        match self.keys().get(row) {
            Some(keys) => {
                if !full {
                    let c = keys.chars().nth(col).unwrap_or(' ');
                    // The first letter of each word in capitals, as the front end's name entry.
                    let word_start = self.text.is_empty() || self.text.ends_with(' ');
                    self.text.push(if word_start { c.to_ascii_uppercase() } else { c });
                }
                false
            }
            None => match self.commands()[col] {
                "Space" => {
                    if !self.text.is_empty() && !full {
                        self.text.push(' ');
                    }
                    false
                }
                "Delete" => {
                    self.text.pop();
                    false
                }
                _ => true,
            },
        }
    }
}

/// What Cross did on a seat's garage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cross {
    /// Nothing on this row: the first seat leaves the garage with it.
    Leave,
    /// It typed, opened or closed an entry, or loaded a preset.
    Stay,
    /// The seat's look was saved as a preset.
    Saved,
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
    /// The rows of the original's custom rider screen below its Gear Set (P86), in its order: the
    /// jersey and pants, the name, the number, and the four pieces.
    Outfit,
    Name,
    Number,
    Piece(Piece),
    /// Load a saved look (left and right), or save this one (Cross).
    Preset,
}

pub const ROWS: [Row; 15] = [
    Row::Class,
    Row::Model,
    Row::Livery,
    Row::Tier,
    Row::Tuning,
    Row::Rider,
    Row::Colours,
    Row::Outfit,
    Row::Name,
    Row::Number,
    Row::Piece(Piece::Helmet),
    Row::Piece(Piece::Goggles),
    Row::Piece(Piece::Gloves),
    Row::Piece(Piece::Boots),
    Row::Preset,
];

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
    /// The name or number being typed, while it is (P86).
    pub entry: Option<Entry>,
}

impl Garage {
    pub fn new(catalogue: &Catalogue, pick: &Pick) -> Garage {
        Garage { row: 0, pick: catalogue.fit(pick), entry: None }
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
            Row::Outfit => {
                // The gear set's own first, then the file's list.
                let ids: Vec<&str> = std::iter::once("").chain(c.wardrobe.outfits.iter().map(|o| o.id.as_str())).collect();
                let at = ids.iter().position(|i| i.eq_ignore_ascii_case(&p.look.outfit)).unwrap_or(0);
                p.look.outfit = ids[wrap(at, by, ids.len())].to_string();
            }
            Row::Piece(piece) => {
                let list = &c.wardrobe.pieces[piece as usize];
                let mats: Vec<&str> = std::iter::once("").chain(list.iter().map(String::as_str)).collect();
                let now = p.look.piece(piece);
                let at = mats.iter().position(|m| m.eq_ignore_ascii_case(now)).unwrap_or(0);
                p.look.pieces[piece as usize] = mats[wrap(at, by, mats.len())].to_string();
            }
            Row::Name | Row::Number | Row::Preset => return,
        }
        // A new gear set is the whole set, as the original's Gear Set row puts every piece back.
        if matches!(ROWS[self.row], Row::Rider | Row::Colours) {
            let Look { name, number, .. } = std::mem::take(&mut p.look);
            p.look = Look { name, number, ..Look::default() };
        }
        *p = c.fit(p);
    }

    /// Left or right on the Preset row: the next saved look, loaded whole.
    pub fn turn_preset(&mut self, c: &Catalogue, presets: &Presets, by: i32) {
        let keys = presets.keys();
        if keys.is_empty() {
            return;
        }
        let at = keys.iter().position(|k| k.eq_ignore_ascii_case(&self.pick.preset));
        let next = match at {
            Some(i) => wrap(i, by, keys.len()),
            None if by < 0 => keys.len() - 1,
            None => 0,
        };
        if let Some(p) = presets.find(keys[next]) {
            self.pick = c.fit(p);
        }
    }

    /// Load a saved look by its name and number.
    pub fn load_preset(&mut self, c: &Catalogue, presets: &Presets, key: &str) -> bool {
        match presets.find(key) {
            Some(p) => {
                self.pick = c.fit(p);
                true
            }
            None => false,
        }
    }

    /// Cross on the current row: on Name and Number it opens the keys and types on them, on Preset
    /// it saves the look under its name and number. Anywhere else it is the first seat's way out.
    pub fn cross(&mut self, presets: &mut Presets) -> Cross {
        if let Some(e) = &mut self.entry {
            if e.press() {
                self.finish_entry();
            }
            return Cross::Stay;
        }
        match ROWS[self.row] {
            Row::Name => {
                self.entry = Some(Entry { number: false, text: self.pick.look.name.clone(), key: (0, 0) });
                Cross::Stay
            }
            Row::Number => {
                self.entry = Some(Entry { number: true, text: self.pick.look.number.clone(), key: (0, 0) });
                Cross::Stay
            }
            Row::Preset => match presets.put(&self.pick) {
                Some(key) => {
                    self.pick.preset = key;
                    Cross::Saved
                }
                None => Cross::Stay,
            },
            _ => Cross::Leave,
        }
    }

    /// The typed text into the look, and the keys closed.
    pub fn finish_entry(&mut self) {
        let Some(e) = self.entry.take() else { return };
        match e.number {
            true => self.pick.look.number = clean_number(&e.text),
            false => self.pick.look.name = clean_rider_name(&e.text),
        }
    }

    /// The keys closed and the text left as it was.
    pub fn cancel_entry(&mut self) {
        self.entry = None;
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
            Row::Outfit => {
                let id = if p.look.outfit.is_empty() { &p.rider } else { &p.look.outfit };
                c.wardrobe.outfit(id).map_or_else(|| "Own".to_string(), |o| o.label.clone())
            }
            Row::Name | Row::Number => {
                let typing = self.entry.as_ref().filter(|e| e.number == (row == Row::Number));
                match typing {
                    Some(e) => format!("{}_", e.text),
                    None => {
                        let v = if row == Row::Name { &p.look.name } else { &p.look.number };
                        if v.is_empty() { "None".into() } else { v.clone() }
                    }
                }
            }
            Row::Piece(piece) => match p.look.piece(piece) {
                "" => c.own(&p.rider, piece.word()).map_or_else(|| "Own".to_string(), |m| piece_label(&m)),
                m => piece_label(m),
            },
            Row::Preset => if p.preset.is_empty() { "None".into() } else { p.preset.clone() },
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
            wardrobe: Wardrobe::default(),
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
        assert_eq!(rider_of("fox2"), Some(Rider { archive: "fox2".into(), name: "fox".into(), colours: 2, suit: vec![] }));
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
        for _ in g.row..ROWS.len() {
            g.down();
        }
        assert_eq!(g.row, 0, "the rows wrap");
        assert_eq!(c.fit(&g.pick), g.pick);
    }

    #[test]
    fn fit_mends_a_pick_the_disc_cannot_ride() {
        let c = tiny();
        let odd = Pick { bike: "cr250_faisst".into(), rider: "nobody1".into(), ..Pick::default() };
        let fit = c.fit(&odd);
        assert_eq!(fit.set.engine, Engine::Cc250);
        assert_eq!(fit.rider, "thq1");
        assert_eq!(c.fit(&Pick { bike: "gone".into(), ..Pick::default() }).bike, "125sx");
    }

    /// A cut of `rider\gear.dat` and `bike\bikes.dat` in the disc's own shape.
    const GEAR: &str = "[Helmets]\nitem=THQ_Helmet_01\nitem=Fox_Helmet_02\n;item=Shoei_Helmet_04\nitem=Gone_Helmet_01\n\n\
        [Goggles]\nitem=Electric_Goggles_01\n\n[Gloves]\nitem=Fox_Gloves_02\n\n[Boots]\nitem=Sidi_Boots_01\n\n\
        [JerseyPants]\nitem=\"THQ1\"\nitem=\"Thor1\"\n\n\
        [THQ1]\nJerseyMaterial=THQ_JerseySleeves_01\nPantMaterial=THQ_Pants_01\nNameFont=name3\nNumberFont=number3\n\n\
        [Thor1]\nJerseyMaterial=Thor_JerseySleeves_01\nPantMaterial=Thor_Pants_01\nNameFont=name3_black\nNumberFont=number3_black\n\n\
        [Uniforms]\nitem=\"THQ1\"\n\n[THQ1]\n";
    const BIKES: &str = "[125cc]\nbike=\"125SX\"\n\n[125SX]\nNumberPlateFont=plate3\n\n[Fast50_01]\nNumberPlateFont=plate3_black\n";

    fn wardrobe() -> Wardrobe {
        Wardrobe::parse(GEAR, BIKES, &|m: &str| !m.starts_with("Gone"))
    }

    #[test]
    fn the_wardrobe_is_the_gear_files_lists_the_custom_archive_can_paint() {
        let w = wardrobe();
        assert_eq!(w.pieces[Piece::Helmet as usize], ["THQ_Helmet_01", "Fox_Helmet_02"]);
        assert_eq!(w.outfits.len(), 2);
        assert_eq!(w.outfits[1].id, "thor1");
        assert_eq!(w.fonts("thor1", &Look::default()), ("name3_black".into(), "number3_black".into()));
        assert_eq!(w.fonts("parsons1", &Look::default()), (NAME_FONT.into(), NUMBER_FONT.into()));
        let thor = Look { outfit: "thor1".into(), ..Look::default() };
        assert_eq!(w.fonts("parsons1", &thor).0, "name3_black");
        assert_eq!(w.plate_font("fast50_01"), "plate3_black");
        assert_eq!(w.plate_font("cr125_faisst"), PLATE_FONT);
        // The repaint goes on the body's own base names, after the gear set's.
        let base: Vec<String> = ["THQ_Helmet_01", "THQ_JerseySleeves_01", "THQ_Pants_01", "THQ_Boots_01"].map(String::from).to_vec();
        let mut look = thor.clone();
        look.pieces[Piece::Helmet as usize] = "Fox_Helmet_02".into();
        let pairs = w.materials(&look, &base);
        assert!(pairs.contains(&("THQ_Helmet_01".into(), "Fox_Helmet_02".into())));
        assert!(pairs.contains(&("THQ_JerseySleeves_01".into(), "Thor_JerseySleeves_01".into())));
        assert!(pairs.contains(&("THQ_Pants_01".into(), "Thor_Pants_01".into())));
        assert_eq!(pairs.len(), 3);
        assert_eq!(piece_label("O'neal_Helmet_02"), "O'neal 2");
    }

    #[test]
    fn a_look_and_its_preset_read_back_as_written() {
        let mut picks = Picks::default();
        picks.0[1].look = Look {
            outfit: "thor1".into(),
            pieces: ["Fox_Helmet_02".into(), String::new(), "O'neal_Gloves_01".into(), String::new()],
            name: "Mc Grath, Jr".into(),
            number: "7".into(),
        };
        picks.0[1].look.name = clean_rider_name(&picks.0[1].look.name);
        picks.0[1].preset = "Mc Grath Jr 7".into();
        let text = picks.text();
        assert!(text.starts_with("# openmx's garage"));
        assert!(!text.contains("seat1.look"), "an untouched seat writes the old line alone");
        assert_eq!(Picks::parse(&text), picks);
        assert_eq!(Look::parse(&picks.0[1].look.text()), picks.0[1].look);
        // An older file still opens, and a number keeps only its digits.
        assert_eq!(Picks::parse("seat1 = 125sx race125p thq1\n"), Picks::default());
        assert_eq!(Look::parse(", , , , , 12x34, Ann").number, "123");

        let mut presets = Presets::default();
        let mut p = Pick::default();
        assert_eq!(presets.put(&p), None, "a look with no name is not saved");
        p.look.name = "Ann".into();
        p.look.number = "42".into();
        assert_eq!(presets.put(&p).as_deref(), Some("Ann 42"));
        p.bike = "cr125_faisst".into();
        presets.put(&p);
        assert_eq!(presets.0.len(), 1, "the same name and number saves over");
        p.look.number = "5".into();
        presets.put(&p);
        let back = Presets::parse(&presets.text());
        assert_eq!(back, presets);
        assert_eq!(back.find("ann 42").map(|p| p.bike.as_str()), Some("cr125_faisst"));
    }

    #[test]
    fn a_seat_types_its_name_and_number_and_saves_and_loads_its_look() {
        let mut c = tiny();
        c.wardrobe = wardrobe();
        let mut presets = Presets::default();
        let mut g = Garage::new(&c, &Pick::default());
        let row = |r: Row| ROWS.iter().position(|x| *x == r).unwrap();
        g.row = row(Row::Piece(Piece::Helmet));
        g.turn(&c, 1);
        assert_eq!(g.pick.look.piece(Piece::Helmet), "THQ_Helmet_01");
        g.turn(&c, 1);
        assert_eq!(g.value(&c, Row::Piece(Piece::Helmet)), "Fox 2");
        g.row = row(Row::Name);
        assert_eq!(g.cross(&mut presets), Cross::Stay);
        // b, e, then Done on the command row.
        g.entry.as_mut().unwrap().walk(0, 1);
        g.cross(&mut presets);
        g.entry.as_mut().unwrap().walk(0, 3);
        g.cross(&mut presets);
        assert_eq!(g.value(&c, Row::Name), "Be_");
        let e = g.entry.as_mut().unwrap();
        e.walk(-1, 0);
        e.walk(0, 1);
        assert_eq!(e.key, (4, 2));
        g.cross(&mut presets);
        assert_eq!((g.entry.is_none(), g.pick.look.name.as_str()), (true, "Be"));
        g.row = row(Row::Number);
        g.cross(&mut presets);
        g.entry.as_mut().unwrap().walk(0, -1);
        g.cross(&mut presets);
        g.finish_entry();
        assert_eq!(g.pick.look.number, "5");
        g.row = row(Row::Preset);
        assert_eq!(g.cross(&mut presets), Cross::Saved);
        assert_eq!(g.pick.preset, "Be 5");
        // Another seat loads it whole.
        let mut h = Garage::new(&c, &Pick::default());
        h.row = row(Row::Preset);
        h.turn_preset(&c, &presets, 1);
        assert_eq!(h.pick, g.pick);
        // A new gear set puts the pieces back and keeps who it is.
        h.row = row(Row::Colours);
        h.turn(&c, 1);
        assert_eq!(h.pick.look, Look { name: "Be".into(), number: "5".into(), ..Look::default() });
    }
}
