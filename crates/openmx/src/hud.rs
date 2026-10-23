//! The riding overlay: the pad as the simulation reads it, and the numbers that say whether the ride
//! is doing what you asked.
//!
//! This is the shell's own debugging furniture. It matches **nothing on the disc** — the original's
//! own HUD is a front-end page with its own art, and none of it is ported here — so there is nothing
//! to be 1:1 with and nothing to prove against a capture. What it has to be is honest about the one
//! thing it shows: every value is read straight off the port, never recomputed, so a wrong number on
//! the screen is a wrong number in the simulation.
//!
//! Why it is worth having at all: a person riding cannot otherwise tell a dead key from a bike that
//! ignores the key. The sixteen controls are drawn as [`game_sim::input::Pad`] holds them — held,
//! pressed this frame, and how hard for the four analogue ones — and the two sticks are drawn twice
//! over, once where the report byte is and once where `input.rs`'s dead zone and two segments put it.
//! A key that never arrives moves nothing; a key inside the dead zone moves the dot and not the cross
//! the game reads; a bike that ignores a control you can see held is the bike's fault.
//!
//! The commands beside them are the other half of the same question: the three ramps
//! ([`game_sim::ride::Held`]) are what the bike is actually holding, against what the pad asked for
//! this frame, so a throttle that is still coming up reads as a bar short of its marker rather than as
//! a bike that will not go.
//!
//! # How it draws
//!
//! The cheapest way the rasteriser allows, which is not to use it: this is a 2-D blit straight into
//! [`Frame::color`] after the scene is drawn, with no triangles, no depth test and no texture. The
//! font is a 3×5 bitmap, five bytes a glyph, scaled by whole pixels. A frame of the whole overlay
//! costs tens of microseconds at 640×448 (the test at the bottom of this file holds it under a tenth
//! of a millisecond), and with the flag off it is one `if` and no pixels at all.

use game_render::raster::Frame;
use game_sim::bindings::Table;
use game_sim::input::{AXIS_FULL, Axis, AxisRange, Control, Pad, PlaneCommand, PlaneInput};
use game_sim::markers::{Pick, SLOTS};
use game_sim::ride::Controls;
use game_sim::rider::Rider;
use game_sim::state::Phase;
use game_sim::trick::Wreck;
use std::time::{Duration, Instant};

/// The overlay, and the two things about the ride that are events rather than state: the last trick
/// and the last wreck. Everything else is read off the port at draw time.
pub struct Hud {
    /// Whether to draw at all. `--hud` sets it and H toggles it; off, [`Hud::draw`] returns before
    /// it touches a pixel.
    pub on: bool,
    /// Whether the rider is in the biplane: the control list is then the plane's eight commands over
    /// the controls they are bound to, and the ramps beside it are the plane's five numbers. The shell
    /// sets it every frame from which machine is up.
    pub plane: bool,
    /// The last trick the rider started or landed, as the stunt table spells it — the shell's own
    /// line, because the table lives in the shell and not in the overlay.
    trick: String,
    /// The last wreck, as the state machine's own reason code and this file's name for it.
    wreck: String,
    /// The camera preset the follow camera is on, by its file's name for it.
    camera: String,
    /// Which machine has the controller, the bike or the biplane (P19). With the plane up the
    /// numbers beside it are the bike's — where it was left — but for [`Hud::flight`]'s two.
    vehicle: String,
    /// The plane's throttle (`0..1`) and airspeed (feet a second), as its flight holds them, while
    /// the plane is up and flying (P31). The first column's top two lines show these instead of the
    /// bike's speed and engine.
    pub flight: Option<(f32, f32)>,
    /// The binding table the bike's commands are read through, the seat's controller preset (P79):
    /// the shell sets it every frame from [`game_sim::Sim::table`].
    pub bindings: Table,
    /// The settings' HUD scale (V52), 1.0 the panel's own size: the panel grows or shrinks about the
    /// view's bottom-left corner, and grows no larger than the view holds.
    pub scale: f32,
}

impl Hud {
    pub fn new(on: bool) -> Hud {
        Hud {
            on,
            plane: false,
            trick: "-".to_string(),
            wreck: "-".to_string(),
            camera: "-".to_string(),
            vehicle: "bike".to_string(),
            flight: None,
            bindings: Table::SHIPPED,
            scale: 1.0,
        }
    }

    /// Which camera preset is up, latched the same way: the shell switches presets, not the overlay.
    pub fn note_camera(&mut self, name: &str) {
        self.camera = name.to_string();
    }

    /// Which machine is up, latched the same way: the shell swaps machines, not the overlay.
    pub fn note_vehicle(&mut self, name: &str) {
        self.vehicle = name.to_string();
    }

    /// What the rider just did, latched for the overlay to show until the next one.
    pub fn note_trick(&mut self, what: &str) {
        self.trick = what.to_string();
    }

    /// Why the rider came off, latched the same way. The reason is the state machine's own code; the
    /// name beside it is this file's, for the five codes the port has named.
    pub fn note_wreck(&mut self, why: Wreck) {
        self.wreck = match wreck_name(why) {
            Some(name) => format!("{} {}", why.0, name),
            None => format!("{}", why.0),
        };
    }

    /// One frame of the overlay, over a frame the scene is already drawn into.
    ///
    /// `riding` is the shell's freeze, which is not a state of the game's machine — a frozen ride is
    /// still `Riding` as far as [`Phase`] is concerned — so it is shown beside the state and not as
    /// one.
    pub fn draw(&self, frame: &mut Frame, pad: &Pad, rider: &Rider, riding: bool) {
        if !self.on {
            return;
        }
        let fit = (frame.width as f32 / (PANEL_W + 2 * PAD) as f32).min(frame.height as f32 / (PANEL_H + 2 * PAD) as f32);
        let k = if self.scale > 1.0 { self.scale.min(fit).max(1.0) } else { self.scale.max(0.1) };
        if k == 1.0 {
            let (x, y) = panel_at(frame.width, frame.height);
            self.body(frame, x, y, pad, rider, riding);
            return;
        }
        // Another size (V52): the panel at its own into a black frame of its size, then over the
        // view's corner at `k`, nearest pixel — the scene darkened under all of it, and the
        // panel's ink, never black, wherever it drew.
        let mut own = Frame::new(PANEL_W as usize, PANEL_H as usize, [0; 3]);
        self.body(&mut own, 0, 0, pad, rider, riding);
        let (w, h) = ((PANEL_W as f32 * k).round() as i32, (PANEL_H as f32 * k).round() as i32);
        let (x, y) = crate::aspect::corner((frame.width, frame.height), (w, h), crate::aspect::Corner::BottomLeft, PAD);
        panel(frame, x, y, w, h);
        let (fw, fh) = (frame.width as i32, frame.height as i32);
        for py in y.max(0)..(y + h).min(fh) {
            let sy = ((py - y) * PANEL_H / h).min(PANEL_H - 1) as usize;
            for px in x.max(0)..(x + w).min(fw) {
                let sx = ((px - x) * PANEL_W / w).min(PANEL_W - 1) as usize;
                let from = (sy * own.width + sx) * 3;
                let ink = &own.color[from..from + 3];
                if ink != [0; 3] {
                    let to = (py as usize * frame.width + px as usize) * 3;
                    frame.color[to..to + 3].copy_from_slice(ink);
                }
            }
        }
    }

