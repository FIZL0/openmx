//! The animated toys the shell builds from the disc (V43), against first_recording's save state of
//! free09: its four seti windmills and its rustic windmill, as the original loaded them. Each toy the
//! shell builds is one of the state's, by its placement, with the state's skeleton (parents, rest
//! matrices), joint map and clip (duration, looping, every key) bit for bit.
//!
//! Needs the user's own disc and the state, so it skips with the reason when either is not there.

use game_render::animated_toy::Animated;
use game_render::disc::{Disc, Res};
use game_sim::math::Mat4;
use game_sim::toys::RotKeys;
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";

/// The fields (`research/oracle.txt` V43).
mod at {
    pub const MODEL: u32 = 0xb0;
    pub const HANDLE: u32 = 0x48;
    pub const ROOT: u32 = 0x30;
    pub const JOINT_COUNT: u32 = 0x230;
    pub const JOINTS: u32 = 0x234;
    pub const JOINT: u32 = 0x60;
    pub const NODE: u32 = 0x34;
    pub const LOCAL: u32 = 0x40;
    pub const PARENT: u32 = 0x100;
    pub const POSITIONS: u32 = 0x18;
    pub const ROTATIONS: u32 = 0x1c;
    pub const POSITION_COUNT: u32 = 0x20;
    pub const ROTATION_COUNT: u32 = 0x24;
    pub const FLAGS: u32 = 0x28;
    pub const DURATION: u32 = 0x3c;
    pub const COUNT: u32 = 0x8;
    pub const KEYS: u32 = 0x14;
}

fn mat(ee: &Ee, a: u32) -> [[u32; 4]; 4] {
    [0, 16, 32, 48].map(|r| [0, 4, 8, 12].map(|c| ee.u32_at(a + r + c)))
}

fn bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(Vf::to_bits)
}

fn free09() -> Option<Vec<Res>> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return None;
        }
    };
    let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with("\\free09.res"))?.name.clone();
    Some(vec![disc.res(&member).expect("free09's archive")])
}

