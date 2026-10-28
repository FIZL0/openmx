//! **The activities' start zones in the world, and the session markers as small ones** (V51).
//!
//! The original marks where each free-ride activity starts with a portal: a disc on the ground with
//! an arrow on it pointing the way the zone looks (`hitzone_start_circle`, its material swapped for
//! `hitzone_start_activity_circle`), a column of light standing round it whose stripes crawl up and
//! round (`hitzone_landing_cylinder`, added), and over it the activity's word (`startzone_hit`,
//! `_run`, `_race`, `_stunt`) with the zone's number in its progression under it (`startzone_1` ..
//! `_5`, the hit and run progressions only), turned to face the camera and rising as it draws away.
//! Every piece is drawn in the activity's colour, the radar icon's: the disc and the words at 0.6
//! alpha, the column at 0.5 fading to nothing at its top.
//!
//! [`Spot`] is a zone as `hit.dat` places it; [`Portal`] is one built over the ground, a zone's at
//! full size or a session marker's at [`MARKER_FRACTION`] of it, in its slot's colour, facing the
//! way the marker's bike faced. [`Portal::glows`] lays it out for one view as
//! [`game_render::glow::Glow`]s cut from [`Art`]'s pictures.

use crate::overlay::Activity;
use game_render::glow::{Glow, Vertex};
use game_render::pst::Texture;

/// A start zone, as a track's `hit.dat` places it (`[ASZ n]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub activity: Activity,
    /// `Position`'s `x`, `y` and `z`.
    pub at: [f32; 3],
    /// `LookVector`'s `x` and `z`: the way the disc's arrow points.
    pub look: [f32; 2],
    /// `Scale`'s `x` and `z`: the disc's size across, in feet, along its side and along its look.
    pub size: [f32; 2],
    /// Which of its progression's zones it is, from 1: where its `HitSetID` is in `[Progression
    /// HitSets]` (a hit zone) or `[Run HitSets]` (a run zone). `None` for the race and the stunt
    /// zones, and a zone whose set neither list names.
    pub number: Option<usize>,
}

/// The track's freestyle event (P155): its `hit.dat`'s stunt zones, `spots` of [`spots`], as the
/// simulation's trigger boxes, and the file's target.
pub fn stunt_event(spots: &[Spot], hit_dat: &str) -> game_sim::event::Event {
    let zones = spots
        .iter()
        .filter(|s| s.activity == Activity::Stunt)
        .map(|s| game_sim::event::Zone::new(s.at, s.look, s.size))
        .collect();
    game_sim::event::Event::on_track(zones, game_sim::event::target(hit_dat).unwrap_or(0))
}

