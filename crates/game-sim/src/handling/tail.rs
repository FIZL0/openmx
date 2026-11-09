//! **SLUS-21104's sim frame tail, and the bike's vehicle update in it** (F31b4a): what runs after the
//! physics world's step, once a frame. Where the original keeps it and its calls: research/oracle.txt
//! `# F31b4a`.
//!
//! The tail is a walk of the sim's listener tree ([`walk`]): a group with its step bit set steps each
//! of its listeners that is on, neither held nor gone, and asks to be stepped. A listener's step is
//! its own; a group's step is the walk again, one level down. On state 4's world the walk steps three
//! listeners, and one of them is the bike: [`vehicle_update`].
//!
//! The bike's vehicle update, in order:
//!
//! 1. with the game paused, nothing (the pause is the outside world's answer, [`Vehicles::paused`]);
//! 2. the bike's own update ([`inner`]): the fade and the wreck test, or with the bike down the wreck's
//!    three steps and its own listeners' walk; the put-back countdown; the fade's end; and the vehicle's
//!    base update ([`base`]): its clocks, its ride (down or not), and the vehicle's parts and listeners;
//! 3. the grounded words and the stuck timer ([`stuck`]): a bike on the ground with a ray down that
//!    has sat past its surface's limit is put back;
//! 4. the pad's rumble off the rougher wheel, past 10 ft/s;
//! 5. the wheelie and stoppie clocks, their bests and the trick they start after a second;
//! 6. the run's clocks: time, top speed, distance along the bike's heading, landings, and the run's
//!    end with its listeners told;
//! 7. the lap clock, the frame clock and the frame's message.
//!
//! Everything the update calls outside the bike's own words goes through [`Vehicles`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::node::Ram;
use super::seed::Words;
use crate::air::normalize;
use ps2_float::{F, Vf, fpu};

/// A group's flags (`+0x1c`): [`STEPS`] set, the walk steps its listeners.
pub const GROUP_FLAGS: u32 = 0x1c;
pub const STEPS: u32 = 2;
/// A group's first listener (`+0x10`) and a listener's next (`+0xc`).
pub const FIRST: u32 = 0x10;
pub const NEXT: u32 = 0xc;
/// A listener's state byte (`+0x21`): [`ON`] set, [`HELD`] and [`GONE`] clear, it may be stepped.
pub const STATE_BYTE: u32 = 0x21;
pub const ON: u32 = 1;
pub const HELD: u32 = 4;
pub const GONE: u32 = 8;
/// A listener's wants (`+0x18`): [`WANTS_STEP`] set, it is stepped.
pub const WANTS: u32 = 0x18;
pub const WANTS_STEP: u32 = 2;

/// The listener tree as the walk reads it: words, and each listener's step. The walk reads a
/// listener's next only after the listener has stepped.
pub trait Listeners {
    fn word(&mut self, at: u32) -> u32;
    fn step(&mut self, listener: u32, dt: F);
}

/// Whether the listener at `e` is stepped by its group's walk.
pub fn steps(l: &mut impl Listeners, e: u32) -> bool {
    let at = e + STATE_BYTE;
    let b = l.word(at & !3) >> (8 * (at & 3)) & 0xff;
    b & ON != 0 && b & HELD == 0 && b & GONE == 0 && l.word(e + WANTS) & WANTS_STEP != 0
}

/// The walk of `group`'s listeners: with the group's step bit set, each listener in turn that
/// [`steps`] is stepped with `dt`.
pub fn walk(l: &mut impl Listeners, group: u32, dt: F) {
    if l.word(group + GROUP_FLAGS) & STEPS == 0 {
        return;
    }
    let mut e = l.word(group + FIRST);
    while e != 0 {
        if steps(l, e) {
            l.step(e, dt);
        }
        e = l.word(e + NEXT);
    }
}

/// The sim's element lists (its records at `+0x24` and its after-tail list at `+0x3c`), each
/// `{count, _, array}`: the count a signed word, the array a word of addresses.
pub const LIST_COUNT: u32 = 0;
pub const LIST_ARRAY: u32 = 8;

/// The `i`th element of the sim list at `list`. Unchecked, as the original's getter: any `i` reads
/// the array's word at `4 * i`.
pub fn list_get(m: &mut impl Ram, list: u32, i: i32) -> u32 {
    let array = m.word(list + LIST_ARRAY);
    m.word(array.wrapping_add((i as u32).wrapping_mul(4)))
}

