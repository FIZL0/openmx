//! Each seat's camera presets (P163): which preset the camera button last left the seat's bike and
//! biplane on, the numbers a single preset has of its own, and the player's own presets, which copy
//! one of the disc's under a name of their own and join the camera button's cycle beside the disc's
//! or in their place. The menu knows names and numbers only; the shell lays them over the disc's
//! presets (`chase::seat_presets`), and a seat with none of them rides the Camera page's one set
//! over every preset, as before (P82).

use super::{CAMERA_FOV, CAMERA_SHARE, CameraView, Machine, SEATS_MAX, clean_name};

/// One preset's three sliders: the field of view's offset from the disc's, in degrees, and how far
/// behind and how high above its machine the camera sits, in percent of the disc's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub fov: i16,
    pub distance: u16,
    pub height: u16,
}

impl Look {
    /// The disc's numbers.
    pub const ORIGINAL: Look = Look { fov: 0, distance: 100, height: 100 };

    /// A seat's one set over every preset, as it frames `machine`'s.
    pub fn of(view: &CameraView, machine: Machine) -> Look {
        match machine {
            Machine::Bike => Look { fov: view.fov, distance: view.distance, height: view.height },
            Machine::Biplane => Look { fov: view.fov, distance: view.plane_distance, height: view.plane_height },
        }
    }

    /// Slider `slider` (field of view, distance, height) as [`CameraView::get`] numbers it.
    pub fn get(&self, slider: usize) -> i32 {
        match slider {
            0 => self.fov as i32,
            1 => self.distance as i32,
            _ => self.height as i32,
        }
    }

    fn set(&mut self, slider: usize, v: i32) {
        let share = |v: i32| v.clamp(CAMERA_SHARE.0 as i32, CAMERA_SHARE.1 as i32) as u16;
        match slider {
            0 => self.fov = v.clamp(CAMERA_FOV.0 as i32, CAMERA_FOV.1 as i32) as i16,
            1 => self.distance = share(v),
            _ => self.height = share(v),
        }
    }

    /// Slider `slider` a step left or right, held at its ends; with neither, back to the disc's.
    pub fn step(&mut self, slider: usize, left: bool, right: bool) {
        let step = if slider == 0 { 1 } else { super::CAMERA_STEP as i32 };
        let v = self.get(slider);
        let to = match (left, right) {
            (true, _) => v - step,
            (_, true) => v + step,
            _ => Look::ORIGINAL.get(slider),
        };
        self.set(slider, to);
    }

    /// A slider's value as its row shows it: `Original` on the disc's notch.
    pub fn label(&self, slider: usize) -> String {
        let v = self.get(slider);
        match slider {
            _ if v == Look::ORIGINAL.get(slider) => "Original".to_string(),
            0 => format!("{v:+} deg"),
            _ => format!("{v}%"),
        }
    }

    /// As the shell frames a camera: (degrees added, distance share, height share).
    pub fn framing(&self) -> (f32, f32, f32) {
        (self.fov as f32, self.distance as f32 / 100.0, self.height as f32 / 100.0)
    }

    fn text(&self) -> String {
        format!("{} {} {}", self.fov, self.distance, self.height)
    }

    fn parse(v: &str) -> Option<Look> {
        let n: Vec<i32> = v.split_whitespace().map(|w| w.parse::<i32>().ok()).collect::<Option<_>>()?;
        let [fov, distance, height] = n[..] else { return None };
        let mut l = Look::ORIGINAL;
        l.set(0, fov);
        l.set(1, distance);
        l.set(2, height);
        Some(l)
    }
}

/// What the camera button cycles through: the disc's presets alone, the disc's and then the
/// player's own, or the player's own in place of the disc's (the disc's when there are none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cycle {
    Disc,
    #[default]
    Both,
    Mine,
}

impl Cycle {
    pub const ALL: [Cycle; 3] = [Cycle::Disc, Cycle::Both, Cycle::Mine];

    pub fn word(self) -> &'static str {
        match self {
            Cycle::Disc => "disc",
            Cycle::Both => "both",
            Cycle::Mine => "mine",
        }
    }

    pub fn from_word(w: &str) -> Option<Cycle> {
        Cycle::ALL.into_iter().find(|c| c.word() == w)
    }

    pub fn label(self) -> &'static str {
        match self {
            Cycle::Disc => "Disc's",
            Cycle::Both => "Disc's and Mine",
            Cycle::Mine => "Mine Only",
        }
    }
}

/// One preset of a seat's own: `base` is the disc preset it copies, by the name its file gives it.
/// Named as `base`, it is that disc preset's own numbers; named otherwise, it is a preset of the
/// player's, which the camera button reaches under its own name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Own {
    pub machine: Machine,
    pub base: String,
    pub name: String,
    pub look: Look,
}

impl Own {
    /// Whether it is a preset of the player's rather than a disc preset's own numbers.
    pub fn custom(&self) -> bool {
        !self.name.eq_ignore_ascii_case(&self.base)
    }
}