    /// The panel with its top-left at `x`, `y`.
    fn body(&self, frame: &mut Frame, x: i32, y: i32, pad: &Pad, rider: &Rider, riding: bool) {
        panel(frame, x, y, PANEL_W, PANEL_H);

        // Three bands down the panel: the numbers, the sixteen controls, and the sticks with the
        // three commands that ramp beside them.
        let (x, y) = (x + PAD, y + PAD);
        self.numbers(frame, x, y, rider, riding);
        if self.plane {
            self.plane_controls(frame, x, y + CONTROLS_Y, pad);
        } else {
            self.controls(frame, x, y + CONTROLS_Y, pad);
        }
        self.sticks(frame, x, y + STICKS_Y, pad);
        if self.plane {
            self.plane_commands(frame, x + 2 * (STICK + 6), y + STICKS_Y, pad);
        } else {
            self.commands(frame, x + 2 * (STICK + 6), y + STICKS_Y, pad, rider);
        }
    }

    /// The sixteen controls, in the order the game's own map holds them — which is the order the
    /// input layer queues events in, so it is the order worth reading them in.
    ///
    /// A cell's fill is how hard the control is held: full for a digital one, the pressure byte's own
    /// strength for the four analogue ones, so a trigger half down reads as half a cell. A cell with
    /// a bright border went down *this frame*; a released one is not marked, because nothing in the
    /// ride reads a release.
    fn controls(&self, frame: &mut Frame, x: i32, y: i32, pad: &Pad) {
        for (i, control) in Control::ALL.iter().enumerate() {
            let (col, row) = (i % 8, i / 8);
            let (cx, cy) = (x + col as i32 * (CELL_W + 2), y + row as i32 * (CELL_H + 2));
            let held = pad.held(*control);
            fill(frame, cx, cy, CELL_W, CELL_H, EMPTY);
            if held {
                // `strength` is the port's own: 1.0 for a digital control, (pressure + 1) / 256 for
                // an analogue one. A held control always shows at least a sliver.
                let strength = pad.strength(*control).0.clamp(0.0, 1.0);
                let w = ((CELL_W - 2) as f32 * strength).round().max(1.0) as i32;
                fill(frame, cx + 1, cy + 1, w, CELL_H - 2, HELD);
            }
            if pad.pressed(*control) {
                outline(frame, cx, cy, CELL_W, CELL_H, PRESSED);
            }
            text(frame, cx + 2, cy + 3, control.name(), if held { INK } else { DIM }, 1);
        }
    }

    /// The plane's eight commands in the top row, as [`PlaneInput::from_pad`] reads them, and under
    /// each the control the shipped table binds it to, as the pad holds it.
    ///
    /// The top row is what the plane gets, so it is not the bottom row again: a stick half over fills
    /// a bank cell by the cube of half (an eighth), the d-pad fills its cell whatever the stick says,
    /// and with both rudder buttons down only the left one fills, because left wins the original's
    /// test. A command cell fills by how much of the command the input step wrote — a throttle's
    /// pressure, a surface's deflection on its side of centre — and never by a number of its own.
    fn plane_controls(&self, frame: &mut Frame, x: i32, y: i32, pad: &Pad) {
        let asked = PlaneInput::from_pad(pad);
        for (i, command) in PlaneCommand::ALL.iter().enumerate() {
            let cx = x + i as i32 * (CELL_W + 2);
            let amount = plane_amount(&asked, *command);
            fill(frame, cx, y, CELL_W, CELL_H, EMPTY);
            if amount > 0.0 {
                let w = ((CELL_W - 2) as f32 * amount.min(1.0)).round().max(1.0) as i32;
                fill(frame, cx + 1, y + 1, w, CELL_H - 2, HELD);
            }
            let ink = if amount > 0.0 { INK } else { DIM };
            text(frame, cx + 2, y + 3, plane_label(*command), ink, 1);

            let control = command.control();
            let cy = y + CELL_H + 2;
            let held = pad.held(control);
            fill(frame, cx, cy, CELL_W, CELL_H, EMPTY);
            if held {
                let strength = pad.strength(control).0.clamp(0.0, 1.0);
                let w = ((CELL_W - 2) as f32 * strength).round().max(1.0) as i32;
                fill(frame, cx + 1, cy + 1, w, CELL_H - 2, HELD);
            }
            if pad.pressed(control) {
                outline(frame, cx, cy, CELL_W, CELL_H, PRESSED);
            }
            text(frame, cx + 2, cy + 3, control.name(), if held { INK } else { DIM }, 1);
        }
    }

    /// The plane's five numbers as its input step wrote them this frame. The rate limits and the
    /// throttle's integration are the joystick step's, which the shell does not run yet, so there is
    /// no "holding" bar beside them as there is for the bike.
    fn plane_commands(&self, frame: &mut Frame, x: i32, y: i32, pad: &Pad) {
        let asked = PlaneInput::from_pad(pad);
        let rows = [
            ("thr+", asked.throttle_up.0),
            ("thr-", asked.throttle_down.0),
            ("elev", asked.elevator.0),
            ("ail", asked.aileron.0),
            ("rud", asked.rudder.0),
        ];
        for (i, (name, v)) in rows.iter().enumerate() {
            let ry = y + i as i32 * 8;
            text(frame, x, ry + 1, name, DIM, 1);
            text(frame, x + 26, ry + 1, &format!("{v:>6.3}"), INK, 1);
        }
    }