/// The start zones of a track's `hit.dat`, in the file's order. A section with a type this does not
/// know, or no position, is left out, as [`crate::overlay::zones`] leaves it.
pub fn spots(text: &str) -> Vec<Spot> {
    #[derive(Default)]
    struct Read {
        at: Option<[f32; 3]>,
        look: Option<[f32; 2]>,
        size: Option<[f32; 2]>,
        activity: Option<Activity>,
        set: Option<i64>,
    }
    let floats = |value: &str| -> Vec<f32> { value.split(',').filter_map(|n| n.trim().parse().ok()).collect() };
    let (mut progression, mut run) = (Vec::new(), Vec::new());
    let mut read = Vec::new();
    // Which list or zone the lines are in.
    enum In {
        Nothing,
        Progression,
        Run,
        Zone,
    }
    let mut now = In::Nothing;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            now = match line {
                "[Progression HitSets]" => In::Progression,
                "[Run HitSets]" => In::Run,
                _ if line.starts_with("[ASZ ") => {
                    read.push(Read::default());
                    In::Zone
                }
                _ => In::Nothing,
            };
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim());
        match now {
            In::Progression | In::Run if key == "HitSetID" => {
                if let Ok(id) = value.parse::<i64>() {
                    if matches!(now, In::Progression) { progression.push(id) } else { run.push(id) }
                }
            }
            In::Zone => {
                let r = read.last_mut().expect("a zone's section was opened");
                match key {
                    "Position" => r.at = Some(floats(value)).filter(|v| v.len() >= 3).map(|v| [v[0], v[1], v[2]]),
                    "LookVector" => r.look = Some(floats(value)).filter(|v| v.len() >= 3).map(|v| [v[0], v[2]]),
                    "Scale" => r.size = Some(floats(value)).filter(|v| v.len() >= 3).map(|v| [v[0], v[2]]),
                    "Type" => r.activity = Activity::of(value),
                    "HitSetID" => r.set = value.parse().ok(),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    read.into_iter()
        .filter_map(|r| {
            let (at, activity) = (r.at?, r.activity?);
            let list = match activity {
                Activity::Hit => &progression,
                Activity::Run => &run,
                Activity::Race | Activity::Stunt => &Vec::new(),
            };
            let number = r.set.and_then(|id| list.iter().position(|&s| s == id)).map(|i| i + 1);
            Some(Spot { activity, at, look: r.look.unwrap_or([0.0, 1.0]), size: r.size.unwrap_or([ZONE_SIZE; 2]), number })
        })
        .collect()
}

/// The start zones the original shows a player who has started none, as
/// [`crate::overlay::shown_zones`] picks them for the radar: the first of each activity, and not the
/// machine race's while the player is in a machine.
pub fn shown(spots: &[Spot], in_machine: bool) -> Vec<Spot> {
    Activity::ALL
        .iter()
        .filter(|&&a| !(in_machine && a == Activity::Race))
        .filter_map(|&a| spots.iter().find(|s| s.activity == a).copied())
        .collect()
}

/// Every zone of the shipped tracks is this across, `Scale = 40, 0, 40`: what a marker's portal is a
/// fraction of.
pub const ZONE_SIZE: f32 = 40.0;
/// A session marker's portal against a zone's: a quarter, 10 ft across, so it stands out on the
/// ground without being mistaken for an activity. The port's own choice.
pub const MARKER_FRACTION: f32 = 0.25;
/// How much more opaque the selected marker's portal is than the rest.
pub const SELECTED_GAIN: f32 = 1.6;

/// The disc's and the words' alpha, and the column's at its foot.
const DISC_ALPHA: f32 = 0.6;
const WORD_ALPHA: f32 = 0.6;
const COLUMN_ALPHA: f32 = 0.5;
/// The column, as the zone sizes it: 225 ft tall, its middle 87.5 ft over the zone (so it starts
/// 25 ft under the ground), and a foot narrower than the disc.
const COLUMN_HEIGHT: f32 = 225.0;
const COLUMN_MIDDLE: f32 = 87.5;
const COLUMN_NARROWER: f32 = 1.0;
/// How fast the column's stripes crawl, in texture repeats a second: round, and up.
const COLUMN_CRAWL: [f32; 2] = [0.75, -0.30];
/// How high the zone's number stands over the zone: 15 ft near it, rising to 90 at 500 ft and beyond;
/// the activity's word 15 ft over the number. The number is 20 ft to the model's foot, the word 40.
const WORD_LOW: f32 = 15.0;
const WORD_RISE: f32 = 75.0;
const WORD_RISE_OVER: f32 = 500.0;
const WORD_OVER_NUMBER: f32 = 15.0;
const NUMBER_SCALE: f32 = 20.0;
const WORD_SCALE: f32 = 40.0;
/// How far over the ground the disc is laid, so the terrain does not cut through it.
const DISC_LIFT: f32 = 0.5;
/// The disc's mesh: rings out from the middle, and points round each.
const DISC_RINGS: usize = 6;
const DISC_ROUND: usize = 32;

/// A model's triangles: each corner where it is in the model, its texture coordinates, and its
/// colour (`1.0` the model's full `128`).
type Mesh = Vec<[([f32; 3], [f32; 2], [f32; 4]); 3]>;

/// The pictures and models the portals are cut from, out of the track's second archive.
#[derive(Clone, Debug, Default)]
pub struct Art {
    /// The glow textures: the disc's arrow, the column's stripes, and the words' white, in that
    /// order ([`DISC`], [`COLUMN`], [`WORDS`]).
    pub textures: Vec<Texture>,
    column: Mesh,
    /// `startzone_hit`, `_run`, `_race`, `_stunt` and the numbers, by the model's name after
    /// `startzone_`.
    words: Vec<(String, Mesh)>,
}

/// The archive the art is read from on a track whose own lacks it: a free-ride track's second.
pub const ART_ARCHIVE: &str = "free06_2.res";

/// [`Art::textures`]' entries.
pub const DISC: usize = 0;
pub const COLUMN: usize = 1;
pub const WORDS: usize = 2;

impl Art {
    /// The portals' art, read through `read` (a member's bytes by its name's ending), or `None` when
    /// the disc's texture or the column is not there — a track with no start zones.
    pub fn load(read: impl Fn(&str) -> Option<Vec<u8>>) -> Option<Art> {
        let texture = |name: &str| read(&format!("tex\\{name}.pst")).and_then(|b| game_render::replace::parse(&b));
        let mesh = |name: &str| -> Option<Mesh> {
            let model = game_render::psg::Model::parse(&read(&format!("geom\\{name}.psg"))?).ok()?;
            let colour = |c: [u8; 4]| c.map(|k| k as f32 / 128.0);
            let tris = model.triangles(0);
            Some(tris.iter().map(|t| t.v.map(|v| ([v.pos.x, v.pos.y, v.pos.z], v.uv, colour(v.color)))).collect())
        };
        let white = Texture { width: 1, height: 1, rgba: vec![255; 4] };
        let textures = vec![texture("hitzone_start_activity")?, texture("hitzone_anim01")?, texture("startzone_objects").unwrap_or(white)];
        let column = mesh("hitzone_landing_cylinder")?;
        let names = ["hit", "run", "race", "stunt", "1", "2", "3", "4", "5"];
        let words = names.iter().filter_map(|n| Some((n.to_string(), mesh(&format!("startzone_{n}"))?))).collect();
        Some(Art { textures, column, words })
    }

    /// Whether it has every slot's number, `startzone_1` to `_5`: the free-ride tracks' archives
    /// do, the MX tracks' have the disc and the column alone.
    pub fn numbered(&self) -> bool {
        (1..=game_sim::markers::SLOTS).all(|n| self.word(&n.to_string()).is_some())
    }

    fn word(&self, name: &str) -> Option<&Mesh> {
        self.words.iter().find(|(n, _)| n == name).map(|(_, m)| m)
    }
}

/// The word over an activity's zone, `startzone_<word>`.
pub fn word_of(activity: Activity) -> &'static str {
    match activity {
        Activity::Hit => "hit",
        Activity::Run => "run",
        Activity::Race => "race",
        Activity::Stunt => "stunt",
    }
}

/// What a portal is: a zone's or a session marker's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A start zone, its activity's word and number over it.
    Zone(Activity),
    /// A session marker in its slot (from 0): the slot's number over it, and the selection's on it or not.
    Marker { slot: usize, selected: bool },
}