/// One seat's presets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seat {
    /// The disc presets' own numbers and the player's own presets, in the order they were saved.
    pub own: Vec<Own>,
    pub cycle: Cycle,
    /// The preset the camera button last left the bike's camera and the biplane's on, by name;
    /// empty for the shell's own start.
    pub start: [String; 2],
}

impl Seat {
    /// The seat's presets but where its rides start, which is all the shell frames by.
    pub fn framing_part(&self) -> (Vec<Own>, Cycle) {
        (self.own.clone(), self.cycle)
    }

    /// A disc preset's own numbers, if it has some.
    pub fn own_of(&self, machine: Machine, name: &str) -> Option<&Own> {
        self.own.iter().find(|o| o.machine == machine && !o.custom() && o.base.eq_ignore_ascii_case(name))
    }

    /// The player's own presets on `machine`, in the order they were saved.
    pub fn customs(&self, machine: Machine) -> impl Iterator<Item = &Own> {
        self.own.iter().filter(move |o| o.machine == machine && o.custom())
    }

    /// The numbers `name` (a disc preset of `machine`'s, or one of the player's) is framed by:
    /// its own, or else `view`'s one set over every preset.
    pub fn look(&self, view: &CameraView, machine: Machine, name: &str) -> Look {
        match self.own.iter().find(|o| o.machine == machine && o.name.eq_ignore_ascii_case(name)) {
            Some(o) => o.look,
            None => Look::of(view, machine),
        }
    }

    /// Set the numbers `name` has of its own: a disc preset's, made its own on its first change.
    pub fn set_look(&mut self, machine: Machine, name: &str, look: Look) {
        match self.own.iter_mut().find(|o| o.machine == machine && o.name.eq_ignore_ascii_case(name)) {
            Some(o) => o.look = look,
            None => self.own.push(Own { machine, base: name.to_string(), name: name.to_string(), look }),
        }
    }

    /// Keep `look` over `base` as a preset of the player's called `name`, over one already called
    /// that; a name a disc preset of the machine already has is taken with ` Mine` after it. The
    /// cycle takes the player's presets if it did not. The name it was kept under.
    pub fn save_custom(&mut self, machine: Machine, base: &str, name: &str, look: Look, disc: &[String]) -> String {
        let mut name = name.to_string();
        if disc.iter().any(|d| d.eq_ignore_ascii_case(&name)) || name.eq_ignore_ascii_case(base) {
            name += " Mine";
        }
        match self.own.iter_mut().find(|o| o.machine == machine && o.custom() && o.name.eq_ignore_ascii_case(&name)) {
            Some(o) => {
                o.base = base.to_string();
                o.look = look;
            }
            None => self.own.push(Own { machine, base: base.to_string(), name: name.clone(), look }),
        }
        if self.cycle == Cycle::Disc {
            self.cycle = Cycle::Both;
        }
        name
    }

    /// Forget a preset's own numbers or a preset of the player's.
    pub fn remove(&mut self, machine: Machine, name: &str) {
        self.own.retain(|o| !(o.machine == machine && o.name.eq_ignore_ascii_case(name)));
    }

    /// Forget every disc preset's own numbers, keeping the player's own presets.
    pub fn clear_disc_looks(&mut self) {
        self.own.retain(Own::custom);
    }

    /// Whether the camera button and every disc preset are the disc's: no disc preset with numbers
    /// of its own, and the player's presets not in the cycle.
    pub fn is_disc(&self) -> bool {
        self.own.iter().all(Own::custom) && (self.cycle == Cycle::Disc || self.own.is_empty())
    }
}

/// A seat's presets on `machine` that the shell lays over its camera: every disc preset's own
/// numbers, and the player's own presets unless the camera button is on the disc's alone.
pub fn laid(own: &[Own], cycle: Cycle, machine: Machine) -> impl Iterator<Item = &Own> {
    own.iter().filter(move |o| o.machine == machine && (!o.custom() || cycle != Cycle::Disc))
}

/// Every seat's presets as `settings.txt` has them, one line a thing and nothing for a seat at the
/// default, so a file from before them is written as it was. Seats count from 1.
pub fn text(seats: &[Seat; SEATS_MAX]) -> String {
    let mut out = String::new();
    for (i, s) in seats.iter().enumerate() {
        let n = i + 1;
        if s.start.iter().any(|w| !w.is_empty()) {
            out += &format!("camera_start = {n} | {} | {}\n", s.start[0], s.start[1]);
        }
        if s.cycle != Cycle::default() {
            out += &format!("camera_cycle = {n} | {}\n", s.cycle.word());
        }
        for o in &s.own {
            out += &format!("camera_preset = {n} | {} | {} | {} | {}\n", o.machine.word(), o.base, o.name, o.look.text());
        }
    }
    out
}