    /// The two sticks, each twice over: the blue dot is the report byte and the white cross is what
    /// the player's stick consumers hand the ride after the dead zone and the two segments —
    /// [`AxisRange::PLAYER`], on the bike and in the plane alike (I6, I7).
    ///
    /// The grey box is the dead zone, in the raw dot's own space. Inside it the cross sits dead
    /// centre however far the dot has moved, which is the first 30% of every stick's travel
    /// doing nothing at all — the thing about this game's input that surprises a rider most, and the
    /// reason both dots are here instead of one.
    fn sticks(&self, frame: &mut Frame, x: i32, y: i32, pad: &Pad) {
        for (i, (name, ax, ay)) in [
            ("l", Axis::LeftX, Axis::LeftY),
            ("r", Axis::RightX, Axis::RightY),
        ]
        .iter()
        .enumerate()
        {
            let bx = x + i as i32 * (STICK + 6);
            fill(frame, bx, y, STICK, STICK, EMPTY);
            let (mid_x, mid_y) = (bx + STICK / 2, y + STICK / 2);
            // The centre cross and the dead zone, both in the raw dot's space: the player's range
            // throws away 0.3 of the half travel either side of centre.
            fill(frame, bx + 1, mid_y, STICK - 2, 1, GRID);
            fill(frame, mid_x, y + 1, 1, STICK - 2, GRID);
            let dead = (STICK / 2) * 3 / 10;
            outline(frame, mid_x - dead, mid_y - dead, 2 * dead + 1, 2 * dead + 1, GRID);

            // The report byte, on −1..1, with the one axis the poll inverts put back the way the
            // stick is actually pointing.
            let raw = |axis: Axis| {
                let b = pad.raw_axis(axis).0;
                let b = if axis.is_inverted() { AXIS_FULL.0 - b } else { b };
                b / AXIS_FULL.0 * 2.0 - 1.0
            };
            let at = |fx: f32, fy: f32| {
                (
                    mid_x + (fx.clamp(-1.0, 1.0) * (STICK / 2 - 2) as f32).round() as i32,
                    mid_y + (fy.clamp(-1.0, 1.0) * (STICK / 2 - 2) as f32).round() as i32,
                )
            };
            // The cross is what the game reads and the dot is where the stick is, the dot on top:
            // the two sit on each other while the stick is inside the dead zone, and the dot is the
            // half of that pair a rider needs to be able to see.
            let (vx, vy) = (pad.axis_through(*ax, &AxisRange::PLAYER).0, pad.axis_through(*ay, &AxisRange::PLAYER).0);
            let (px, py) = at(vx, vy);
            fill(frame, px - 3, py, 7, 1, INK);
            fill(frame, px, py - 3, 1, 7, INK);
            let (rx, ry) = at(raw(*ax), raw(*ay));
            fill(frame, rx - 1, ry - 1, 3, 3, RAW);

            text(frame, bx + 2, y + 2, name, DIM, 1);
            text(frame, bx, y + STICK + 2, &format!("x{vx:>6.2}"), INK, 1);
            text(frame, bx, y + STICK + 8, &format!("y{vy:>6.2}"), INK, 1);
        }
    }

    /// The three commands that ramp: what the pad asked for this frame, as a marker, against what the
    /// bike is holding, as the bar.
    ///
    /// Both sides come from the port — [`Controls::through`] the seat's table is the shell's own binding and
    /// [`game_sim::ride::Held`] the `.phy`'s own rate limit — so a bar behind its marker is the
    /// rate limit doing its job and not a dropped input.
    fn commands(&self, frame: &mut Frame, x: i32, y: i32, pad: &Pad, rider: &Rider) {
        let want = Controls::through(pad, &self.bindings);
        let held = &rider.ride.held;
        let rows = [
            ("gas", want.gas.0, held.gas.0),
            ("brake", want.brake.max(want.front_brake).0, held.front_brake.0),
            ("clutch", want.clutch.0, held.clutch.0),
        ];
        for (i, (name, asked, now)) in rows.iter().enumerate() {
            let ry = y + i as i32 * 8;
            text(frame, x, ry + 1, name, DIM, 1);
            let bx = x + 26;
            fill(frame, bx, ry, BAR, 6, EMPTY);
            let w = ((BAR - 2) as f32 * now.clamp(0.0, 1.0)).round() as i32;
            if w > 0 {
                fill(frame, bx + 1, ry + 1, w, 4, HELD);
            }
            let marker = bx + 1 + ((BAR - 2) as f32 * asked.clamp(0.0, 1.0)).round() as i32;
            fill(frame, marker.min(bx + BAR - 1), ry, 1, 6, PRESSED);
            text(frame, bx + BAR + 3, ry + 1, &format!("{now:>5.2}"), INK, 1);
        }
    }

    /// The numbers that say whether the ride is doing what you asked, each one read off the port's own
    /// field and converted no further than a unit a rider can read.
    ///
    /// Speed is the chassis' own `speed` — the velocity's length, in feet a second, as the ride keeps
    /// it — shown in miles an hour beside it the way the loop's own line shows it. Roll and pitch are
    /// the chassis' euler angles, which the simulation keeps in radians and nothing in the step reads,
    /// in degrees. The state is the ride state machine's [`Phase`], with the shell's own freeze
    /// beside it and the reset clock while one is counting.
    fn numbers(&self, frame: &mut Frame, x: i32, y: i32, rider: &Rider, riding: bool) {
        let ride = &rider.ride;
        let speed = ride.chassis.speed.0;
        let gear = ride.engine.gear;
        let euler = ride.chassis.euler;
        let state = match rider.phase() {
            Phase::Riding if !riding => "frozen".to_string(),
            Phase::Riding => "riding".to_string(),
            Phase::Down => "down".to_string(),
            Phase::Resetting => format!("reset {:.1}", rider.state.elapsed.0),
        };
        let trick = match rider.tricks.in_progress {
            // A trick that is running says how far through it is, because the landing test is a
            // question about exactly that: `safe` is the half of it a rider can see.
            true => format!(
                "{} {:>4.2}{}",
                self.trick,
                rider.tricks.progress.0,
                if rider.tricks.safe { " safe" } else { "" }
            ),
            false => self.trick.clone(),
        };
        let (first, second) = match self.flight.filter(|_| self.plane) {
            Some((throttle, airspeed)) => (
                format!("air  {airspeed:>7.1} f/s{:>6.1} mph", airspeed * FEET_TO_MPH),
                format!("throttle{:>5.0}%", throttle * 100.0),
            ),
            None => (
                format!("speed{speed:>7.1} f/s{:>6.1} mph", speed * FEET_TO_MPH),
                format!("rpm{:>9.0}   gear{gear:>7}", ride.engine.rpm.0),
            ),
        };
        let columns = [
            [
                first,
                second,
                format!(
                    "clutch{:>6.2}   {}",
                    ride.held.clutch.0,
                    if ride.engine.clutch_locked { "locked" } else { "slipping" }
                ),
                format!(
                    "ground {:<5}{}",
                    if ride.air.on_ground { "yes" } else { "no" },
                    if ride.touching { "touching" } else { "" }
                ),
                format!("vehicle {}", self.vehicle),
            ],
            [
                format!("roll{:>8.1}   pitch{:>6.1}", deg(euler.z()), deg(euler.x())),
                format!("state  {state}"),
                format!("trick  {trick}"),
                format!("wreck  {}", self.wreck),
                // Clipped to the column: a name the walk can reach is short, but `--camera` can
                // start on any of them.
                format!("camera {}", self.camera.chars().take(19).collect::<String>()),
            ],
        ];
        for (c, lines) in columns.iter().enumerate() {
            for (i, line) in lines.iter().enumerate() {
                text(frame, x + c as i32 * NUMBERS_COL, y + i as i32 * NUMBERS_LINE, line, INK, 2);
            }
        }
    }
}

/// Where the panel goes in a view of `width` by `height` pixels: its top-left pixel. Pinned to the
/// view's bottom-left corner (V24), out of the way of the bike, whatever the view's shape — so on a
/// wide window it sits at the window's corner and not a 4:3 box's, and in a split it sits in its own
/// player's view. It is drawn at its own size, never stretched with the view; a view too short to
/// hold it keeps its top, the numbers, and clips the bottom.
pub fn panel_at(width: usize, height: usize) -> (i32, i32) {
    crate::aspect::corner((width, height), (PANEL_W, PANEL_H), crate::aspect::Corner::BottomLeft, PAD)
}

