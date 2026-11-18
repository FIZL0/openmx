// --- MD81f: Untamed's sandrail drives (the buggy/baja class), SLUS-21701's frame and the port's beside it

use super::untamed::*;
use super::untamed_terrain::terrain_test;

/// The baja's class vtable, which Untamed's sandrail is (car kind 8; research/md81f/addresses.txt).
const U_BAJA_VTABLE: u32 = 0x003f_09d0;
/// The player's sandrail in MD81f's spawn image (Free Ride, Rocksbury).
const SANDRAIL: u32 = 0x01be_46c0;

const S_THROTTLE: UDrive = UDrive { set: "md81f", name: "throttle", image: "spawn", car: SANDRAIL, vtable: U_BAJA_VTABLE, before: true };
const S_THROTTLE_2: UDrive = UDrive { set: "md81f", name: "throttle-2", image: "spawn", car: SANDRAIL, vtable: U_BAJA_VTABLE, before: true };
const S_STEER: UDrive = UDrive { set: "md81f", name: "steer", image: "spawn", car: SANDRAIL, vtable: U_BAJA_VTABLE, before: true };
const S_JUMP: UDrive = UDrive { set: "md81f", name: "jump", image: "spawn", car: SANDRAIL, vtable: U_BAJA_VTABLE, before: true };

/// The interpreter's frames against MD81f's capture. This recorder reads each row before its frame
/// (row 0 is the spawn image), so every row is the interpreter's after the frames before it, bit for
/// bit (body, engine, tires), but the late reads (read after the frame: the frame's own, throttle's
/// row 27; throttle-2 holds it early). No row is torn, and the drive moves the car.
fn sandrail_capture_test(d: &UDrive, late_rows: &[usize]) {
    let Some((n, early, torn)) = u_capture_rows(d) else { return };
    assert!(torn.is_empty(), "{}: torn rows {torn:?}", d.name);
    let late: Vec<usize> = (0..n).filter(|r| !early.contains(r)).collect();
    assert_eq!(late, late_rows, "{}: the rows read after their frame", d.name);
    let rows = u_rows(d.set, d.name).expect("read");
    assert_ne!(rows[0]["body"], rows[n - 1]["body"], "{}: the sandrail never moved", d.name);
}

#[test]
fn sandrail_throttle_capture() {
    sandrail_capture_test(&S_THROTTLE, &[27]);
}

#[test]
fn sandrail_throttle_2_capture() {
    sandrail_capture_test(&S_THROTTLE_2, &[]);
}

#[test]
fn sandrail_steer_capture() {
    sandrail_capture_test(&S_STEER, &[]);
}

#[test]
fn sandrail_jump_capture() {
    sandrail_capture_test(&S_JUMP, &[]);
}

/// Drive `d` beside, through the frame or the Ride: every frame of the drive compared, word for word
/// against SLUS-21701's, those with a tire on an object too (`on_objects`, answered since MD233:
/// throttle's rough ground past the whoops); `wrecked` the frames compared wrecked (steer's
/// wall).
fn sandrail_beside_test(d: &UDrive, ride: bool, on_objects: &[usize], wrecked: usize) {
    let Some(s) = u_beside(d, ride) else { return };
    let n = u_rows(d.set, d.name).expect("read").len();
    assert_eq!(s.on_objects, on_objects, "{}: frames on objects", d.name);
    assert_eq!((s.frames, s.wrecked), (n, wrecked), "{}: frames compared, wrecked", d.name);
    assert_eq!(s.respawned, Vec::<usize>::new(), "{}: frames put back", d.name);
}

/// Throttle's frames with a tire on an object.
const THROTTLE_ON_OBJECTS: [usize; 16] = [111, 112, 145, 147, 148, 149, 150, 155, 156, 157, 158, 292, 293, 294, 299, 307];

#[test]
fn sandrail_throttle_frame_beside() {
    sandrail_beside_test(&S_THROTTLE, false, &THROTTLE_ON_OBJECTS, 0);
}

#[test]
fn sandrail_throttle_ride_beside() {
    sandrail_beside_test(&S_THROTTLE, true, &THROTTLE_ON_OBJECTS, 0);
}

#[test]
fn sandrail_steer_frame_beside() {
    sandrail_beside_test(&S_STEER, false, &[], 17);
}

#[test]
fn sandrail_steer_ride_beside() {
    sandrail_beside_test(&S_STEER, true, &[], 17);
}

#[test]
fn sandrail_jump_frame_beside() {
    sandrail_beside_test(&S_JUMP, false, &[], 0);
}

#[test]
fn sandrail_jump_ride_beside() {
    sandrail_beside_test(&S_JUMP, true, &[], 0);
}

#[test]
fn sandrail_throttle_ride_on_the_ports_terrain() {
    terrain_test(&S_THROTTLE, true);
}

#[test]
fn sandrail_steer_ride_on_the_ports_terrain() {
    terrain_test(&S_STEER, true);
}

#[test]
fn sandrail_jump_ride_on_the_ports_terrain() {
    terrain_test(&S_JUMP, true);
}
