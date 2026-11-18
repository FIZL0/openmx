// --- MD163a: MD164's Untamed baja (trophy truck) drives, SLUS-21701's frame and the port's beside it

use super::untamed::*;
use super::untamed_terrain::terrain_test;

/// The baja's class vtable (MD164's trophy truck, car kind 6).
const U_BAJA_VTABLE: u32 = 0x003f_09d0;
/// The player's baja in MD164's spawn and air images (Free Ride, Rocksbury; kept-context/md164/addresses.txt).
const BAJA: u32 = 0x01be_4740;

const B_THROTTLE: UDrive = UDrive { set: "md164", name: "throttle", image: "spawn", car: BAJA, vtable: U_BAJA_VTABLE, before: true };
const B_THROTTLE_2: UDrive = UDrive { set: "md164", name: "throttle-2", image: "spawn", car: BAJA, vtable: U_BAJA_VTABLE, before: true };
const B_STEER: UDrive = UDrive { set: "md164", name: "steer", image: "spawn", car: BAJA, vtable: U_BAJA_VTABLE, before: true };
const B_BRAKE: UDrive = UDrive { set: "md164", name: "brake", image: "spawn", car: BAJA, vtable: U_BAJA_VTABLE, before: true };
const B_JUMP: UDrive = UDrive { set: "md164", name: "jump", image: "spawn", car: BAJA, vtable: U_BAJA_VTABLE, before: true };
const B_PRELOAD: UDrive = UDrive { set: "md164", name: "preload", image: "spawn", car: BAJA, vtable: U_BAJA_VTABLE, before: true };
const B_LAND: UDrive = UDrive { set: "md164", name: "land", image: "air", car: BAJA, vtable: U_BAJA_VTABLE, before: true };

/// The rows where a decoded column of MD164's capture reads `want`.
fn rows_where(d: &UDrive, column: &str, want: f32) -> Vec<usize> {
    let rows = u_rows(d.set, d.name).expect("read");
    (0..rows.len()).filter(|&r| rows[r][column].parse::<f32>().unwrap() == want).collect()
}

/// The interpreter's frames against MD164's capture. This recorder reads each row before its frame
/// (row 0 is the image), so every row is the interpreter's after the frames before it, bit for bit
/// (body, engine, four tires): no row is read late or torn, and the drive moves the car.
fn baja_capture_test(d: &UDrive) {
    let Some((n, early, torn)) = u_capture_rows(d) else { return };
    assert!(torn.is_empty(), "{}: torn rows {torn:?}", d.name);
    let late: Vec<usize> = (0..n).filter(|r| !early.contains(r)).collect();
    assert_eq!(late, Vec::<usize>::new(), "{}: the rows read after their frame", d.name);
    let rows = u_rows(d.set, d.name).expect("read");
    assert_ne!(rows[0]["body"], rows[n - 1]["body"], "{}: the baja never moved", d.name);
}

#[test]
fn baja_throttle_capture() {
    baja_capture_test(&B_THROTTLE);
}

#[test]
fn baja_throttle_2_capture() {
    baja_capture_test(&B_THROTTLE_2);
}

/// Steer's natural wreck: car state 14 on rows 209–269, put back on row 270 (the respawn at the spot
/// Free Ride's script keeps, laid in off the row before the frame).
#[test]
fn baja_steer_capture() {
    baja_capture_test(&B_STEER);
    if u_rows(B_STEER.set, B_STEER.name).is_some() {
        assert_eq!(rows_where(&B_STEER, "carstate", 14.0), (209..270).collect::<Vec<_>>(), "the wrecked rows");
    }
}

#[test]
fn baja_brake_capture() {
    baja_capture_test(&B_BRAKE);
}

#[test]
fn baja_jump_capture() {
    baja_capture_test(&B_JUMP);
}

/// The preload arms on row 119 and fires on row 123 (armed cleared, its timer run to −1/120).
#[test]
fn baja_preload_capture() {
    baja_capture_test(&B_PRELOAD);
    if let Some(rows) = u_rows(B_PRELOAD.set, B_PRELOAD.name) {
        assert_eq!(rows[123]["pretime.y"].parse::<f32>().unwrap().to_bits(), 0xbc08_850a, "the fire row's timer");
    }
}

