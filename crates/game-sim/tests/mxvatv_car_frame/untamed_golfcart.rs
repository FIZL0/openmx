// --- MD170: MD81e's Untamed golf-cart drives, SLUS-21701's frame and the port's beside it

use super::untamed::*;
use super::untamed_terrain::terrain_test;

/// The golf cart's class vtable (car kind 7; research/md81e/addresses.txt).
const U_GOLF_VTABLE: u32 = 0x003f_1370;
/// The player's cart in MD81e's spawn image (Free Ride, Rocksbury; kept-context/md81e/addresses.txt).
const CART: u32 = 0x01be_4740;

const G_THROTTLE: UDrive = UDrive { set: "md81e", name: "throttle", image: "spawn", car: CART, vtable: U_GOLF_VTABLE, before: false };
const G_THROTTLE_2: UDrive = UDrive { set: "md81e", name: "throttle2", image: "spawn", car: CART, vtable: U_GOLF_VTABLE, before: false };
const G_ANALOG: UDrive = UDrive { set: "md81e", name: "analog", image: "spawn", car: CART, vtable: U_GOLF_VTABLE, before: false };
const G_STEER: UDrive = UDrive { set: "md81e", name: "steer", image: "spawn", car: CART, vtable: U_GOLF_VTABLE, before: false };
const G_JUMP: UDrive = UDrive { set: "md81e", name: "jump", image: "spawn", car: CART, vtable: U_GOLF_VTABLE, before: false };

/// The rows where every tire's touch column of MD81e's capture reads 0.
fn rows_off(d: &UDrive) -> Vec<usize> {
    let rows = u_rows(d.set, d.name).expect("read");
    (0..rows.len()).filter(|&r| (0..4).all(|k| rows[r][&format!("tire{k}.touch")].parse::<f32>().unwrap() == 0.0)).collect()
}

/// The interpreter's frames against MD81e's capture. This recorder reads each row after its frame
/// (row 0 after frame 1, as MD160's), so every row is the interpreter's after its frame, bit for bit
/// (body, engine, four tires), but `early` (read before it) and `torn` (read while it ran: the rows
/// throttle2 and jump's recorder fell behind on; throttle, the same tape, reads none); and the drive
/// moves the cart.
fn golf_capture_test(d: &UDrive, early_rows: &[usize], torn_rows: &[usize]) {
    let Some((n, early, torn)) = u_capture_rows(d) else { return };
    assert_eq!(early, early_rows, "{}: the rows read before their frame", d.name);
    assert_eq!(torn, torn_rows, "{}: the torn rows", d.name);
    let rows = u_rows(d.set, d.name).expect("read");
    assert_ne!(rows[0]["body"], rows[n - 1]["body"], "{}: the cart never moved", d.name);
}

/// The hop: all four tires off the ground on rows 272–324.
#[test]
fn golfcart_throttle_capture() {
    golf_capture_test(&G_THROTTLE, &[], &[]);
    if u_rows(G_THROTTLE.set, G_THROTTLE.name).is_some() {
        assert!((272..=324).all(|r| rows_off(&G_THROTTLE).contains(&r)), "the hop's rows");
    }
}

#[test]
fn golfcart_throttle_2_capture() {
    golf_capture_test(&G_THROTTLE_2, &[113, 114, 115, 118, 123, 124, 125, 126, 128, 135, 146], &[119, 122, 127, 136, 138, 142, 168, 266, 349]);
}

/// Cross's pressure stepped through bytes 21–49: the gas comes on at byte 27 (row 33, read after frame
/// 47). The recorder missed rows (vsync 4 or 26 apart, not 2): 211 rows over 225 frames, each row
/// still the interpreter's after the frame it was read after.
#[test]
fn golfcart_analog_capture() {
    golf_capture_test(&G_ANALOG, &[], &[]);
    if let Some(rows) = u_rows(G_ANALOG.set, G_ANALOG.name) {
        let gas = (0..rows.len()).find(|&r| rows[r]["gas.x"].parse::<f32>().unwrap() > 0.0);
        assert_eq!(gas, Some(33), "the first row with gas");
        assert_eq!(u_frames(&G_ANALOG, &rows)[32..34], [46, 47], "the frames rows 32 and 33 were read after");
        assert_eq!(u_frames(&G_ANALOG, &rows).last(), Some(&225), "the last row's frame");
    }
}

