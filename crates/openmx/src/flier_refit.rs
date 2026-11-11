//! The fliers' files a livery flies off (MD80e, P315, P315b): its plane's file, ray probe, hull and
//! model's lift, and its helicopter, off a machine pack's skin where the seat picked one and the
//! disc's otherwise. The ride's load reads them once a livery, and since P315g the pause's Switch To
//! Machine reads them again for the one livery whose pick changed, so the switch flies the skin just
//! picked without leaving the ride.

use game_render::disc::Res;
use game_sim::flight_search::Rays;
use game_sim::phy;
use game_sim::track::Mesh;
use ps2_float::F;

use super::vehicle::{self, PackFlier};

/// One livery's plane files, what its flights are built off: the plane's `.phy` (the pack's, else
/// the disc's), whether it is MX vs. ATV's and which of its two models (`Some(true)` model one, on
/// SLUS-21104's steps), and its collision object's ray probe and hull.
#[derive(Clone, Default)]
pub struct PlaneFiles {
    pub phy: Option<Vec<u8>>,
    pub mxvatv: Option<bool>,
    pub rays: Option<Rays>,
    pub hull: Option<Mesh>,
}

impl PlaneFiles {
    /// `pack`'s, where the seat picked one, else the disc's out of the machines' archive `res`.
    /// Without the archive there are no rays or hull, as the disc's plane has none either.
    pub fn read(res: Option<&Res>, pack: Option<&PackFlier>) -> PlaneFiles {
        let disc = |name: &str| res.and_then(|res| res.read(&res.find_suffix(name)?.name).ok());
        let own = pack.and_then(|f| f.phy_bytes().map(|b| (b, f.model_one())));
        let mxvatv = own.as_ref().map(|(_, one)| *one);
        let phy = own.map(|(b, _)| b).or_else(|| disc(phy::BIPLANE_PHY));
        // Its collision object's ray probe (S48), which is what it strikes the ground and the
        // track's meshes with; without it the plane meets the world only with its wheels.
        let rays = res.and_then(|_| pack.and_then(|f| f.rays()).or_else(|| disc(game_sim::flight_search::BIPLANE_RAYS))).and_then(|bytes| {
            match game_sim::track::Rays::parse(&bytes) {
                Ok(r) => Some(Rays::of(&r)),
                Err(e) => {
                    eprintln!("biplane: {}: {e}", game_sim::flight_search::BIPLANE_RAYS);
                    None
                }
            }
        });
        // Its own hull (N22a), which the riders' bikes strike while a player flies it.
        let hull = res.and_then(|_| pack.and_then(|f| f.hull()).or_else(|| disc(game_sim::flight::HULL))).and_then(|bytes| {
            Mesh::parse(&bytes).map_err(|e| eprintln!("biplane: {}: {e}", game_sim::flight::HULL)).ok()
        });
        PlaneFiles { phy, mxvatv, rays, hull }
    }

    /// The plane's stock file the modifiers go on: MX vs. ATV's read for the skin's model.
    pub fn stock(&self) -> Option<phy::Plane> {
        let b = self.phy.as_deref()?;
        Some(match self.mxvatv {
            Some(one) => phy::Plane::read_mxvatv_bytes(b, one).plane,
            None => phy::Plane::read_bytes(b),
        })
    }
}

/// How high the original stands the plane's body over a file bay's ground (S164): the model's
/// bounds, off its own `.psg` — `pack`'s skin's, else the disc's.
pub fn plane_lift(res: Option<&Res>, pack: Option<&PackFlier>) -> Option<F> {
    let res = res?;
    let bytes = match pack.and_then(|f| f.psg()) {
        Some(bytes) => bytes,
        None => res.read(&res.find_suffix(game_sim::flight::BIPLANE_MODEL)?.name).ok()?,
    };
    game_sim::flight::spawn_lift(&bytes).map_err(|e| eprintln!("biplane: {}: {e}", game_sim::flight::BIPLANE_MODEL)).ok()
}