/// From the air image, held to land.csv (it parts from jump.csv's tail at the first touch). Its first
/// frame runs on the released pad of the load.
#[test]
fn baja_land_capture() {
    baja_capture_test(&B_LAND);
}

/// Drive `d` beside, through the frame or the Ride: every frame of the drive compared, word for word
/// against SLUS-21701's, those with a tire on an object too (`on_objects`, each ask answered with
/// what the original's tire stood on); `wrecked` the frames compared wrecked, `put_back` the frames
/// put back. Returns what it saw.
fn baja_beside_test(d: &UDrive, ride: bool, on_objects: &[usize], wrecked: usize, put_back: &[usize]) -> Option<USeen> {
    let s = u_beside(d, ride)?;
    let n = u_rows(d.set, d.name).expect("read").len();
    assert_eq!(s.on_objects, on_objects, "{}: frames on objects", d.name);
    assert_eq!(s.cofgs, s.stands_on, "{}: asks and centres of mass written back", d.name);
    assert_eq!((s.frames, s.wrecked), (n, wrecked), "{}: frames compared, wrecked", d.name);
    assert_eq!(s.respawned, put_back, "{}: frames put back", d.name);
    Some(s)
}

/// Steer's wreck: frames 209–269 wrecked (61), put back on frame 270 through Untamed's respawn.
fn baja_steer_test(ride: bool) {
    baja_beside_test(&B_STEER, ride, &[], 61, &[270]);
}

/// The preload's fire on frame 123 posts its cue once, the port's as the original's.
fn baja_preload_test(ride: bool) {
    let Some(s) = baja_beside_test(&B_PRELOAD, ride, &[], 0, &[]) else { return };
    assert_eq!(s.handed.get("PreloadCue"), Some(&1), "the preload's cues");
}

#[test]
fn baja_throttle_frame_beside() {
    baja_beside_test(&B_THROTTLE, false, &[113], 0, &[]);
}

#[test]
fn baja_throttle_ride_beside() {
    baja_beside_test(&B_THROTTLE, true, &[113], 0, &[]);
}

#[test]
fn baja_steer_frame_beside() {
    baja_steer_test(false);
}

#[test]
fn baja_steer_ride_beside() {
    baja_steer_test(true);
}

#[test]
fn baja_brake_frame_beside() {
    baja_beside_test(&B_BRAKE, false, &[116, 117], 0, &[]);
}

#[test]
fn baja_brake_ride_beside() {
    baja_beside_test(&B_BRAKE, true, &[116, 117], 0, &[]);
}

#[test]
fn baja_jump_frame_beside() {
    baja_beside_test(&B_JUMP, false, &[], 0, &[]);
}

#[test]
fn baja_jump_ride_beside() {
    baja_beside_test(&B_JUMP, true, &[], 0, &[]);
}

#[test]
fn baja_preload_frame_beside() {
    baja_preload_test(false);
}

#[test]
fn baja_preload_ride_beside() {
    baja_preload_test(true);
}

#[test]
fn baja_land_frame_beside() {
    baja_beside_test(&B_LAND, false, &[], 0, &[]);
}

#[test]
fn baja_land_ride_beside() {
    baja_beside_test(&B_LAND, true, &[], 0, &[]);
}

#[test]
fn baja_throttle_ride_on_the_ports_terrain() {
    terrain_test(&B_THROTTLE, true);
}

#[test]
fn baja_steer_ride_on_the_ports_terrain() {
    terrain_test(&B_STEER, true);
}

#[test]
fn baja_brake_ride_on_the_ports_terrain() {
    terrain_test(&B_BRAKE, true);
}

#[test]
fn baja_jump_ride_on_the_ports_terrain() {
    terrain_test(&B_JUMP, true);
}

#[test]
fn baja_preload_ride_on_the_ports_terrain() {
    terrain_test(&B_PRELOAD, true);
}

#[test]
fn baja_land_ride_on_the_ports_terrain() {
    terrain_test(&B_LAND, true);
}
