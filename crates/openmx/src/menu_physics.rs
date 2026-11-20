//! P387's physics editor page: the ridden bike's `.phy` as knobs under MXA's groups, a handling mod
//! saved from them and the ones saved before loaded back. The page only shows and asks: the shell
//! owns the file (`phy_editor.rs`), fills [`Panel`] from it and takes [`Ask`]s off it, so the menu
//! holds no physics of its own.

use super::*;

/// One knob as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub group: &'static str,
    pub label: &'static str,
    pub about: &'static str,
    /// Its scale, 100 the file's.
    pub pct: u16,
    /// The file's first value under it, as edited; none where the bike's file has none of it.
    pub value: Option<String>,
}

/// What the page asked of the shell, taken with [`Menu::take_physics`].
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// A knob up or down a step.
    Turn(usize, bool),
    /// A knob back to the file's.
    Stock(usize),
    /// Every knob back.
    Reset,
    /// The knobs saved as a handling mod under this name.
    Save(String),
    /// The saved mod the Load row is on, by its place in [`Panel::saved`].
    Load(usize),
}

/// The page's state, which the shell fills.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panel {
    /// The bike's file, `race125p.phy`; empty with nothing to edit.
    pub bike: String,
    pub knobs: Vec<Shown>,
    /// Why nothing can be turned now: no ride, another handling, an online ride.
    pub held: Option<String>,
    /// The handling mods saved for this bike, by name.
    pub saved: Vec<String>,
    /// The last thing done, for the side panel: saved, loaded, or why not.
    pub said: Option<String>,
    /// The saved mod the Load row is on.
    at: usize,
    asks: Vec<Ask>,
}

const TAKES: &str = "100% is the bike's own file. The ride takes a change on its next tick, from where it is.";

impl Menu {
    /// Open the physics editor page on its first knob, from the Ride tab.
    pub(in super::super) fn open_physics(&mut self) {
        self.push(Screen::Physics);
        let first = self.physics_rows().iter().position(Row::selectable).unwrap_or(0);
        self.cursor[Screen::Physics.index()] = first;
    }

    /// What the page asked since the last time, oldest first.
    pub fn take_physics(&mut self) -> Vec<Ask> {
        std::mem::take(&mut self.physics.asks)
    }

    /// The Ride tab's two rows: the switch, and with it on the row into the page.
    pub(super) fn physics_switch_rows(&self) -> Vec<Row> {
        let on = self.settings.physics_editor;
        let mut rows = vec![Row::on_off(
            Key::PhysicsEditor,
            "Physics Editor",
            on,
            "Edit the bike's physics file as you ride it: grip, suspension, engine, gears, weight and the rest, saved as handling mods. Off is the original game. An edited bike can only ride online where every rider has the same handling mod.",
        )];
        if on {
            let mut open = Row::new(Key::PhysicsPage, Kind::Opens, "Edit Physics", "The ridden bike's knobs. Cross opens them.");
            let changed = self.physics.knobs.iter().filter(|k| k.pct != 100).count();
            open.value = match changed {
                0 => "Stock".into(),
                n => format!("{n} Changed"),
            };
            rows.push(open);
        }
        rows
    }

    /// The page's rows: each group's knobs under its heading, then the handling mods.
    pub(super) fn physics_rows(&self) -> Vec<Row> {
        let p = &self.physics;
        let held = p.held.as_deref();
        let mut rows = Vec::new();
        let mut group = "";
        for (i, k) in p.knobs.iter().enumerate() {
            if k.group != group {
                group = k.group;
                rows.push(Row::heading(group));
            }
            let value = match &k.value {
                Some(v) => format!("{}%  {v}", k.pct),
                None => format!("{}%", k.pct),
            };
            let meter = f32::from(k.pct) / 200.0;
            let mut r = Row::slider(Key::PhyKnob(i), k.label, value, meter, k.about);
            r.note = Some(TAKES.into());
            rows.push(r.off_if(k.value.is_none(), "This bike's file has none of it").off_if(held.is_some(), held.unwrap_or("")));
        }
        rows.push(Row::heading("Handling Mods"));
        let none = p.knobs.is_empty();
        let mut save = Row::action(Key::PhySave, "Save as Mod", "Name and keep these knobs as a handling mod in the mods folder, for this bike. A lobby rides it only where every rider has it.");
        save.note = p.said.clone();
        rows.push(save.off_if(none, held.unwrap_or("Nothing to save")));
        let names: Vec<&str> = p.saved.iter().map(String::as_str).collect();
        let mut load = if names.is_empty() {
            Row::action(Key::PhyLoad, "Load Mod", "Put a saved handling mod's knobs on this bike.").off_if(true, "None saved for this bike")
        } else {
            Row::choice(Key::PhyLoad, "Load Mod", &names, p.at.min(names.len() - 1), "Left and right pick a saved handling mod; Cross puts its knobs on this bike.")
        };
        load.note = Some(format!("handling/ in the mods folder{}", if p.bike.is_empty() { String::new() } else { format!(", {}", p.bike) }));
        rows.push(load.off_if(held.is_some(), held.unwrap_or("")));
        rows.push(Row::action(Key::PhyReset, "Reset All", "Every knob back to the bike's own file.").off_if(none || held.is_some(), held.unwrap_or("Nothing to reset")));
        rows
    }

    /// A press on one of the page's rows (or the Ride tab's switch rows); false where the row is not
    /// one of them.
    pub(super) fn change_physics(&mut self, row: &Row, p: Press) -> bool {
        match row.key {
            Key::PhysicsEditor => {
                self.settings.physics_editor = !self.settings.physics_editor;
                self.changed = true;
            }
            Key::PhysicsPage if p.ok => self.open_physics(),
            Key::PhyKnob(i) if p.left || p.right => self.physics.asks.push(Ask::Turn(i, p.right)),
            // Cross puts the one knob back, as on the modifiers page.
            Key::PhyKnob(i) if p.ok => self.physics.asks.push(Ask::Stock(i)),
            Key::PhySave if p.ok => self.type_for(Typing::Physics),
            Key::PhyLoad if (p.left || p.right) && !self.physics.saved.is_empty() => {
                self.physics.at = cycle(self.physics.at, by(p), self.physics.saved.len());
            }
            Key::PhyLoad if p.ok && !self.physics.saved.is_empty() => {
                let at = self.physics.at.min(self.physics.saved.len() - 1);
                self.physics.asks.push(Ask::Load(at));
            }
            Key::PhyReset if p.ok => self.physics.asks.push(Ask::Reset),
            Key::PhysicsPage | Key::PhyKnob(_) | Key::PhySave | Key::PhyLoad | Key::PhyReset => {}
            _ => return false,
        }
        true
    }

    /// The keyboard's name for Save as Mod, typed.
    pub(in super::super) fn finish_physics_name(&mut self, name: &str) {
        if !name.is_empty() {
            self.physics.asks.push(Ask::Save(name.to_string()));
        }
    }
}