/// Takes the `i`th element out of the sim list at `list`: the last element moves into its place and
/// the count drops by one. Answers false, changing nothing, for an empty list, one with no array, or
/// an `i` at or past the count (a negative `i` is not refused: it writes before the array). The count
/// is read again after the move, which may have written it.
pub fn list_remove(m: &mut impl Ram, list: u32, i: i32) -> bool {
    let n = m.word(list + LIST_COUNT) as i32;
    if n < 1 {
        return false;
    }
    let array = m.word(list + LIST_ARRAY);
    if array == 0 || i >= n {
        return false;
    }
    let last = m.word((n as u32).wrapping_mul(4).wrapping_add(array).wrapping_sub(4));
    m.set(array.wrapping_add((i as u32).wrapping_mul(4)), last);
    let n = m.word(list + LIST_COUNT);
    m.set(list + LIST_COUNT, n.wrapping_sub(1));
    true
}

/// The bike's state word (`+0xbc4`, what the bike rides in) and its kind (the state's `+0x28`): the
/// wreck's kinds, and the one the pad rumbles and the lap clock runs in.
pub const STATE: u32 = 0xbc4;
pub const KIND: u32 = 0x28;
pub const WRECK_KINDS: [u32; 2] = [3, 4];
pub const RIDING: u32 = 1;
/// The run's flags (its `+8`): [`STILL`] set, the run's clock stands; [`NO_STUCK`] set, a stuck
/// bike is not counted as one.
pub const STILL: u32 = 0x200;
pub const NO_STUCK: u32 = 0x40_0000;
/// The frame message's category and message.
pub const FRAME_MESSAGE: (u32, u32) = (8, 4);
/// The rumble's length.
pub const RUMBLE_TIME: f32 = 0.2;

/// One wheel, as the update reads it: its ray hit (`+0xd0`), touching (`+0xdc`) and roughness
/// (`+0xf0`) words, and what its surface (`+0x110`'s byte) says in the bike's table at `+0xdf4`:
/// held (the entry's word) and for how long before a stuck bike is put back (its `+4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    pub ray_hit: bool,
    pub touching: bool,
    pub rough: F,
    pub surface_holds: bool,
    pub stuck_limit: F,
}