#[test]
fn the_shells_toys_are_the_states() {
    let Some(mut archives) = free09() else { return };
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let state = kit.ctx("states").join(STATE);
    if !state.exists() {
        skip(&format!("{} is not there: `./kit state ram` of the human's first recording", state.display()));
        return;
    }
    let ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    let animated = Animated::load(&mut archives, "free09");
    assert!(animated.skipped.is_empty(), "{:?}", animated.skipped);
    assert_eq!(animated.sim.specs.len(), 5, "free09's rustic windmill and four seti windmills");
    assert_eq!(archives.len(), 1, "the archives came back");

    let vtable = addr("animated_toy_vtable").unwrap();
    let toys: Vec<u32> = (0..32u32 << 20).step_by(4).filter(|&a| ee.u32_at(a) == vtable).map(|a| a - 8).collect();
    assert_eq!(toys.len(), 5, "the state's animated toys");
    let mut matched = vec![false; toys.len()];
    for (k, spec) in animated.sim.specs.iter().enumerate() {
        let what = |field: &str| format!("toy {k} ({}) {field}", animated.models[k]);
        let found = toys.iter().position(|&toy| {
            let model = ee.u32_at(toy + at::MODEL);
            mat(&ee, ee.u32_at(model + at::ROOT) + at::LOCAL) == bits(&spec.place)
        });
        let Some(i) = found else { panic!("{}", what("is at no state toy's place")) };
        assert!(!matched[i], "{}", what("matches a toy already matched"));
        matched[i] = true;
        let model = ee.u32_at(toys[i] + at::MODEL);

        // The skeleton.
        let root = ee.u32_at(model + at::ROOT);
        let joints = ee.u32_at(model + at::JOINTS);
        let nodes: Vec<u32> =
            (0..ee.u32_at(model + at::JOINT_COUNT)).map(|j| ee.u32_at(joints + j * at::JOINT + at::NODE)).collect();
        let parents: Vec<Option<usize>> = nodes
            .iter()
            .map(|&n| {
                let p = ee.u32_at(n + at::PARENT);
                (p != root).then(|| nodes.iter().position(|&q| q == p).expect("a joint's parent"))
            })
            .collect();
        assert_eq!(spec.rig.parents, parents, "{}", what("parents"));
        // The state's locals are posed: a joint a channel drives holds the clip's rotation (rows 0-2)
        // or translation (row 3) there, so only what no channel writes is the rest.
        let motion = &animated.sim.motions[spec.motion];
        let driven = |names: Vec<u32>| -> Vec<usize> { names.iter().map(|&n| usize::from(spec.rig.map[n as usize])).collect() };
        let turned = driven(motion.rotation.iter().map(|c| c.name).collect());
        let moved = driven(motion.position.iter().map(|c| c.name).collect());
        let mut rests = 0;
        for (j, &n) in nodes.iter().enumerate() {
            let (ours, theirs) = (bits(&spec.rig.rest[j]), mat(&ee, n + at::LOCAL));
            let rows = if turned.contains(&j) { 0..0 } else { 0..3 };
            for r in rows.chain((!moved.contains(&j)).then_some(3..4).into_iter().flatten()) {
                assert_eq!(ours[r], theirs[r], "{}", what(&format!("joint {j} rest row {r}")));
                rests += 1;
            }
        }
        assert!(rests > 0, "{}", what("has no rest row to check"));

        // The clip and the map.
        let handle = ee.u32_at(model + at::HANDLE);
        let clip = ee.u32_at(handle + 4);
        assert_eq!(motion.timing.duration.to_bits(), ee.u32_at(clip + at::DURATION), "{}", what("duration"));
        assert_eq!(motion.timing.looping, ee.u32_at(clip + at::FLAGS) & 1 != 0, "{}", what("looping"));
        let list = |at_list, at_count| -> Vec<u32> {
            let (l, n) = (ee.u32_at(clip + at_list), ee.u32_at(clip + at_count));
            (0..n).map(|c| ee.u32_at(l + 4 * c)).collect()
        };
        let rot = list(at::ROTATIONS, at::ROTATION_COUNT);
        assert_eq!(motion.rotation.len(), rot.len(), "{}", what("rotation channels"));
        for (c, (ours, &ch)) in motion.rotation.iter().zip(&rot).enumerate() {
            assert_eq!(ours.name, ee.u32_at(ch), "{}", what(&format!("rotation {c} name")));
            let (n, keys) = (ee.u32_at(ch + at::COUNT), ee.u32_at(ch + at::KEYS));
            let theirs: Vec<u32> = match &ours.keys {
                RotKeys::Wide(_) => (0..n * 3).map(|w| ee.u32_at(keys + 4 * w)).collect(),
                RotKeys::Narrow(_) => (0..n * 2).map(|w| ee.u32_at(keys + 4 * w)).collect(),
            };
            let mine: Vec<u32> = match &ours.keys {
                RotKeys::Wide(k) => k
                    .iter()
                    .flat_map(|k| {
                        let a = k.axis.map(|a| a as u16 as u32);
                        [k.t.to_bits(), u32::from(k.turn) | a[0] << 16, a[1] | a[2] << 16]
                    })
                    .collect(),
                RotKeys::Narrow(k) => k
                    .iter()
                    .flat_map(|k| {
                        let a = k.axis.map(|a| a as u8 as u32);
                        [k.t.to_bits(), u32::from(k.turn) | a[0] << 8 | a[1] << 16 | a[2] << 24]
                    })
                    .collect(),
            };
            assert_eq!(mine, theirs, "{}", what(&format!("rotation {c} keys")));
        }
        let pos = list(at::POSITIONS, at::POSITION_COUNT);
        assert_eq!(motion.position.len(), pos.len(), "{}", what("position channels"));
        for (c, (ours, &ch)) in motion.position.iter().zip(&pos).enumerate() {
            assert_eq!(ours.name, ee.u32_at(ch), "{}", what(&format!("position {c} name")));
            let (n, keys) = (ee.u32_at(ch + at::COUNT), ee.u32_at(ch + at::KEYS));
            let theirs: Vec<u32> = (0..n * 4).map(|w| ee.u32_at(keys + 4 * w)).collect();
            let mine: Vec<u32> = ours.keys.iter().flat_map(|k| [k.t.to_bits(), k.p[0].to_bits(), k.p[1].to_bits(), k.p[2].to_bits()]).collect();
            assert_eq!(mine, theirs, "{}", what(&format!("position {c} keys")));
        }
        let names = motion.rotation.iter().map(|c| c.name).chain(motion.position.iter().map(|c| c.name)).max().map_or(0, |n| n + 1);
        let map_at = ee.u32_at(handle);
        let theirs: Vec<u8> = (0..names).map(|n| ee.u8_at(map_at + n)).collect();
        assert_eq!(spec.rig.map[..names as usize], theirs[..], "{}", what("joint map"));
    }
    assert!(matched.iter().all(|&m| m), "every state toy is one of the shell's");
}