/// A portal built over the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct Portal {
    pub kind: Kind,
    /// Its middle, on the ground where it stands.
    pub at: [f32; 3],
    /// The way its arrow points, along the ground, `x` and `z`, of length 1.
    pub look: [f32; 2],
    /// Its disc's size across, along its side and its look, in feet.
    pub size: [f32; 2],
    /// Its size against a zone's: 1 for a zone, [`MARKER_FRACTION`] for a marker. The column and the
    /// words are scaled by it.
    pub fraction: f32,
    /// Its colour, `0..1` a channel.
    pub tint: [f32; 3],
    /// The zone's number in its progression, from 1, or the marker's slot's.
    pub number: Option<usize>,
    /// The disc, laid over the ground: each triangle's corners in the world and their texture
    /// coordinates.
    disc: Vec<[([f32; 3], [f32; 2]); 3]>,
}

/// A player's session markers' portals (`built`, one a slot, kept as long as the set), one a filled slot of `markers`, built
/// again where a marker has moved or the selection (`pick`) has come onto it or off it; none where
/// the view shows no markers (`pick` is `None`). `ground` is the ground's height under an `x`, `z`
/// looked for from a height `y`, or `None` off the ground.
pub fn markers(
    built: &mut Vec<Option<Portal>>,
    markers: &game_sim::markers::Markers,
    pick: Option<game_sim::markers::Pick>,
    mut ground: impl FnMut(f32, f32, f32) -> Option<f32>,
) {
    // One a slot, as many as the set has (P139: five, or more off the +).
    built.resize(markers.slots.len(), None);
    for (slot, built) in built.iter_mut().enumerate() {
        let Some((ride, pick)) = markers.slots[slot].as_ref().zip(pick) else {
            *built = None;
            continue;
        };
        let (at, (forward, _)) = (ride.position(), ride.axes());
        let (at, heading) = ([at.x(), at.y(), at.z()], [forward.x(), forward.z()]);
        let selected = pick == game_sim::markers::Pick::Slot(slot);
        if !built.as_ref().is_some_and(|p| p.at == at && p.kind == (Kind::Marker { slot, selected })) {
            // From a little over the bike, so a marker put down under a ledge finds its own ground.
            let from = at[1] + MARKER_LOOK_FROM;
            *built = Some(Portal::marker(slot, selected, at, heading, |x, z| ground(x, from, z)));
        }
    }
}