/// The bike's words the tail's update reads and writes. Pointers are kept as their words: the state
/// (`+0xbc4`), the run (`+0x680`) and the player (`+0x684`), and the fade's object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vehicle {
    pub state: u32,
    /// The state's kind, and whether it holds a rumbler (its `+0x5c`).
    pub kind: u32,
    pub rumbler: bool,
    /// `+0xc38` the signed speed, `+0x174c` under which the bike is slow, `+0x32c` the speed's length.
    pub speed: F,
    pub slow_below: F,
    pub speed_len: F,
    /// The node's side and ahead axes (its `+0x80`, `+0x90`); the body's position and velocity
    /// (`+0x70`, `+0x80`).
    pub side: Vf,
    pub ahead: Vf,
    pub body_position: Vf,
    pub body_velocity: Vf,
    pub wheels: [Wheel; 2],
    /// `+0xc20` a wheel on a holding surface; `+0x31c`/`+0x320` the put-back's due and its last.
    pub held: u32,
    pub put_back_due: u32,
    pub put_back_was: u32,
    /// `+0xc14` free, `+0xc10` stuck, `+0xdf0` how long; `+0xc00` where the bike was last free.
    pub free: u32,
    pub stuck: u32,
    pub stuck_time: F,
    pub last_free: Vf,
    /// `+0x324` on the ground, `+0xde0` airborne, `+0x1190` the hang time, `+0xc80` the wreck reason.
    pub on_ground: u32,
    pub airborne: u32,
    pub hang_time: F,
    pub wreck_reason: u32,
    /// `+0x1744`/`+0x1748` the wheelie's and stoppie's clocks; `+0x704`/`+0x708` their time past the
    /// first second, `+0x70c`/`+0x710` their distance, `+0x714`..`+0x720` the bests of the four; the
    /// tricks they start (`+0x13c4`, `+0x13c8`).
    pub wheelie_clock: F,
    pub stoppie_clock: F,
    pub wheelie: F,
    pub stoppie: F,
    pub wheelie_distance: F,
    pub stoppie_distance: F,
    pub best_wheelie: F,
    pub best_stoppie: F,
    pub best_wheelie_distance: F,
    pub best_stoppie_distance: F,
    pub wheelie_trick: u32,
    pub stoppie_trick: u32,
    /// The run and the player, 0 for none; the run's flags (its `+8`; with no run, the word at 8),
    /// how far it went this frame (`+0x18`), its top speed (`+0x108`), landed (`+0x40`), its end's
    /// clock (`+0xc4`); the player's own clock (`+0x29c`).
    pub run: u32,
    pub player: u32,
    pub run_flags: u32,
    pub run_step: F,
    pub run_top_speed: F,
    pub run_landed: u32,
    pub run_left: F,
    pub player_left: F,
    /// `+0x6f8` the run's time, `+0x6fc` its top speed, `+0x724` its distance, `+0x6c8` landings,
    /// `+0x6cc` landed last frame.
    pub run_time: F,
    pub top_speed: F,
    pub distance: F,
    pub landings: u32,
    pub landed: u32,
    /// `+0x175c` the lap clock on, `+0x1760` what is left of it, `+0x1750` the frame clock, `+0xd20`
    /// the message's id.
    pub lap: u32,
    pub lap_left: F,
    pub clock: F,
    pub id: u32,
    /// `+0x1770` the put-back countdown (frames), `+0x177c` the fade's time, the fade's object (the
    /// `+0x1778` group's `+0x30`), `+0x1650` cleared every frame.
    pub countdown: i32,
    pub fade: F,
    pub fade_object: u32,
    pub frame_word: u32,
    /// `+0xcf8` the ride's clock, `+0xd2c`/`+0xd30` two counting down, `+0xd34` one counting up.
    pub ride_clock: F,
    pub down: [F; 2],
    pub up: F,
    /// `+0x12a8`, `+0x12ac`, `+0x310`, `+0x12c0`: cleared while the bike is down.
    pub cleared: [u32; 4],
}

const PUT_BACK: [u32; 2] = [0x31c, 0x320];
const FREE: [u32; 4] = [0xc14, 0xc10, 0xdf0, 0xc00];
const TRICKS: [u32; 12] = [0x1744, 0x1748, 0x704, 0x708, 0x70c, 0x710, 0x714, 0x718, 0x71c, 0x720, 0x13c4, 0x13c8];
const RUN: [u32; 5] = [0x6f8, 0x6fc, 0x724, 0x6c8, 0x6cc];
const CLOCKS: [u32; 4] = [0x175c, 0x1760, 0x1750, 0xd20];
const FADE: [u32; 3] = [0x1770, 0x177c, 0x1650];
const RIDE: [u32; 4] = [0xcf8, 0xd2c, 0xd30, 0xd34];
const CLEARED: [u32; 4] = [0x12a8, 0x12ac, 0x310, 0x12c0];
const SURFACES: u32 = 0xdf4;
const SURFACE: u32 = 12;