/// How much of one plane command the input step wrote, 0..1: a throttle command's own value, a
/// surface's deflection on that command's side of centre (dive and bank left are the positive sides),
/// and the rudder's sign.
fn plane_amount(asked: &PlaneInput, command: PlaneCommand) -> f32 {
    match command {
        PlaneCommand::ThrottleUp => asked.throttle_up.0,
        PlaneCommand::ThrottleDown => asked.throttle_down.0,
        PlaneCommand::Dive => asked.elevator.0.max(0.0),
        PlaneCommand::Climb => (-asked.elevator.0).max(0.0),
        PlaneCommand::BankLeft => asked.aileron.0.max(0.0),
        PlaneCommand::BankRight => (-asked.aileron.0).max(0.0),
        PlaneCommand::RudderLeft => (-asked.rudder.0).max(0.0),
        PlaneCommand::RudderRight => asked.rudder.0.max(0.0),
    }
}

/// A plane command's name, short enough for a cell: [`PlaneCommand::name`]'s words, abbreviated.
fn plane_label(command: PlaneCommand) -> &'static str {
    match command {
        PlaneCommand::ThrottleUp => "thr up",
        PlaneCommand::ThrottleDown => "thr dn",
        PlaneCommand::Climb => "climb",
        PlaneCommand::Dive => "dive",
        PlaneCommand::BankLeft => "bank l",
        PlaneCommand::BankRight => "bank r",
        PlaneCommand::RudderLeft => "rud l",
        PlaneCommand::RudderRight => "rud r",
    }
}

/// Feet a second to miles an hour, the same constant the shell's own once-a-second line uses.
const FEET_TO_MPH: f32 = 0.681_818;

/// Radians, which is what the chassis keeps its euler angles in, to degrees.
fn deg(r: f32) -> f32 {
    r.to_degrees()
}

/// The five wreck reasons the port has names for: two the ride state machine decides for itself and
/// three the airborne test decides. A code with no name here is shown as the bare number, which is
/// what the state machine actually holds.
fn wreck_name(why: Wreck) -> Option<&'static str> {
    Some(match why {
        Wreck::CANNONBALL => "cannonball",
        Wreck::NO_CONTROL => "no control",
        Wreck::THROWN => "thrown",
        Wreck::ATTITUDE => "attitude",
        Wreck::TRICK => "trick",
        _ => return None,
    })
}

// --- the layout -------------------------------------------------------------------------------------

/// The border around the panel and inside it.
const PAD: i32 = 4;
/// One control's cell: wide enough for the longest of the sixteen names at one pixel a font pixel.
const CELL_W: i32 = 40;
const CELL_H: i32 = 11;
/// A stick box, square.
const STICK: i32 = 46;
/// A command bar.
const BAR: i32 = 56;
/// The numbers, at two pixels a font pixel: how far apart two lines are, and how far apart the two
/// columns of five are — 27 characters of an 8-pixel advance, which is wider than any line in the
/// left column.
const NUMBERS_LINE: i32 = 12;
const NUMBERS_COL: i32 = 28 * 8;
/// Where each band starts, measured from inside the panel's border.
const CONTROLS_Y: i32 = 5 * NUMBERS_LINE + 6;
const STICKS_Y: i32 = CONTROLS_Y + 2 * (CELL_H + 2) + 6;
/// The whole panel: as wide as the two columns of numbers and as tall as the three bands, with the
/// stick boxes' own two lines of text under them.
const PANEL_W: i32 = NUMBERS_COL + 26 * 8 + PAD;
const PANEL_H: i32 = STICKS_Y + STICK + 14 + 2 * PAD;

// --- the ink ----------------------------------------------------------------------------------------

/// Text, and a stick's own dot.
const INK: [u8; 3] = [255, 255, 255];
/// Text for something that is not held, and the quiet lines.
const DIM: [u8; 3] = [140, 140, 140];
/// A cell or a bar with nothing in it.
const EMPTY: [u8; 3] = [40, 40, 44];
/// The centre cross and the dead zone.
const GRID: [u8; 3] = [90, 90, 96];
/// A stick's raw report byte, behind the dot the game actually reads.
const RAW: [u8; 3] = [90, 150, 250];
/// Held, and the part of a command the bike is holding.
const HELD: [u8; 3] = [80, 210, 110];
/// Pressed this frame, and what the pad is asking for.
const PRESSED: [u8; 3] = [250, 210, 70];
/// The panel itself: the scene behind it, darkened.
const PANEL_DARK: u8 = 58;

// --- the blit ---------------------------------------------------------------------------------------

/// A filled rectangle, clipped to the frame. Everything else here is made of these.
fn fill(frame: &mut Frame, x: i32, y: i32, w: i32, h: i32, color: [u8; 3]) {
    let (fw, fh) = (frame.width as i32, frame.height as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(fw), (y + h).min(fh));
    for py in y0..y1 {
        let row = py as usize * frame.width;
        for px in x0..x1 {
            let at = (row + px as usize) * 3;
            frame.color[at..at + 3].copy_from_slice(&color);
        }
    }
}

/// A one-pixel border.
fn outline(frame: &mut Frame, x: i32, y: i32, w: i32, h: i32, color: [u8; 3]) {
    fill(frame, x, y, w, 1, color);
    fill(frame, x, y + h - 1, w, 1, color);
    fill(frame, x, y, 1, h, color);
    fill(frame, x + w - 1, y, 1, h, color);
}

/// **The markers' slot display** (P88): the five slots and the other players, shown for a while
/// whenever the selection or a slot changes, and for as long as a d-pad direction is held for the
/// markers. Drawn whether or not the overlay above is on: it is the markers' only feedback. The
/// port's own, like the markers themselves.
#[derive(Clone, Debug, Default)]
pub struct MarkerBanner {
    /// What was shown last: the selection and which slots hold a marker.
    seen: Option<(Pick, [bool; SLOTS])>,
    /// Until when it stays up.
    until: Option<Instant>,
}

impl MarkerBanner {
    /// How long it stays up after a change or a hold.
    pub const SHOWN: Duration = Duration::from_millis(2000);

    /// This frame's markers, and whether to draw them: up for [`MarkerBanner::SHOWN`] after the
    /// selection or a slot changes (not on the first frame, which has nothing to change from) and
    /// while `held`, a d-pad direction down for the markers ([`game_sim::markers::held`]) — so a
    /// press that changes nothing (Up on an empty slot) still shows what it was on.
    pub fn update(&mut self, now: Instant, pick: Pick, filled: [bool; SLOTS], held: bool) -> bool {
        let seen = Some((pick, filled));
        if (self.seen.is_some() && self.seen != seen) || held {
            self.until = Some(now + MarkerBanner::SHOWN);
        }
        self.seen = seen;
        self.until.is_some_and(|until| now < until)
    }
}