/// How far over a marker's bike the ground under its disc is looked for, in feet.
const MARKER_LOOK_FROM: f32 = 10.0;

/// A colour word's channels, `0..1`.
fn unit(c: [u8; 4]) -> [f32; 3] {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
}

/// `look` along the ground, of length 1 — straight down `z` for one with no length there.
fn flat(look: [f32; 2]) -> [f32; 2] {
    let l = (look[0] * look[0] + look[1] * look[1]).sqrt();
    if l > 1e-6 { [look[0] / l, look[1] / l] } else { [0.0, 1.0] }
}

impl Portal {
    /// A start zone's portal, full size, its disc laid over `ground` (the height of the ground at an
    /// `x`, `z`, or `None` off it, where the disc keeps the zone's height).
    pub fn zone(spot: &Spot, ground: impl FnMut(f32, f32) -> Option<f32>) -> Portal {
        let tint = unit(crate::overlay::activity_colour(spot.activity));
        Portal::build(Kind::Zone(spot.activity), spot.at, spot.look, spot.size, 1.0, tint, spot.number, ground)
    }

    /// A session marker's portal: a zone's at [`MARKER_FRACTION`] of its size, where the marker's bike
    /// was put down (`at`), facing the way it faced (`heading`, along the ground), in its slot's
    /// colour, with its slot's number over it.
    pub fn marker(slot: usize, selected: bool, at: [f32; 3], heading: [f32; 2], ground: impl FnMut(f32, f32) -> Option<f32>) -> Portal {
        let tint = unit(crate::overlay::marker_colour(slot));
        let size = [ZONE_SIZE * MARKER_FRACTION; 2];
        Portal::build(Kind::Marker { slot, selected }, at, heading, size, MARKER_FRACTION, tint, Some(slot + 1), ground)
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        kind: Kind,
        at: [f32; 3],
        look: [f32; 2],
        size: [f32; 2],
        fraction: f32,
        tint: [f32; 3],
        number: Option<usize>,
        mut ground: impl FnMut(f32, f32) -> Option<f32>,
    ) -> Portal {
        let look = flat(look);
        let side = [look[1], -look[0]];
        // A point of the unit disc (`x` along its side, `z` along its look, each -0.5 to 0.5), in the
        // world over the ground, and its texture coordinates: the arrow up the picture points along `z`.
        let mut point = |x: f32, z: f32| {
            let wx = at[0] + side[0] * x * size[0] + look[0] * z * size[1];
            let wz = at[2] + side[1] * x * size[0] + look[1] * z * size[1];
            let y = ground(wx, wz).unwrap_or(at[1]) + DISC_LIFT * fraction.max(0.25);
            ([wx, y, wz], [x + 0.5, 0.5 - z])
        };
        let mut rings = vec![vec![point(0.0, 0.0)]];
        for r in 1..=DISC_RINGS {
            let radius = 0.5 * r as f32 / DISC_RINGS as f32;
            rings.push(
                (0..DISC_ROUND)
                    .map(|k| {
                        let a = std::f32::consts::TAU * k as f32 / DISC_ROUND as f32;
                        point(radius * a.cos(), radius * a.sin())
                    })
                    .collect(),
            );
        }
        let mut disc = Vec::new();
        for k in 0..DISC_ROUND {
            let n = (k + 1) % DISC_ROUND;
            disc.push([rings[0][0], rings[1][k], rings[1][n]]);
            for r in 1..DISC_RINGS {
                let (i, o) = (&rings[r], &rings[r + 1]);
                disc.push([i[k], o[k], o[n]]);
                disc.push([i[k], o[n], i[n]]);
            }
        }
        Portal { kind, at, look, size, fraction, tint, number, disc }
    }