/// The bike's words at `bike`, as the update reads them.
pub fn read(m: &impl Words, bike: u32) -> Vehicle {
    let f = |o: u32| m.f(bike + o);
    let w = |o: u32| m.word(bike + o);
    let state = w(STATE);
    let (body, node) = (w(super::seed::BODY), w(super::seed::NODE));
    let wheel = |at: u32| {
        let s = m.word((at + 0x110) & !3) >> (8 * ((at + 0x110) & 3)) & 0xff;
        let e = bike + SURFACES + SURFACE * s;
        Wheel {
            ray_hit: m.b(at + 0xd0),
            touching: m.b(at + 0xdc),
            rough: m.f(at + 0xf0),
            surface_holds: m.b(e),
            stuck_limit: m.f(e + 4),
        }
    };
    let (run, player) = (w(0x680), w(0x684));
    Vehicle {
        state,
        kind: m.word(state + KIND),
        rumbler: m.b(state + 0x5c),
        speed: f(0xc38),
        slow_below: f(0x174c),
        speed_len: f(0x32c),
        side: m.v(node + 0x80),
        ahead: m.v(node + 0x90),
        body_position: m.v(body + 0x70),
        body_velocity: m.v(body + 0x80),
        wheels: super::seed::WHEELS.map(|o| wheel(w(o))),
        held: w(0xc20),
        put_back_due: w(PUT_BACK[0]),
        put_back_was: w(PUT_BACK[1]),
        free: w(FREE[0]),
        stuck: w(FREE[1]),
        stuck_time: f(FREE[2]),
        last_free: m.v(bike + FREE[3]),
        on_ground: w(0x324),
        airborne: w(0xde0),
        hang_time: f(0x1190),
        wreck_reason: w(0xc80),
        wheelie_clock: f(TRICKS[0]),
        stoppie_clock: f(TRICKS[1]),
        wheelie: f(TRICKS[2]),
        stoppie: f(TRICKS[3]),
        wheelie_distance: f(TRICKS[4]),
        stoppie_distance: f(TRICKS[5]),
        best_wheelie: f(TRICKS[6]),
        best_stoppie: f(TRICKS[7]),
        best_wheelie_distance: f(TRICKS[8]),
        best_stoppie_distance: f(TRICKS[9]),
        wheelie_trick: w(TRICKS[10]),
        stoppie_trick: w(TRICKS[11]),
        run,
        player,
        run_flags: m.word(run + 8),
        run_step: m.f(run + 0x18),
        run_top_speed: m.f(run + 0x108),
        run_landed: m.word(run + 0x40),
        run_left: m.f(run + 0xc4),
        player_left: m.f(player + 0x29c),
        run_time: f(RUN[0]),
        top_speed: f(RUN[1]),
        distance: f(RUN[2]),
        landings: w(RUN[3]),
        landed: w(RUN[4]),
        lap: w(CLOCKS[0]),
        lap_left: f(CLOCKS[1]),
        clock: f(CLOCKS[2]),
        id: w(CLOCKS[3]),
        countdown: w(FADE[0]) as i32,
        fade: f(FADE[1]),
        fade_object: m.word(w(0x1778) + 0x30),
        frame_word: w(FADE[2]),
        ride_clock: f(RIDE[0]),
        down: [f(RIDE[1]), f(RIDE[2])],
        up: f(RIDE[3]),
        cleared: CLEARED.map(w),
    }
}

/// The words the update writes, at `bike`: its own, and the run's end clock (with a run).
pub fn words(v: &Vehicle, bike: u32) -> Vec<(u32, u32)> {
    let b = |x: F| x.to_bits();
    let mut out = vec![
        (bike + 0xc20, v.held),
        (bike + PUT_BACK[0], v.put_back_due),
        (bike + PUT_BACK[1], v.put_back_was),
        (bike + FREE[0], v.free),
        (bike + FREE[1], v.stuck),
        (bike + FREE[2], b(v.stuck_time)),
    ];
    out.extend(v.last_free.to_bits().into_iter().enumerate().map(|(k, w)| (bike + FREE[3] + 4 * k as u32, w)));
    let tricks = [
        v.wheelie_clock,
        v.stoppie_clock,
        v.wheelie,
        v.stoppie,
        v.wheelie_distance,
        v.stoppie_distance,
        v.best_wheelie,
        v.best_stoppie,
        v.best_wheelie_distance,
        v.best_stoppie_distance,
    ];
    out.extend(TRICKS.into_iter().zip(tricks).map(|(o, x)| (bike + o, b(x))));
    out.extend(RUN.into_iter().zip([b(v.run_time), b(v.top_speed), b(v.distance), v.landings, v.landed]).map(|(o, w)| (bike + o, w)));
    out.extend(CLOCKS.into_iter().zip([v.lap, b(v.lap_left), b(v.clock), v.id]).map(|(o, w)| (bike + o, w)));
    out.extend(FADE.into_iter().zip([v.countdown as u32, b(v.fade), v.frame_word]).map(|(o, w)| (bike + o, w)));
    out.extend(RIDE.into_iter().zip([v.ride_clock, v.down[0], v.down[1], v.up]).map(|(o, x)| (bike + o, b(x))));
    out.extend(CLEARED.into_iter().zip(v.cleared).map(|(o, w)| (bike + o, w)));
    if v.run != 0 {
        out.push((v.run + 0xc4, b(v.run_left)));
    }
    out
}

