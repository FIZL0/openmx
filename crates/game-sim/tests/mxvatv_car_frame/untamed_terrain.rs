// --- MD234: MD160's Untamed drives with the ground and the noise from the port ----------------------

use super::untamed::*;
use super::*;
use game_sim::atv_ground;
use game_sim::noise::SmoothNoise;

/// SLUS-21701's smooth noise tables (the permutation's words and the gradients) and the word that
/// says they are not built yet: SLUS-21104's [`super::whole::NOISE_PERM`] layout, moved (research/oracle.txt
/// `# MD234`).
const U_NOISE_PERM: u32 = 0x0040_8080;
const U_NOISE_G1: u32 = 0x0040_b0c0;
const U_NOISE_UNBUILT: u32 = 0x003a_d638;

/// The port's world round the truck: Rocksbury's terrain (Free Ride's map in MD160's images) for
/// the ground queries, the smooth noise on the image's tables for the engine's; and how many of
/// each it answered in a frame.
#[derive(Clone)]
pub(crate) struct UPort {
    pub(crate) terrain: &'static atv_ground::Terrain<'static>,
    pub(crate) noise: SmoothNoise,
    pub(crate) answered: usize,
    pub(crate) noised: usize,
}

/// Rocksbury's terrain file, `Env\SX_A\SX_A.ps2.trn` out of the disc's `SX_A.res` (MD168b's copy).
fn rocksbury_terrain() -> Option<&'static atv_ground::Terrain<'static>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let at = kit.ctx("untamed/sx_a").join(format!("SX_A.ps2{}", ".trn"));
    if !at.exists() {
        skip(&format!("{} is not there: Rocksbury's terrain out of Untamed's SX_A.res (MD168b)", at.display()));
        return None;
    }
    let trn: &'static [u8] = Box::leak(std::fs::read(&at).unwrap().into_boxed_slice());
    let map: &'static game_sim::track::Terrain<'static> = Box::leak(Box::new(game_sim::track::Terrain::parse(trn).unwrap()));
    Some(Box::leak(Box::new(atv_ground::Terrain::of(map))))
}

/// The port's world for drive `d`: the terrain, and the noise on the tables `d`'s image holds (built
/// before the image was taken; the original builds them off `rand()` on its first sample).
fn port_of(d: &UDrive) -> Option<UPort> {
    let terrain = rocksbury_terrain()?;
    let pre = u_image(d.image)?;
    assert_eq!(pre.u32_at(U_NOISE_UNBUILT), 0, "{}: the noise's tables are built", d.name);
    let perm: [u32; 256] = std::array::from_fn(|k| pre.u32_at(U_NOISE_PERM + 4 * k as u32));
    let mut sorted = perm;
    sorted.sort();
    assert!(sorted.iter().enumerate().all(|(k, &x)| x == k as u32), "{}: no permutation at the noise's table", d.name);
    let noise = SmoothNoise::from_tables(&perm, &std::array::from_fn(|k| F(pre.f32_at(U_NOISE_G1 + 4 * k as u32))));
    Some(UPort { terrain, noise, answered: 0, noised: 0 })
}

/// Drive `d` beside, through the frame or the Ride, with every ground query answered by
/// [`atv_ground::query`] on Rocksbury and every noise sample by [`SmoothNoise::at_accumulated`]: the
/// answers the original's own ground query and noise gave are only checked against.
fn terrain_test(d: &UDrive, ride: bool) {
    let Some(port) = port_of(d) else { return };
    let Some(s) = u_beside_with(d, ride, Some(port)) else { return };
    assert!(s.frames > 0, "{}: no frame compared", d.name);
    assert!(s.port_grounds > 0, "{}: the port's terrain answered nothing", d.name);
    assert!(s.port_noises > 0, "{}: the port's noise answered nothing", d.name);
}

#[test]
fn untamed_throttle_on_the_ports_terrain() {
    terrain_test(&U_THROTTLE, false);
}

#[test]
fn untamed_steer_on_the_ports_terrain() {
    terrain_test(&U_STEER, false);
}

#[test]
fn untamed_jump_on_the_ports_terrain() {
    terrain_test(&U_JUMP, false);
}

#[test]
fn untamed_wreck_on_the_ports_terrain() {
    terrain_test(&U_WRECK, false);
}

#[test]
fn untamed_throttle_ride_on_the_ports_terrain() {
    terrain_test(&U_THROTTLE, true);
}

#[test]
fn untamed_steer_ride_on_the_ports_terrain() {
    terrain_test(&U_STEER, true);
}

#[test]
fn untamed_jump_ride_on_the_ports_terrain() {
    terrain_test(&U_JUMP, true);
}

#[test]
fn untamed_wreck_ride_on_the_ports_terrain() {
    terrain_test(&U_WRECK, true);
}