/// One `settings.txt` line of [`text`]'s onto `seats`; a line it cannot read changes nothing.
pub fn parse(seats: &mut [Seat; SEATS_MAX], key: &str, v: &str) {
    let parts: Vec<&str> = v.split('|').map(str::trim).collect();
    let Some(seat) = parts.first().and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=SEATS_MAX).contains(n)) else { return };
    let s = &mut seats[seat - 1];
    // A disc preset's name is the file's, which the keyboard's letters need not cover; `#` and `|`
    // cannot be in one, since the file's own reader would have cut it there.
    let disc_name = |n: &str| n.chars().filter(|c| !c.is_control()).collect::<String>();
    match (key, &parts[1..]) {
        ("camera_start", [bike, plane]) => s.start = [disc_name(bike), disc_name(plane)],
        ("camera_cycle", [w]) => s.cycle = Cycle::from_word(w).unwrap_or(s.cycle),
        ("camera_preset", [machine, base, name, look]) => {
            let (Some(machine), Some(look)) = (Machine::from_word(machine), Look::parse(look)) else { return };
            let base = disc_name(base);
            // A player's preset's name is the keyboard's; a disc preset's own numbers keep its name.
            let name = if name.eq_ignore_ascii_case(&base) { base.clone() } else { clean_name(name) };
            if base.is_empty() || name.is_empty() {
                return;
            }
            s.remove(machine, &name);
            s.own.push(Own { machine, base, name, look });
        }
        _ => {}
    }
}

/// A preset the Camera page's Preset row can be on: its machine, its name, and whether it is one of
/// the player's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub machine: Machine,
    pub name: String,
    pub custom: bool,
}

impl Pick {
    /// The row's word for it: the biplane's named so, the player's own marked.
    pub fn label(&self) -> String {
        let machine = if self.machine == Machine::Biplane { "Biplane: " } else { "" };
        let mine = if self.custom { " (Mine)" } else { "" };
        format!("{machine}{}{mine}", self.name)
    }
}

impl super::Menu {
    /// The seat the Camera page's rows are.
    pub(super) fn camera_seat(&self) -> usize {
        self.player.min(SEATS_MAX - 1)
    }

    /// The presets the Preset row offers past every preset's: the disc's that cycle, the bike's then
    /// the biplane's, and the seat's own after them.
    pub fn camera_choices(&self) -> Vec<Pick> {
        let set = &self.settings.camera_sets[self.camera_seat()];
        let mut out = Vec::new();
        for (m, names) in Machine::ALL.into_iter().zip(&self.camera_names) {
            out.extend(names.iter().map(|n| Pick { machine: m, name: n.clone(), custom: false }));
        }
        for m in Machine::ALL {
            out.extend(set.customs(m).map(|o| Pick { machine: m, name: o.name.clone(), custom: true }));
        }
        out
    }

    /// The preset the Preset row is on, `None` for every preset's.
    pub fn camera_picked(&self) -> Option<Pick> {
        self.camera_pick.checked_sub(1).and_then(|i| self.camera_choices().get(i).cloned())
    }

    /// The Preset row a step on, round from either end.
    pub(super) fn step_camera_pick(&mut self, by: i32) {
        let n = self.camera_choices().len() + 1;
        self.camera_pick = (self.camera_pick.min(n - 1) as i64 + by as i64).rem_euclid(n as i64) as usize;
    }

    /// The numbers the picked preset is framed by now.
    pub fn camera_look(&self, pick: &Pick) -> Look {
        let seat = self.camera_seat();
        self.settings.camera_sets[seat].look(&self.settings.cameras[seat], pick.machine, &pick.name)
    }

    /// A slider of the picked preset's a step, which makes a disc preset's numbers its own.
    pub(super) fn step_camera_look(&mut self, pick: &Pick, slider: usize, left: bool, right: bool) {
        let mut look = self.camera_look(pick);
        look.step(slider, left, right);
        let seat = self.camera_seat();
        self.settings.camera_sets[seat].set_look(pick.machine, &pick.name, look);
    }

    /// Keep the picked preset as it is framed now as one of the player's called `name`, and put
    /// the Preset row on it.
    pub(super) fn save_camera(&mut self, name: &str) {
        let Some(pick) = self.camera_picked() else { return };
        let seat = self.camera_seat();
        let set = &self.settings.camera_sets[seat];
        let base = match pick.custom {
            true => set.customs(pick.machine).find(|o| o.name == pick.name).map_or(pick.name.clone(), |o| o.base.clone()),
            false => pick.name.clone(),
        };
        let look = self.camera_look(&pick);
        let disc = match pick.machine {
            Machine::Bike => &self.camera_names[0],
            Machine::Biplane => &self.camera_names[1],
        };
        let kept = self.settings.camera_sets[seat].save_custom(pick.machine, &base, name, look, disc);
        if let Some(i) = self.camera_choices().iter().position(|p| p.custom && p.machine == pick.machine && p.name == kept) {
            self.camera_pick = i + 1;
        }
        self.changed = true;
    }

    /// The shell's word that the camera button left `seat`'s `machine` on preset `name` (P163): the
    /// one the seat's next ride on it starts on. Whether that changed anything to save.
    pub fn camera_left_on(&mut self, seat: usize, machine: Machine, name: &str) -> bool {
        let Some(set) = self.settings.camera_sets.get_mut(seat) else { return false };
        let at = &mut set.start[usize::from(machine == Machine::Biplane)];
        if *at == name {
            return false;
        }
        *at = name.to_string();
        self.changed = true;
        true
    }
}