/// What the update hands out: the bike's virtuals and the outside world's calls, `dt` with each that
/// takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// The bike's fade step (its vt `+0x3a4`, dt).
    Fade,
    /// The bike's own listeners' walk ([`walk`] on the bike's listener, dt): while it is down.
    OwnListeners,
    /// The wreck's three steps while the bike is down: vt `+0x4f8`; `+0x4e8` (dt); `+0x4a0` (dt,
    /// the wreck reason as it was then).
    WreckFirst,
    WreckPose,
    WreckLast { reason: u32 },
    /// The wreck test (vt `+0x49c`, dt, 0) and the lean's step (`+0x4f0`, dt), while it is not.
    WreckTest,
    Lean,
    /// The fade over: the fade object's vt `+0x90`.
    FadeOver(u32),
    /// The base update's: vt `+0x3e4`; the ride (`+0x35c`, dt); vt `+0x378` and `+0x37c`; the
    /// network's step (on the bike's `+0x400`, dt); the vehicle's parts and listeners (dt).
    Prepare,
    Ride,
    Sync,
    Snapshot,
    Network,
    Parts,
    /// The put-back (vt `+0x3a8`, 1, 1).
    PutBack,
    /// A wheelie's or stoppie's trick started (vt `+0x4b8`, the trick, 1); the trick ended (vt
    /// `+0x4ac`, −1).
    TrickStart(u32),
    TrickEnd,
    /// The pad's rumble: the rumbler's vt `+0x4c` (soft, level, [`RUMBLE_TIME`]).
    Rumble { soft: bool, level: i32 },
    /// The run over: the run's own vt `+0x68` (the player, the run); each of the run's items' vt
    /// `+0x68` (the player, the run); the player's rider and each it leads, vt `+0x7c` (the player,
    /// the run).
    RunOver { player: u32, run: u32 },
    RunOverItem { item: u32, player: u32, run: u32 },
    RunOverRider { rider: u32, player: u32, run: u32 },
    /// The frame's message: the queue handed [`FRAME_MESSAGE`] with the bike's id.
    Message(u32),
}

/// The update's outside world. After each [`Vehicles::out`], [`Vehicles::vehicle`] answers the
/// bike's words as the call left them.
pub trait Vehicles {
    fn vehicle(&mut self) -> &mut Vehicle;
    /// The frame skipped (the world's skip byte's bit 3).
    fn skipped(&mut self) -> bool;
    /// The game paused: the state's lookup of the game's pause object, and its flag.
    fn paused(&mut self) -> bool;
    /// The pad can rumble (asked only with a level and a rumbler).
    fn rumble_ready(&mut self) -> bool;
    /// A word outside the bike: the run's item and rider lists' links.
    fn word(&mut self, at: u32) -> u32;
    fn out(&mut self, c: Out, dt: F);
}

/// The vehicle's base update. Its ride runs whatever the state's kind: the original reads the kind
/// and passes over the ride only for one that is both 3 and 4.
pub fn base(m: &mut impl Vehicles, dt: F) {
    if m.vehicle().state == 0 {
        m.out(Out::Parts, dt);
        return;
    }
    m.out(Out::Prepare, dt);
    let v = m.vehicle();
    v.ride_clock = v.ride_clock + dt;
    for d in &mut v.down {
        if !(*d <= F::ZERO) {
            *d = *d - dt;
        }
    }
    if !(v.up < F::ZERO) {
        v.up = v.up + dt;
    }
    if v.free != 0 {
        v.last_free = v.body_position;
    }
    m.out(Out::Ride, dt);
    for c in [Out::Sync, Out::Snapshot, Out::Network, Out::Parts] {
        m.out(c, dt);
    }
}

/// The bike's own update, under the tail's.
pub fn inner(m: &mut impl Vehicles, dt: F) {
    if m.vehicle().state == 0 {
        base(m, dt);
        return;
    }
    if !m.skipped() {
        m.out(Out::Fade, dt);
        let v = m.vehicle();
        if v.state != 0 && WRECK_KINDS.contains(&v.kind) {
            v.cleared = [0; 4];
            m.out(Out::OwnListeners, dt);
            m.out(Out::WreckFirst, dt);
            m.out(Out::WreckPose, dt);
            let reason = m.vehicle().wreck_reason;
            m.out(Out::WreckLast { reason }, dt);
        } else {
            m.out(Out::WreckTest, dt);
            m.out(Out::Lean, dt);
        }
    }
    let v = m.vehicle();
    if v.countdown > 0 {
        v.countdown -= 1;
        if v.countdown == 0 {
            v.put_back_due = 1;
        }
    }
    if !(v.fade <= F::ZERO) {
        v.fade = v.fade - dt;
        if v.fade <= F::ZERO {
            let o = v.fade_object;
            if o != 0 {
                m.out(Out::FadeOver(o), dt);
            }
            m.vehicle().fade = F::ZERO;
        }
    }
    base(m, dt);
    m.vehicle().frame_word = 0;
}