/// What the slot display says about the selection: the slot and whether it holds a marker, or the
/// player by their name — `P1`, `P2`… for a seat with none.
pub fn marker_label(pick: Pick, filled: [bool; SLOTS], names: &[Option<String>]) -> String {
    match pick {
        Pick::Slot(s) => format!("marker {} {}", s + 1, if filled[s] { "set" } else { "empty" }),
        Pick::Player(p) => match names.get(p).and_then(|n| n.as_deref()).filter(|n| !n.trim().is_empty()) {
            Some(name) => format!("to {name}"),
            None => format!("to P{}", p + 1),
        },
    }
}

/// The slot display, top centre of `frame`: five boxes — solid in the slot's colour where a marker
/// is (P94: the colour of its post and its radar mark), the selected one ringed — then a box for each
/// other player in the ride, `P2`… by seat (`others`, P94: a seat nobody rides is not listed), and
/// the label under them, and under that how far a Down or Up hold is towards firing (`holding`,
/// [`game_sim::markers::holding`]: P88's bar, back with the hold in P106; Left and Right fire on the
/// press and leave it empty).
#[allow(dead_code)] // the tests', the shell draws at the settings' scale
pub fn draw_markers(frame: &mut Frame, pick: Pick, filled: [bool; SLOTS], label: &str, others: &[usize], holding: u32) {
    draw_markers_scaled(frame, pick, filled, label, others, holding, 1.0);
}

/// [`draw_markers`] at the settings' HUD scale (V52), `hud_scale` times its own size, still top centre:
/// its whole pixels a font pixel are the view's 224-line steps times the scale, rounded down, and
/// never fewer than one.
pub fn draw_markers_scaled(frame: &mut Frame, pick: Pick, filled: [bool; SLOTS], label: &str, others: &[usize], holding: u32, hud_scale: f32) {
    let scale = ((frame.height as i32 / 224) as f32 * hud_scale).floor().max(1.0) as i32;
    let (boxw, gap) = (7 * scale, 3 * scale);
    let tags: Vec<String> = others.iter().map(|p| format!("P{}", p + 1)).collect();
    let tag_w = |t: &str| t.chars().count() as i32 * 4 * scale + 2 * scale;
    let players: i32 = tags.iter().map(|t| gap + tag_w(t)).sum();
    let row = SLOTS as i32 * boxw + (SLOTS as i32 - 1) * gap + players;
    let label_w = label.chars().count() as i32 * 4 * scale;
    let w = row.max(label_w) + 2 * PAD * scale;
    let h = boxw + 5 * scale + 2 * scale + 6 * scale + 2 * PAD * scale;
    let (x, y) = ((frame.width as i32 - w) / 2, 8 * scale);
    panel(frame, x, y, w, h);
    let (bx, by) = (x + (w - row) / 2, y + PAD * scale);
    for (i, &set) in filled.iter().enumerate() {
        let at = bx + i as i32 * (boxw + gap);
        let ink = if pick == Pick::Slot(i) { PRESSED } else { DIM };
        if set {
            fill(frame, at + scale, by + scale, boxw - 2 * scale, boxw - 2 * scale, slot_colour(i));
        }
        outline(frame, at, by, boxw, boxw, ink);
        if pick == Pick::Slot(i) {
            outline(frame, at - scale, by - scale, boxw + 2 * scale, boxw + 2 * scale, ink);
        }
    }
    let mut at = bx + SLOTS as i32 * (boxw + gap) - gap;
    for (&p, tag) in others.iter().zip(&tags) {
        at += gap;
        let (tw, on) = (tag_w(tag), pick == Pick::Player(p));
        let ink = if on { PRESSED } else { DIM };
        outline(frame, at, by, tw, boxw, ink);
        if on {
            outline(frame, at - scale, by - scale, tw + 2 * scale, boxw + 2 * scale, ink);
        }
        text(frame, at + scale + scale / 2, by + scale, tag, ink, scale);
        at += tw;
    }
    let ty = by + boxw + 2 * scale;
    let ink = if matches!(pick, Pick::Player(_)) { PRESSED } else { INK };
    text(frame, x + (w - label_w) / 2, ty, label, ink, scale);
    let bar_y = ty + 7 * scale;
    let hold = game_sim::markers::HOLD_MS;
    fill(frame, bx, bar_y, row, scale, EMPTY);
    let done = row * holding.min(hold) as i32 / hold as i32;
    fill(frame, bx, bar_y, done, scale, if holding >= hold { PRESSED } else { HELD });
}

/// A marker slot's colour (P94), [`crate::overlay::MARKER_COLOURS`], in this overlay's bytes.
fn slot_colour(slot: usize) -> [u8; 3] {
    let [r, g, b, _] = crate::overlay::marker_colour(slot);
    [r, g, b]
}

/// The panel: the scene behind it taken down towards black, so the dirt still shows through and the
/// text on top of it still reads. One multiply-add a pixel, in bytes.
fn panel(frame: &mut Frame, x: i32, y: i32, w: i32, h: i32) {
    let (fw, fh) = (frame.width as i32, frame.height as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(fw), (y + h).min(fh));
    for py in y0..y1 {
        let row = py as usize * frame.width;
        for px in x0..x1 {
            let at = (row + px as usize) * 3;
            for byte in &mut frame.color[at..at + 3] {
                *byte = ((*byte as u32 * PANEL_DARK as u32) >> 8) as u8;
            }
        }
    }
}