/// The helicopter (P216): `kiowa.phy`, which every player's garage flies a copy of, off the
/// machines' archive `res` — or a pack's (MD80e, P315): its file, collision pair and model, where
/// the seat picked one (`pack`, whose file is there); the thrown blades' own files stay the disc's,
/// which the pack has not.
pub fn kiowa_ride(res: &Res, pack: Option<&PackFlier>) -> Option<vehicle::KiowaRide> {
    let read = |name: &str| res.find_suffix(name).and_then(|e| res.read(&e.name).ok());
    let heli = match pack {
        Some(f) => vehicle::KiowaRide::new_mxvatv(&f.phy_bytes()?, f.model_one(), 0, None),
        None => vehicle::KiowaRide::new(&read(game_sim::kiowa::KIOWA_PHY)?, 0, None),
    };
    // And its collision object's ray probe (P216a), what it stands on the ground and strikes the
    // world with.
    let rays = match pack {
        Some(f) => f.rays(),
        None => read(game_sim::kiowa::KIOWA_RAYS),
    };
    let heli = match rays {
        Some(col) => heli.with_rays(&col),
        None => heli,
    };
    // And its model's nodes (S270), where its wreck throws the blades from.
    let psg = match pack {
        Some(f) => f.psg(),
        None => read(game_sim::kiowa::KIOWA_MODEL),
    };
    let heli = match psg {
        Some(psg) => heli.with_model(&psg),
        None => heli,
    };
    // And the blades it throws (P265), bodies of their own: the pack's files where it has them (MD109).
    let heli = match blade_files(res, pack) {
        BladeFiles { phy: Some(phy), psg: Some(psg), rays, shape, .. } => heli.with_blades(&phy, &psg, rays.as_deref(), shape.as_deref()),
        _ => heli,
    };
    // And the hull those blades meet (C21).
    let hull = match pack {
        Some(f) => f.hull(),
        None => read(game_sim::kiowa::KIOWA_HULL),
    };
    Some(match hull {
        Some(col) => heli.with_hull(&col),
        None => heli,
    })
}

/// The files a helicopter's thrown blades are built off (P265): `wreckblade.phy`, `bladebent.psg`
/// and its two collision files, the ray probe (`bladebentm.col`) and the mesh (`bladebentf.col`).
#[derive(Default)]
pub struct BladeFiles {
    pub phy: Option<Vec<u8>>,
    pub psg: Option<Vec<u8>>,
    pub rays: Option<Vec<u8>>,
    pub shape: Option<Vec<u8>>,
    /// How many of the four came off the pack.
    pub from_pack: usize,
}

/// The thrown blades' files (MD109): each the pack's where the seat flies one and the pack has the
/// file, by its name in any folder (MX vs. ATV keeps them as `Veh\WreckBlade.phy`,
/// `Veh\Anim\bladebent.psg`, `Veh\Col\BladeBent{M,F}.col`), else the disc's out of the machines'
/// archive `res`. SLUS-21104's helicopter init loads `bladebent` by that one name for both models:
/// the pack's `bladebent2.psg` goes unread here as it does there.
pub fn blade_files(res: &Res, pack: Option<&PackFlier>) -> BladeFiles {
    let mut files = BladeFiles::default();
    let mut read = |path: &str| {
        let name = path.rsplit('\\').next().unwrap_or(path);
        if let Some(b) = pack.and_then(|f| f.files.read_whole(name).ok()) {
            files.from_pack += 1;
            return Some(b);
        }
        res.find_suffix(path).and_then(|e| res.read(&e.name).ok())
    };
    let phy = read(game_sim::kiowa::WRECK_BLADE_PHY);
    let psg = read(game_sim::kiowa::BLADE_MODEL);
    let rays = read(game_sim::kiowa::BLADE_RAYS);
    let shape = read(game_sim::kiowa::BLADE_SHAPE);
    BladeFiles { phy, psg, rays, shape, ..files }
}

/// What a livery's flier of one kind is, to tell whether a new pick changes it: the pack and skin
/// it is off, or MX Unleashed's livery.
pub fn signature(fliers: &vehicle::Fliers, kind: vehicle::Machine) -> (Option<(String, String)>, usize) {
    let pack = match kind {
        vehicle::Machine::Biplane => fliers.plane.as_ref(),
        _ => fliers.kiowa.as_ref(),
    };
    (pack.map(|f| (f.id.clone(), f.skin.clone())), fliers.original_of(kind))
}