/// The stuck timer (the bike's vt `+0x45c`): with `on` (a wheel on a holding surface, a ray down)
/// and a `limit` (−1 at once, over 0 that long), a bike not free counts up and is put back past it;
/// otherwise it is free.
pub fn stuck(m: &mut impl Vehicles, on: bool, dt: F, limit: F) {
    let v = m.vehicle();
    if !on || (limit <= F::ZERO && limit != F(-1.0)) {
        (v.free, v.stuck, v.stuck_time) = (1, 0, F::ZERO);
        return;
    }
    v.free = 0;
    if v.run_flags & NO_STUCK == 0 {
        v.stuck = 1;
    }
    if limit == F(-1.0) {
        m.out(Out::PutBack, dt);
        m.vehicle().stuck_time = F::ZERO;
    } else if limit <= F::ZERO {
        v.stuck_time = F::ZERO;
    } else {
        v.stuck_time = v.stuck_time + dt;
        if !(v.stuck_time <= limit) {
            m.out(Out::PutBack, dt);
            m.vehicle().stuck_time = F::ZERO;
        }
    }
}

/// The rumble's level off the rougher wheel, and whether it is the strong one: over 0.95 it is
/// 255 × 0.45 × it and at least 95, over 0.73 255 × 0.5 × it and at least 40, else none.
pub fn rumble(rough: F) -> (i32, bool) {
    if !(rough <= F(0.95)) {
        (fpu::to_i32(fpu::mul(255.0, fpu::mul(0.45, rough.0))).max(95), true)
    } else if !(rough <= F(0.73)) {
        (fpu::to_i32(fpu::mul(255.0, fpu::mul(0.5, rough.0))).max(40), false)
    } else {
        (0, false)
    }
}

/// One of the wheelie's or stoppie's clocks run a frame: past a second, its time counts (the trick
/// starts as it does), and with a player and a run its distance does; each keeps its best.
fn trick(m: &mut impl Vehicles, dt: F, stoppie: bool) {
    let v = m.vehicle();
    let clock = if stoppie { &mut v.stoppie_clock } else { &mut v.wheelie_clock };
    *clock = *clock + dt;
    if *clock <= F::ONE {
        return;
    }
    let (time, id) = if stoppie { (v.stoppie, v.stoppie_trick) } else { (v.wheelie, v.wheelie_trick) };
    if time <= F::ZERO {
        m.out(Out::TrickStart(id), dt);
    }
    let v = m.vehicle();
    let (time, best) = if stoppie { (&mut v.stoppie, &mut v.best_stoppie) } else { (&mut v.wheelie, &mut v.best_wheelie) };
    *time = *time + dt;
    if !(*time <= *best) {
        *best = *time;
    }
    if v.player != 0 && v.run != 0 {
        let step = v.run_step;
        let (d, best) = if stoppie {
            (&mut v.stoppie_distance, &mut v.best_stoppie_distance)
        } else {
            (&mut v.wheelie_distance, &mut v.best_wheelie_distance)
        };
        *d = *d + step;
        if !(*d <= *best) {
            *best = *d;
        }
    }
}

/// The run over: the run, its items and the player's riders told, each with the player and the
/// run as they are when it is called.
fn run_over(m: &mut impl Vehicles, dt: F, riders: u32) {
    fn both(m: &mut impl Vehicles) -> (u32, u32) {
        let v = m.vehicle();
        (v.player, v.run)
    }
    let (player, run) = both(m);
    m.out(Out::RunOver { player, run }, dt);
    let run = m.vehicle().run;
    let mut item = m.word(run + 0x1d4);
    while item != 0 {
        let (player, run) = both(m);
        m.out(Out::RunOverItem { item, player, run }, dt);
        item = m.word(item + 0x1c);
    }
    let mut rider = riders;
    let mut next = 4;
    while rider != 0 {
        let (player, run) = both(m);
        m.out(Out::RunOverRider { rider, player, run }, dt);
        rider = m.word(rider + next);
        next = 0x18;
    }
}