/// A string, left to right from its top-left corner, at `scale` pixels a font pixel. Lower case is
/// drawn from the upper-case glyph: the font has one case, which is what keeps it five bytes a glyph.
fn text(frame: &mut Frame, x: i32, y: i32, s: &str, color: [u8; 3], scale: i32) {
    let mut at = x;
    for c in s.chars() {
        if let Some(rows) = glyph(c) {
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..3 {
                    if bits & (0b100 >> col) != 0 {
                        fill(
                            frame,
                            at + col * scale,
                            y + row as i32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
        at += 4 * scale;
    }
}

/// How wide [`text`] draws a string.
#[cfg(test)]
fn text_width(s: &str, scale: i32) -> i32 {
    s.chars().count() as i32 * 4 * scale
}

/// One glyph: five rows of three bits, top row first, most significant bit on the left. `None` for a
/// space and for anything the font has no shape for, which draws nothing and still advances.
fn glyph(c: char) -> Option<[u8; 5]> {
    let i = |c: char, from: char| (c as u8 - from as u8) as usize;
    match c {
        '0'..='9' => Some(DIGITS[i(c, '0')]),
        'A'..='Z' => Some(LETTERS[i(c, 'A')]),
        'a'..='z' => Some(LETTERS[i(c, 'a')]),
        '.' => Some([0, 0, 0, 0, 0b010]),
        ',' => Some([0, 0, 0, 0b010, 0b010]),
        '-' => Some([0, 0, 0b111, 0, 0]),
        '+' => Some([0, 0b010, 0b111, 0b010, 0]),
        ':' => Some([0, 0b010, 0, 0b010, 0]),
        '/' => Some([0b001, 0b001, 0b010, 0b100, 0b100]),
        '%' => Some([0b101, 0b001, 0b010, 0b100, 0b101]),
        '(' => Some([0b010, 0b100, 0b100, 0b100, 0b010]),
        ')' => Some([0b010, 0b001, 0b001, 0b001, 0b010]),
        '?' => Some([0b111, 0b001, 0b011, 0, 0b010]),
        '!' => Some([0b010, 0b010, 0b010, 0, 0b010]),
        '=' => Some([0, 0b111, 0, 0b111, 0]),
        '_' => Some([0, 0, 0, 0, 0b111]),
        '<' => Some([0b001, 0b010, 0b100, 0b010, 0b001]),
        '>' => Some([0b100, 0b010, 0b001, 0b010, 0b100]),
        _ => None,
    }
}

/// The ten digits, 3×5.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

/// The twenty-six letters, 3×5 and one case.
const LETTERS: [[u8; 5]; 26] = [
    [0b111, 0b101, 0b111, 0b101, 0b101], // A
    [0b110, 0b101, 0b110, 0b101, 0b110], // B
    [0b111, 0b100, 0b100, 0b100, 0b111], // C
    [0b110, 0b101, 0b101, 0b101, 0b110], // D
    [0b111, 0b100, 0b111, 0b100, 0b111], // E
    [0b111, 0b100, 0b111, 0b100, 0b100], // F
    [0b111, 0b100, 0b101, 0b101, 0b111], // G
    [0b101, 0b101, 0b111, 0b101, 0b101], // H
    [0b111, 0b010, 0b010, 0b010, 0b111], // I
    [0b001, 0b001, 0b001, 0b101, 0b111], // J
    [0b101, 0b101, 0b110, 0b101, 0b101], // K
    [0b100, 0b100, 0b100, 0b100, 0b111], // L
    [0b101, 0b111, 0b111, 0b101, 0b101], // M
    [0b101, 0b111, 0b111, 0b111, 0b101], // N
    [0b111, 0b101, 0b101, 0b101, 0b111], // O
    [0b111, 0b101, 0b111, 0b100, 0b100], // P
    [0b111, 0b101, 0b101, 0b111, 0b001], // Q
    [0b111, 0b101, 0b111, 0b110, 0b101], // R
    [0b111, 0b100, 0b111, 0b001, 0b111], // S
    [0b111, 0b010, 0b010, 0b010, 0b010], // T
    [0b101, 0b101, 0b101, 0b101, 0b111], // U
    [0b101, 0b101, 0b101, 0b101, 0b010], // V
    [0b101, 0b101, 0b111, 0b111, 0b101], // W
    [0b101, 0b101, 0b010, 0b101, 0b101], // X
    [0b101, 0b101, 0b010, 0b010, 0b010], // Y
    [0b111, 0b001, 0b010, 0b100, 0b111], // Z
];

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::input::{AXIS_CENTRE, Manager, Report};
    use ps2_float::F;

    /// A frame of flat colour, so anything the overlay writes shows as a difference.
    fn blank() -> Frame {
        Frame::new(640, 448, [20, 60, 120])
    }

    fn untouched(frame: &Frame) -> bool {
        frame.color.chunks_exact(3).all(|p| p == [20, 60, 120])
    }

    /// One frame of the input layer, so the pad under test is one the game itself polled rather than
    /// a hand-filled struct.
    fn polled(report: Report) -> Manager {
        let mut m = Manager::connected();
        m.tick(F(1.0 / 60.0), &[report, Report::missing()]);
        m
    }

    fn rider() -> Rider {
        use game_sim::phy::Phy;
        use game_sim::ride::Tables;
        use game_sim::trick::Set;
        use ps2_float::Vf;
        let tables = Tables::build();
        let phy = Phy::read_bytes(b"", &tables.curve);
        let mounts = (Vf::new(1.0, 0.0, 0.0, 1.0), Vf::new(-1.0, 0.0, 0.0, 1.0));
        Rider::new(
            &phy,
            mounts,
            Vf::new(0.0, 0.0, 0.0, 1.0),
            Vf::new(0.0, 0.0, 1.0, 0.0),
            Set::default(),
        )
    }

    #[test]
    fn the_flag_off_draws_nothing_at_all() {
        let mut frame = blank();
        let hud = Hud::new(false);
        hud.draw(&mut frame, polled(Report::idle()).pad(0), &rider(), true);
        assert!(untouched(&frame), "the overlay drew with its flag off");
    }

    #[test]
    fn the_flag_on_draws_in_the_bottom_left_and_nowhere_else() {
        let mut frame = blank();
        let hud = Hud::new(true);
        hud.draw(&mut frame, polled(Report::idle()).pad(0), &rider(), true);
        assert!(!untouched(&frame), "the overlay drew nothing with its flag on");
        // Above the panel and to the right of it the picture is the shell's, not the overlay's.
        let at = |x: usize, y: usize| {
            let i = (y * frame.width + x) * 3;
            [frame.color[i], frame.color[i + 1], frame.color[i + 2]]
        };
        assert_eq!(at(320, 100), [20, 60, 120], "the middle of the picture");
        assert_eq!(at(639, 447), [20, 60, 120], "the bottom right corner");
        assert_ne!(at(8, 448 - PANEL_H as usize / 2), [20, 60, 120], "inside the panel");
    }

    /// The changed pixels' bounds (left, top, right, bottom) of a frame drawn over [`blank`]'s colour.
    fn changed(frame: &Frame) -> (usize, usize, usize, usize) {
        let mut b = (usize::MAX, usize::MAX, 0, 0);
        for (i, p) in frame.color.chunks_exact(3).enumerate() {
            if p != [20, 60, 120] {
                let (x, y) = (i % frame.width, i / frame.width);
                b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
            }
        }
        b
    }

    /// V52: the settings' HUD scale sizes the panel about the view's bottom-left corner — 150 % half
    /// again as big, 50 % half the size, the same margin to the corner, the ink still there — and the
    /// markers' slot display about the top's middle. A view too small for the scale gets the most
    /// it holds, never past its edges.
    #[test]
    fn the_hud_scale_sizes_the_panel_and_the_markers_display() {
        let pad = polled(Report::idle());
        let rider = rider();
        let mut hud = Hud::new(true);
        let (w, h) = (1920, 1080);
        let mut at = |scale: f32| {
            hud.scale = scale;
            let mut frame = Frame::new(w, h, [20, 60, 120]);
            hud.draw(&mut frame, pad.pad(0), &rider, true);
            (changed(&frame), frame)
        };
        let ((x0, _, x1, y1), _) = at(1.0);
        assert_eq!((x0, x1 - x0, y1), (PAD as usize, PANEL_W as usize, h - PAD as usize));
        for scale in [1.5, 0.5] {
            let ((x0, y0, x1, y1), frame) = at(scale);
            let size = ((PANEL_W as f32 * scale).round() as usize, (PANEL_H as f32 * scale).round() as usize);
            assert_eq!((x0, y1), (PAD as usize, h - PAD as usize), "{scale}: pinned to the corner");
            assert_eq!((x1 - x0, y1 - y0), size, "{scale}");
            // The white ink is there, not only the darkened panel.
            let inked = frame.color.chunks_exact(3).filter(|p| *p == INK).count();
            assert!(inked > 100, "{scale}: {inked} pixels of ink");
        }
        // A 640x360 view holds the panel at no more than it fits.
        hud.scale = 1.5;
        let mut small = Frame::new(640, 360, [20, 60, 120]);
        hud.draw(&mut small, pad.pad(0), &rider, true);
        let (x0, y0, x1, y1) = changed(&small);
        assert!(x0 >= PAD as usize && y0 >= PAD as usize && x1 <= 640 - PAD as usize && y1 <= 360 - PAD as usize, "{:?}", (x0, y0, x1, y1));
        assert!(x1 - x0 > PANEL_W as usize, "grown as far as it fits");

        // The markers' display: whole pixels a font pixel, 3 for 2 at 150 % of a 448-line view.
        let banner = |scale: f32| {
            let mut frame = Frame::new(1280, 448, [20, 60, 120]);
            draw_markers_scaled(&mut frame, Pick::Slot(1), [true, false, true, false, false], "marker 2 empty", &[1], 0, scale);
            changed(&frame)
        };
        let (a, b) = (banner(1.0), banner(1.5));
        assert_eq!((b.2 - b.0) * 2, (a.2 - a.0) * 3);
        assert_eq!((b.3 - b.1) * 2, (a.3 - a.1) * 3);
        assert_eq!(a.0 + a.2, b.0 + b.2, "both centred");
        let mut plain = Frame::new(1280, 448, [20, 60, 120]);
        draw_markers(&mut plain, Pick::Slot(1), [true, false, true, false, false], "marker 2 empty", &[1], 0);
        assert_eq!(changed(&plain), a, "100 % is the display as it was");
    }

    /// V24: in every view of a 4:3, 16:9, 21:9 and portrait window, alone and split two and four
    /// ways, the panel is drawn at its own size pinned to that view's bottom-left corner — the
    /// changed pixels are exactly the panel's rectangle there, wherever the view's corner is.
    #[test]
    fn the_panel_is_pinned_to_each_views_own_corner() {
        use crate::aspect::Aspect;
        use crate::split::{Layout, Split};
        let pad = polled(Report::idle());
        let hud = Hud::new(true);
        let rider = rider();
        for window in [(1440, 1080), (1920, 1080), (2560, 1080), (1080, 1920)] {
            let (w, h) = Aspect::Fill.frame(window, 1);
            for (players, split) in [(1, Split::Stacked), (2, Split::Stacked), (2, Split::SideBySide), (4, Split::Stacked)] {
                let layout = Layout::for_players(players, split).unwrap();
                let view = layout.rects(w, h)[players - 1];
                let mut frame = Frame::new(view.w, view.h, [20, 60, 120]);
                hud.draw(&mut frame, pad.pad(0), &rider, true);
                let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
                for (i, p) in frame.color.chunks_exact(3).enumerate() {
                    if p != [20, 60, 120] {
                        let (x, y) = (i % view.w, i / view.w);
                        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                    }
                }
                let what = format!("{window:?} {layout:?} view {}x{}", view.w, view.h);
                assert_eq!((x0, y0), (PAD as usize, view.h - PANEL_H as usize - PAD as usize), "{what}");
                assert_eq!((x1 + 1 - x0, y1 + 1 - y0), (PANEL_W as usize, PANEL_H as usize), "{what}");
            }
        }
    }

    /// The point of the thing: a control the pad reports is a control the overlay fills, and one it
    /// does not report is one it leaves empty.
    #[test]
    fn a_held_control_is_filled_and_an_untouched_one_is_not() {
        // The cell of a control, as `controls` lays them out, inside the panel at its own corner.
        let cell = |control: Control| {
            let i = control.index() as i32;
            let x = PAD + PAD + (i % 8) * (CELL_W + 2);
            let y = 448 - PANEL_H - PAD + PAD + CONTROLS_Y + (i / 8) * (CELL_H + 2);
            // A pixel well inside the fill, past the border and clear of the label's first column.
            ((x + 2) as usize, (y + 1) as usize)
        };
        let at = |frame: &Frame, (x, y): (usize, usize)| {
            let i = (y * frame.width + x) * 3;
            [frame.color[i], frame.color[i + 1], frame.color[i + 2]]
        };

        let mut frame = blank();
        let hud = Hud::new(true);
        let manager = polled(Report::from_buttons(Control::Cross.mask()));
        hud.draw(&mut frame, manager.pad(0), &rider(), true);
        assert!(manager.pad(0).held(Control::Cross), "the pad read the press");
        assert_eq!(at(&frame, cell(Control::Cross)), HELD, "the held control");
        assert_eq!(at(&frame, cell(Control::Circle)), EMPTY, "one nobody touched");
    }

    /// Pressed-this-frame is a border, and it is gone the frame after: the overlay shows the input
    /// layer's own edge, not a latch of its own.
    #[test]
    fn pressed_this_frame_is_marked_and_held_the_next_frame_is_not() {
        let corner = {
            let i = Control::Cross.index() as i32;
            let x = PAD + PAD + (i % 8) * (CELL_W + 2);
            let y = 448 - PANEL_H - PAD + PAD + CONTROLS_Y + (i / 8) * (CELL_H + 2);
            ((x) as usize, (y) as usize)
        };
        let at = |frame: &Frame, (x, y): (usize, usize)| {
            let i = (y * frame.width + x) * 3;
            [frame.color[i], frame.color[i + 1], frame.color[i + 2]]
        };
        let hud = Hud::new(true);
        let down = Report::from_buttons(Control::Cross.mask());
        let mut manager = Manager::connected();
        let ride = rider();

        manager.tick(F(1.0 / 60.0), &[down, Report::missing()]);
        let mut first = blank();
        hud.draw(&mut first, manager.pad(0), &ride, true);
        assert_eq!(at(&first, corner), PRESSED, "the frame the button went down");

        manager.tick(F(1.0 / 60.0), &[down, Report::missing()]);
        let mut second = blank();
        hud.draw(&mut second, manager.pad(0), &ride, true);
        assert_ne!(at(&second, corner), PRESSED, "still held is not pressed again");
    }

    /// The stick is drawn where the input layer put it, not where the byte is: inside the dead zone
    /// the bright dot is at the centre and the faint one is not.
    #[test]
    fn inside_the_dead_zone_the_two_dots_disagree() {
        let mut report = Report::idle();
        // A tenth of the travel: inside the 30% that the player's range throws away. The index
        // is the report's own order (right X, right Y, left X, left Y) and not the pad's.
        const REPORT_LEFT_X: usize = 2;
        report.set_axis(REPORT_LEFT_X, AXIS_CENTRE + 12);
        let manager = polled(report);
        let pad = manager.pad(0);
        assert_eq!(pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), F::ZERO, "the range swallowed it");
        assert!(pad.raw_axis(Axis::LeftX) > F(AXIS_CENTRE as f32), "the byte moved");

        let mut frame = blank();
        Hud::new(true).draw(&mut frame, pad, &rider(), true);
        // The left stick's centre, as `sticks` lays it out.
        let x = PAD + PAD + STICK / 2;
        let y = 448 - PANEL_H - PAD + PAD + STICKS_Y + STICK / 2;
        let at = |x: i32, y: i32| {
            let i = (y as usize * frame.width + x as usize) * 3;
            [frame.color[i], frame.color[i + 1], frame.color[i + 2]]
        };
        // The cross's own centre, and the dot two pixels off it: a tenth of the travel over the box's
        // half width, which is what the byte asked for and the range threw away.
        assert_eq!(at(x, y), INK, "the curve's cross is dead centre");
        assert_eq!(at(x + 2, y + 1), RAW, "the byte's dot is off to the side");
    }

    /// With the plane up, the top row is what the plane's input step read and the row under it the
    /// controls: both rudder buttons down fill both control cells but only the left rudder, because
    /// left wins the original's test; and Cross fills the throttle.
    #[test]
    fn the_plane_shows_its_eight_over_the_controls_they_are_bound_to() {
        let cell = |i: i32, row: i32| {
            let x = PAD + PAD + i * (CELL_W + 2);
            let y = 448 - PANEL_H - PAD + PAD + CONTROLS_Y + row * (CELL_H + 2);
            ((x + 2) as usize, (y + 1) as usize)
        };
        let at = |frame: &Frame, (x, y): (usize, usize)| {
            let i = (y * frame.width + x) * 3;
            [frame.color[i], frame.color[i + 1], frame.color[i + 2]]
        };
        let slot = |c: PlaneCommand| PlaneCommand::ALL.iter().position(|&p| p == c).unwrap() as i32;
        let mut hud = Hud::new(true);
        hud.plane = true;
        let buttons = Control::L1.mask() | Control::R1.mask() | Control::Cross.mask();
        let manager = polled(Report::from_buttons(buttons));
        let mut frame = blank();
        hud.draw(&mut frame, manager.pad(0), &rider(), true);
        let left = slot(PlaneCommand::RudderLeft);
        let right = slot(PlaneCommand::RudderRight);
        assert_eq!(at(&frame, cell(left, 1)), HELD, "l1 is held");
        assert_eq!(at(&frame, cell(right, 1)), HELD, "r1 is held");
        assert_eq!(at(&frame, cell(left, 0)), HELD, "the plane reads left rudder");
        assert_eq!(at(&frame, cell(right, 0)), EMPTY, "and not right: left wins the tie");
        assert_eq!(at(&frame, cell(slot(PlaneCommand::ThrottleUp), 0)), HELD, "cross is the throttle");
        assert_eq!(at(&frame, cell(slot(PlaneCommand::Climb), 0)), EMPTY, "nobody touched the stick");
    }

    /// The ride's stick goes through the player's ±0.3 range, not the main menu's ±0.2: a quarter of
    /// the travel would move a menu consumer and leaves the cross dead centre, in the plane and on the
    /// bike alike.
    #[test]
    fn the_planes_stick_has_the_players_dead_zone() {
        let mut report = Report::idle();
        const REPORT_LEFT_X: usize = 2;
        report.set_axis(REPORT_LEFT_X, AXIS_CENTRE + 32);
        let manager = polled(report);
        let pad = manager.pad(0);
        assert!(pad.axis(Axis::LeftX) > F::ZERO, "past the main menu's dead zone");
        assert_eq!(pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), F::ZERO, "inside the plane's");

        let mut hud = Hud::new(true);
        hud.plane = true;
        let mut frame = blank();
        hud.draw(&mut frame, pad, &rider(), true);
        let x = PAD + PAD + STICK / 2;
        let y = 448 - PANEL_H - PAD + PAD + STICKS_Y + STICK / 2;
        let i = (y as usize * frame.width + x as usize) * 3;
        assert_eq!([frame.color[i], frame.color[i + 1], frame.color[i + 2]], INK, "the cross is dead centre");
    }

    /// The font is what the numbers are read off, so a glyph missing from it is a number nobody can
    /// read. Everything the overlay formats comes out of this set.
    #[test]
    fn the_font_has_a_shape_for_everything_the_overlay_writes() {
        let used = "abcdefghijklmnopqrstuvwxyz0123456789./-+";
        for c in used.chars() {
            assert!(glyph(c).is_some(), "no glyph for {c:?}");
        }
        assert!(glyph(' ').is_none(), "a space is a space");
        assert_eq!(text_width("abc", 2), 24);
    }

    /// The panel has to fit the window the shell opens smallest in its own help text, and the
    /// numbers column has to fit the widest line the overlay writes.
    #[test]
    fn the_panel_fits_the_smallest_window_the_shell_offers() {
        assert!(PANEL_W + 2 * PAD <= 512, "{PANEL_W} wide");
        assert!(PANEL_H + 2 * PAD <= 384, "{PANEL_H} tall");
        let widest = "speed  123.4 f/s  84.1 mph";
        assert!(
            NUMBERS_COL + text_width(widest, 2) + PAD <= PANEL_W,
            "the numbers run past the panel"
        );
        // And the band below them: eight cells and the gaps between them.
        assert!(8 * (CELL_W + 2) - 2 + 2 * PAD <= PANEL_W, "the controls run past the panel");
    }

    /// A frame of the overlay has to cost a rider nothing: the brief's budget is a millisecond, and
    /// that is the bound here, averaged over enough frames that a scheduler hiccup cannot carry the
    /// whole measurement. The margin is wider than it looks — the tests build without optimisation,
    /// where a frame of the panel measures around 150 µs against about a tenth of that in the
    /// `--release` build the shell is actually ridden in, so this passes in the slow build and the
    /// number a rider pays is the fast one.
    #[test]
    fn a_frame_of_the_overlay_costs_well_under_a_millisecond() {
        let mut frame = blank();
        let hud = Hud::new(true);
        let manager = polled(Report::from_buttons(Control::Cross.mask()));
        let ride = rider();
        // Once first, so the measurement is not paying for the first touch of the buffer.
        hud.draw(&mut frame, manager.pad(0), &ride, true);
        let frames = 200;
        let started = std::time::Instant::now();
        for _ in 0..frames {
            hud.draw(&mut frame, manager.pad(0), &ride, true);
        }
        let each = started.elapsed() / frames;
        println!("a frame of the overlay at 640x448: {each:?}");
        assert!(
            each < std::time::Duration::from_micros(1000),
            "a frame of the overlay cost {each:?}"
        );
        // And with the flag off it is one branch.
        let off = Hud::new(false);
        let started = std::time::Instant::now();
        for _ in 0..frames {
            off.draw(&mut frame, manager.pad(0), &ride, true);
        }
        assert!(started.elapsed() / frames < std::time::Duration::from_micros(2));
    }
}