    /// How much more opaque than a zone's it is drawn: [`SELECTED_GAIN`] for the selected marker.
    fn gain(&self) -> f32 {
        match self.kind {
            Kind::Marker { selected: true, .. } => SELECTED_GAIN,
            _ => 1.0,
        }
    }

    /// The portal as a view sees it from `eye` looking along `forward`, `seconds` into the ride (the
    /// column's stripes crawl with it): the disc, the column, and the words over it, cut from `art`'s
    /// textures. The words face the camera, and rise as it draws away.
    pub fn glows(&self, art: &Art, eye: [f32; 3], forward: [f32; 3], seconds: f32) -> Vec<Glow> {
        let f = self.fraction;
        let gain = self.gain();
        let tint = |alpha: f32, c: [f32; 4]| [self.tint[0] * c[0], self.tint[1] * c[1], self.tint[2] * c[2], (alpha * gain * c[3]).min(1.0)];
        let mut out = Vec::new();
        // The disc.
        let a = tint(DISC_ALPHA, [1.0; 4]);
        let tris = self.disc.iter().map(|t| t.map(|(at, uv)| Vertex { at, uv, tint: a })).collect();
        out.push(Glow { texture: DISC, additive: false, tris });
        // The column round it, its stripes crawling.
        let [sx, sz] = [(self.size[0] - COLUMN_NARROWER * f).max(0.0), (self.size[1] - COLUMN_NARROWER * f).max(0.0)];
        let side = [self.look[1], 0.0, -self.look[0]];
        let look = [self.look[0], 0.0, self.look[1]];
        let middle = [self.at[0], self.at[1] + COLUMN_MIDDLE * f, self.at[2]];
        let crawl = [COLUMN_CRAWL[0] * seconds, COLUMN_CRAWL[1] * seconds];
        let place = |p: [f32; 3], scale: [f32; 3], x: [f32; 3], y: [f32; 3], z: [f32; 3], o: [f32; 3]| {
            let (px, py, pz) = (p[0] * scale[0], p[1] * scale[1], p[2] * scale[2]);
            [0, 1, 2].map(|i| o[i] + x[i] * px + y[i] * py + z[i] * pz)
        };
        let up = [0.0, 1.0, 0.0];
        let tris = art
            .column
            .iter()
            .map(|t| {
                t.map(|(p, uv, c)| Vertex {
                    at: place(p, [sx, COLUMN_HEIGHT * f, sz], side, up, look, middle),
                    uv: [uv[0] + crawl[0], uv[1] + crawl[1]],
                    tint: tint(COLUMN_ALPHA, c),
                })
            })
            .collect();
        out.push(Glow { texture: COLUMN, additive: true, tris });
        // The words, turned to the camera: the number low, the activity's word over it.
        let ahead = flat([forward[0], forward[2]]);
        let (z, x) = ([ahead[0], 0.0, ahead[1]], [ahead[1], 0.0, -ahead[0]]);
        let d = ((self.at[0] - eye[0]).powi(2) + (self.at[2] - eye[2]).powi(2)).sqrt();
        let low = self.at[1] + ((d / WORD_RISE_OVER).min(1.0) * WORD_RISE + WORD_LOW) * f;
        let mut word = |name: &str, height: f32, scale: f32| {
            let Some(mesh) = art.word(name) else { return };
            let o = [self.at[0], height, self.at[2]];
            let tris = mesh.iter().map(|t| t.map(|(p, uv, c)| Vertex { at: place(p, [scale; 3], x, up, z, o), uv, tint: tint(WORD_ALPHA, c) })).collect();
            out.push(Glow { texture: WORDS, additive: false, tris });
        };
        if let Some(n) = self.number {
            word(&n.to_string(), low, NUMBER_SCALE * f);
        }
        if let Kind::Zone(activity) = self.kind {
            word(word_of(activity), low + WORD_OVER_NUMBER * f, WORD_SCALE * f);
        }
        out
    }
}