/// **The bike's vehicle update**: the tail's step of the bike's listener.
pub fn vehicle_update(m: &mut impl Vehicles, dt: F) {
    if m.vehicle().state == 0 {
        inner(m, dt);
        return;
    }
    if m.paused() {
        return;
    }
    let v = m.vehicle();
    let slow = v.speed < v.slow_below;
    inner(m, dt);

    let v = m.vehicle();
    let [front, rear] = v.wheels;
    v.held = u32::from(front.surface_holds || rear.surface_holds);
    let on = v.held != 0 && (front.ray_hit || rear.ray_hit);
    let limit = front.stuck_limit.max(rear.stuck_limit);
    stuck(m, on, dt, limit);

    let v = m.vehicle();
    if v.put_back_due != 0 && v.put_back_was != 0 {
        v.put_back_due = 0;
    }
    v.put_back_was = v.put_back_due;
    if !(v.speed_len <= F(10.0)) && v.state != 0 && v.kind == RIDING && v.wreck_reason == 0 {
        let (level, strong) = rumble(v.wheels[0].rough.max(v.wheels[1].rough));
        if level > 0 && v.rumbler && m.rumble_ready() {
            m.out(Out::Rumble { soft: !strong, level }, dt);
        }
    }

    let v = m.vehicle();
    if v.wreck_reason != 0 {
        (v.wheelie_clock, v.stoppie_clock) = (F::ZERO, F::ZERO);
    } else {
        let (fd, rd) = (v.wheels[0].touching, v.wheels[1].touching);
        if !fd && rd && !slow {
            trick(m, dt, false);
        } else if fd && !rd && !slow {
            trick(m, dt, true);
        } else if v.on_ground != 0 || (v.airborne == 0 && slow) || (v.airborne != 0 && !(v.hang_time <= F(0.1))) {
            if !(v.wheelie <= F::ZERO) || !(v.stoppie <= F::ZERO) {
                m.out(Out::TrickEnd, dt);
            }
            let v = m.vehicle();
            (v.wheelie_clock, v.stoppie_clock, v.wheelie, v.stoppie, v.wheelie_distance, v.stoppie_distance) =
                (F::ZERO, F::ZERO, F::ZERO, F::ZERO, F::ZERO, F::ZERO);
        }
    }

    let v = m.vehicle();
    if v.run != 0 {
        if v.run_flags & STILL == 0 {
            v.run_time = v.run_time + dt;
        }
        if !(v.run_top_speed <= v.top_speed) {
            v.top_speed = v.run_top_speed;
        }
        let along = if v.wheels[0].touching {
            v.speed
        } else {
            let y = F(v.ahead.y());
            let mut heading = if y <= F(-1.0) || !(y < F::ONE) { v.side } else { v.ahead };
            heading.0[1] = 0.0;
            let n = normalize(heading);
            let b = v.body_velocity;
            F(fpu::madd(fpu::adda(0.0, fpu::mul(b.z(), n.z())), b.x(), n.x()))
        };
        let along = if along < F::ZERO { F::ZERO } else { along };
        v.distance = F(fpu::madd(fpu::adda(0.0, v.distance.0), along.0, dt.0));
        if v.landed != 0 && v.run_landed == 0 {
            v.landings = v.landings.wrapping_add(1);
        }
        v.landed = v.run_landed;
        if !(v.run_left <= F::ZERO) && v.player != 0 && v.player_left <= F::ZERO {
            let player = v.player;
            let riders = m.word(player + 0x24);
            let v = m.vehicle();
            v.run_left = v.run_left - dt;
            if v.run_left <= F::ZERO {
                run_over(m, dt, riders);
            }
        }
    }

    let v = m.vehicle();
    if v.lap != 0 && v.state != 0 && v.kind == RIDING {
        if v.lap_left < F::ZERO {
            v.lap = 0;
        } else {
            v.lap_left = v.lap_left - dt;
        }
    }
    v.clock = v.clock + dt;
    let id = v.id;
    m.out(Out::Message(id), dt);
}