#[test]
fn golfcart_steer_capture() {
    golf_capture_test(&G_STEER, &[], &[]);
}

/// The jump's rough landing wrecks the cart: car state 14 on rows 234–294, put back on row 295.
#[test]
fn golfcart_jump_capture() {
    golf_capture_test(&G_JUMP, &[242], &[253, 272]);
    if let Some(rows) = u_rows(G_JUMP.set, G_JUMP.name) {
        let wrecked: Vec<usize> = (0..rows.len()).filter(|&r| rows[r]["carstate"].parse::<f32>().unwrap() == 14.0).collect();
        assert_eq!(wrecked, (234..295).collect::<Vec<_>>(), "the wrecked rows");
    }
}

/// Drive `d` beside, through the frame or the Ride: every frame of the drive compared, word for word
/// against SLUS-21701's, those with a tire on an object too (`on_objects`, each ask answered with
/// what the original's tire stood on); `frames` compared, `wrecked` of them wrecked, `put_back` the
/// frames put back.
fn golf_beside_test(d: &UDrive, ride: bool, frames: usize, on_objects: &[usize], wrecked: usize, put_back: &[usize]) {
    let Some(s) = u_beside(d, ride) else { return };
    assert_eq!(s.on_objects, on_objects, "{}: frames on objects", d.name);
    assert_eq!(s.cofgs, s.stands_on, "{}: asks and centres of mass written back", d.name);
    assert_eq!((s.frames, s.wrecked), (frames, wrecked), "{}: frames compared, wrecked", d.name);
    assert_eq!(s.respawned, put_back, "{}: frames put back", d.name);
}

const THROTTLE_ON_OBJECTS: [usize; 7] = [113, 114, 115, 116, 117, 118, 274];

#[test]
fn golfcart_throttle_frame_beside() {
    golf_beside_test(&G_THROTTLE, false, 370, &THROTTLE_ON_OBJECTS, 0, &[]);
}

#[test]
fn golfcart_throttle_ride_beside() {
    golf_beside_test(&G_THROTTLE, true, 370, &THROTTLE_ON_OBJECTS, 0, &[]);
}

#[test]
fn golfcart_throttle_2_ride_beside() {
    golf_beside_test(&G_THROTTLE_2, true, 370, &THROTTLE_ON_OBJECTS, 0, &[]);
}

/// The 14 frames of the rows the recorder missed are compared too.
#[test]
fn golfcart_analog_frame_beside() {
    golf_beside_test(&G_ANALOG, false, 225, &[143, 144, 145, 146, 147, 148, 149], 0, &[]);
}

#[test]
fn golfcart_analog_ride_beside() {
    golf_beside_test(&G_ANALOG, true, 225, &[143, 144, 145, 146, 147, 148, 149], 0, &[]);
}

#[test]
fn golfcart_steer_frame_beside() {
    golf_beside_test(&G_STEER, false, 280, &[268, 269, 271, 272, 274], 0, &[]);
}

#[test]
fn golfcart_steer_ride_beside() {
    golf_beside_test(&G_STEER, true, 280, &[268, 269, 271, 272, 274], 0, &[]);
}

/// The landing's wreck: frames 235–295 wrecked (61), put back on frame 296 through Untamed's respawn.
#[test]
fn golfcart_jump_frame_beside() {
    golf_beside_test(&G_JUMP, false, 370, &[], 61, &[296]);
}

#[test]
fn golfcart_jump_ride_beside() {
    golf_beside_test(&G_JUMP, true, 370, &[], 61, &[296]);
}

#[test]
fn golfcart_throttle_ride_on_the_ports_terrain() {
    terrain_test(&G_THROTTLE, true);
}

#[test]
fn golfcart_analog_ride_on_the_ports_terrain() {
    terrain_test(&G_ANALOG, true);
}

#[test]
fn golfcart_steer_ride_on_the_ports_terrain() {
    terrain_test(&G_STEER, true);
}

#[test]
fn golfcart_jump_ride_on_the_ports_terrain() {
    terrain_test(&G_JUMP, true);
}
