//! The ride state machine against the original.
//!
//! Five proofs:
//!
//! 1. **the machine**, ten thousand generated frames through the original's own per-frame function:
//!    every field it writes, the reset it asks for and the way it asks, bit for bit.
//! 2. **the machine again**, exhaustively: every combination of what it is told, and then every
//!    combination of the clock against each limit — what a generated set reaches by luck, this
//!    reaches on purpose.
//! 3. **the wreck entry**, both halves: the base vehicle's three state writes for every reason code,
//!    and the player's two on top of them.
//! 4. **the put-back**, through the original's respawn and the unwreck under it: what it clears, what
//!    it sets, what it hands on, and the two fields it leaves alone.
//! 5. **the clocks in frames**, port-side only: how many frames each window takes at a steady frame
//!    time, and the state a bike is left in. Not a proof of the original — a check that the numbers
//!    in the module's docs are the numbers the code produces.
//!
//! Two of the original's own calls stand in for the real thing in proofs 1 and 2 — the wreck test
//! (S10's; the gates in front of it are the only part this task owns) and the wreck entry and respawn
//! the machine reaches through the vehicle's vtable, which proofs 3 and 4 show write what the stand-ins
//! write. The two page questions are *not* hooked: they are field reads, so the test lays out the
//! fields instead and the original's own composition of them is what runs. What is not proved at all,
//! and why, is in the journal entry's last section.

use game_sim::state::{
    Allowed, Kind, Owner, Params, Phase, Reset, Ride, SETTLE, SETTLE_CANNONBALL, Sense, Step,
};
use game_sim::trick::Wreck;
use ps2_float::F;
use ps2_oracle::{DONE, Ee, addr, game_or_skip, skip};
use std::cell::Cell;
use std::rc::Rc;

/// Instruction budget per call. A frame of the machine is a few hundred instructions, and the reset
/// request it runs for real is a few hundred more.
const BUDGET: u64 = 200_000;

/// Where this test puts things in the oracle's RAM: well above the program image.
const OBJ: u32 = 0x0100_0000;
const VT: u32 = 0x0101_0000;
const OWNER: u32 = 0x0102_0000;
const SCREEN_OBJ: u32 = 0x0103_0000;
const SCREEN_VT: u32 = 0x0104_0000;
const ROOT: u32 = 0x0105_0000;
const SCREENS: u32 = 0x0106_0000;
const PAGES: u32 = 0x0107_0000;
const GROUP_A: u32 = 0x0107_4000;
const GROUP_B: u32 = 0x0107_8000;
const RECORD_OBJ: u32 = 0x0108_0000;
const CAMERA_OBJ: u32 = 0x0109_0000;
const ROUTE_OBJ: u32 = 0x010a_0000;
const ANIM_OBJ: u32 = 0x010b_0000;
const ANIM_VT: u32 = 0x010c_0000;
const STATE_OBJ: u32 = 0x010d_0000;
const PRIMARY: u32 = 0x010e_0000;

/// Addresses that are not code: each one is hooked, so a vtable can be a table of them.
const HOOK_ENTRY: u32 = 0x0070_0000;
const HOOK_RESPAWN: u32 = 0x0070_0010;
const HOOK_CLOSE_PAGE: u32 = 0x0070_0020;
const HOOK_HAND_HOLD: u32 = 0x0070_0030;
const HOOK_ANIM_STOP: u32 = 0x0070_0040;
const HOOK_PLACE: u32 = 0x0070_0050;
const HOOK_ANIM_RESET: u32 = 0x0070_0060;
const HOOK_SOUND: u32 = 0x0070_0070;
const HOOK_LOADS: u32 = 0x0070_0080;

/// The vehicle's fields, by the offsets the original uses. Named here because a test may say where
/// things live; the port may not.
const OWNER_PTR: u32 = 0x20;
const ORIENTATION: u32 = 0x18;
const VTABLE: u32 = 0x48;
const REASON: u32 = 0x748;
const LATCH: u32 = 0x74c;
const FRESH: u32 = 0x750;
const CLOCK: u32 = 0x794;
const MAX_WRECK: u32 = 0x798;
const CANNONBALL_WRECK: u32 = 0x79c;
const STATE: u32 = 0x7a4;
const ANIM: u32 = 0x7a8;
const DAMPING: u32 = 0x6a8;
const TRICK_RUNNING: u32 = 0xa0c;
const TRICK_QUEUE: u32 = 0xa38;
const ACTION: u32 = 0xa78;
const REQUESTED: u32 = 0xa7c;
const RESTART: u32 = 0xaf8;
const SETTLES: u32 = 0xd3c;
const RECORD: u32 = 0xf9c;
const ROUTE: u32 = 0x10d0;
const WRECKS: u32 = 0x12d8;
const ENTRIES: u32 = 0x12f8;
const CAMERA: u32 = 0x1498;
const SHOWN: u32 = 0x14b4;
const KIND: u32 = 0x14d4;
/// Inside the things the vehicle points at.
const CONTROL: u32 = 0x34;
const SCREEN: u32 = 0x50;
const PAD: u32 = 0x68;
const ROUTE_FLAG: u32 = 0xd4;
const FLAGS: u32 = 8;
const RIDING_BIT: u64 = 0x8_0000;
const ANIM_RIG: u32 = 0x110;
const ANIM_TARGET: u32 = 0x120;
const ANIM_VT_PTR: u32 = 0x38;
/// Inside the application object.
const APP_SCREENS: u32 = 0x3cc;
const APP_PAGES: u32 = 0x3d0;
const APP_PRIMARY: u32 = 0x64;
const APP_PAGE_HANDLE: u32 = 0xd4;
const APP_SPLIT: u32 = 0x74;
const APP_PLACER: u32 = 0xc0;
/// Inside one screen's page group: the two fields the machine asks about before raising a page.
const GROUP_SLOTS: u32 = 0x40;
const GROUP_Q1: u32 = 0x54;
const GROUP_Q2: u32 = 0x48;
/// The vtable slots the machine and the put-back reach through.
const SLOT_WRECK: u32 = 0xfc;
const SLOT_RESPAWN: u32 = 0x248;
const SLOT_LOADS: u32 = 0x104;
const SLOT_ANIM_STOP: u32 = 0x114;
const SLOT_HAND_HOLD: u32 = 0x118;
const SLOT_PLACE: u32 = 0x138;
const SLOT_CLOSE_PAGE: u32 = 0xa0;
const SLOT_SOUND: u32 = 0x80;
const SLOT_ANIM_RESET: u32 = 0x120;
/// What the unwreck puts in the two action fields: the grounded idle pose.
const GROUNDED_IDLE: u32 = 0x1c;

/// Resolve the names this test needs, or skip with the first one that is missing.
fn named(names: &[&str]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => out.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    Some(out)
}

/// A tiny xorshift, so every generated input is the same on every machine and a failure is
/// reproducible from the seed in this file alone.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A float in `-span..=span`, with the exponent spread so small and large magnitudes both appear.
    fn float(&mut self, span: f32) -> f32 {
        let r = self.next();
        let unit = (r >> 8) as f32 / (1u32 << 24) as f32;
        let sign = if r & 1 == 0 { 1.0 } else { -1.0 };
        sign * span * unit
    }
    fn upto(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn flag(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

/// The reason codes the wreck test can answer with, plus the machine's own two.
const REASONS: [u8; 11] = [1, 2, 3, 4, 5, 7, 8, 0xb, 0xc, 13, 14];

/// The names proofs 1 and 2 need.
const MACHINE_NAMES: [&str; 8] = [
    "ride_state_machine",
    "bike_wreck_test",
    "jump_record_end",
    "wreck_page_raise",
    "wreck_page_show",
    "app_pointer",
    "wreck_enabled_flag",
    "network_session",
];

// ---------------------------------------------------------------------------------------------------
// one frame, both sides
// ---------------------------------------------------------------------------------------------------

/// Everything one frame of the machine depends on: the state it starts from, what it is told, and
/// what the things it calls out to answer.
#[derive(Clone, Copy, Debug)]
struct Case {
    dt: f32,
    /// `None`: no AI route object at all. `Some(false)`: one that is not following anything.
    route: Option<bool>,
    /// `None`: no owning player record. `Some((human, screen))`: one, driven from a local pad or
    /// not, with a screen of its own or not.
    owner: Option<(bool, bool)>,
    /// The owner's screen is the primary one, which is what decides *which* screen's pages the
    /// machine asks about.
    primary: bool,
    enabled: bool,
    settles: bool,
    reason: u8,
    latch: bool,
    clock: f32,
    kind: u32,
    shown: bool,
    wrecks: i32,
    entries: i32,
    fresh: bool,
    flags: u64,
    max: f32,
    cannonball: f32,
    /// What the attitude and landing clauses of the wreck test decide, 0 for nothing. Only reached
    /// when the machine's gate lets them run.
    verdict: u8,
    /// The clause in front of that gate fires: thrown into something. It can wreck a rider who is
    /// already down, which is why the gate is not simply "not wrecked yet".
    collision: bool,
    /// What the front end has to say when asked whether it already has a page up.
    q1: bool,
    q2: bool,
}

impl Case {
    /// A frame with nothing happening in it, to vary one thing at a time from.
    fn blank() -> Case {
        Case {
            dt: 1.0 / 60.0,
            route: Some(true),
            owner: Some((true, true)),
            primary: true,
            enabled: true,
            settles: false,
            reason: 0,
            latch: false,
            clock: 0.0,
            kind: 0,
            shown: false,
            wrecks: 0,
            entries: 0,
            fresh: false,
            flags: RIDING_BIT | 0x21,
            max: 3.0,
            cannonball: 5.0,
            verdict: 0,
            collision: false,
            q1: false,
            q2: false,
        }
    }
}

/// What a frame left behind: every field the machine writes, and how it asked for the rider back.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Out {
    reason: u8,
    latch: bool,
    clock: u32,
    kind: u32,
    shown: bool,
    wrecks: i32,
    entries: i32,
    fresh: bool,
    flags: u64,
    reset: Option<Reset>,
}

/// What the stand-ins saw, which is how the reset is read off the emulator side.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Seen {
    entered: Option<u8>,
    respawned: bool,
    page_raised: bool,
    closed: bool,
}

/// The machine's half of the wreck entry, as proof 3 shows the original's two entries write it.
fn entry_writes(ee: &mut Ee, reason: u32) {
    ee.set_u32(OBJ + REASON, reason);
    ee.set_f32(OBJ + CLOCK, 0.0);
    ee.set_u32(OBJ + LATCH, 0);
    ee.set_u32(OBJ + SHOWN, u32::from(reason != 0));
    ee.set_u32(OBJ + ENTRIES, ee.u32_at(OBJ + ENTRIES).wrapping_add(1));
}

/// The machine's half of the put-back, as proof 4 shows the respawn and the unwreck under it write
/// it. The clock and the latch are deliberately untouched.
fn respawn_writes(ee: &mut Ee) {
    ee.set_u32(OBJ + REASON, 0);
    ee.set_u32(OBJ + FRESH, 1);
    ee.set_u32(OBJ + SHOWN, 0);
    let flags = ee.read(RECORD_OBJ + FLAGS, 8) as u64 | RIDING_BIT;
    ee.write(RECORD_OBJ + FLAGS, 8, u128::from(flags));
}

/// Install everything proofs 1 and 2 replace. The case is read out of the cell the caller refills.
fn arrange(ee: &mut Ee, at: &[u32], case: &Rc<Cell<Case>>, seen: &Rc<Cell<Seen>>) {
    let (test, request_end, raise, show) = (at[1], at[2], at[3], at[4]);

    // The wreck test is S10's and is replaced here. It decides from the object's own fields, exactly
    // the way the original's two gates do, so a port that got its half of those gates wrong disagrees
    // with it.
    let (c, s) = (Rc::clone(case), Rc::clone(seen));
    ee.hooks.insert(
        test,
        Box::new(move |ee| {
            let case = c.get();
            let down = ee.u32_at(OBJ + REASON) != 0;
            let reason = if !case.enabled {
                0
            } else if case.collision && ee.u32_at(OBJ + REASON) != 13 {
                13
            } else if down {
                0
            } else {
                u32::from(case.verdict)
            };
            if reason != 0 {
                entry_writes(ee, reason);
                let mut was = s.get();
                was.entered = Some(reason as u8);
                s.set(was);
            }
        }),
    );
    // The jump record the reset request ends before anything else; nothing in this task reads it.
    ee.hooks.insert(request_end, Box::new(|_| {}));
    // The two calls that raise the wreck page.
    for hook in [raise, show] {
        let s = Rc::clone(seen);
        ee.hooks.insert(
            hook,
            Box::new(move |_| {
                let mut was = s.get();
                was.page_raised = true;
                s.set(was);
            }),
        );
    }
    // The two vtable slots the machine goes through, proved separately.
    let s = Rc::clone(seen);
    ee.hooks.insert(
        HOOK_ENTRY,
        Box::new(move |ee| {
            let reason = ee.r(5) as u32;
            entry_writes(ee, reason);
            let mut was = s.get();
            was.entered = Some(reason as u8);
            s.set(was);
        }),
    );
    let s = Rc::clone(seen);
    ee.hooks.insert(
        HOOK_RESPAWN,
        Box::new(move |ee| {
            respawn_writes(ee);
            let mut was = s.get();
            was.respawned = true;
            s.set(was);
        }),
    );
    let s = Rc::clone(seen);
    ee.hooks.insert(
        HOOK_CLOSE_PAGE,
        Box::new(move |_| {
            let mut was = s.get();
            was.closed = true;
            s.set(was);
        }),
    );
}

/// Write one case into the oracle's RAM.
fn place(ee: &mut Ee, at: &[u32], c: &Case) {
    let (app, enabled_flag, network) = (at[5], at[6], at[7]);
    ee.load(OBJ, &vec![0u8; 0x1500]);
    ee.load(VT, &vec![0u8; 0x300]);
    for obj in [
        OWNER,
        SCREEN_OBJ,
        SCREEN_VT,
        ROOT,
        SCREENS,
        PAGES,
        GROUP_A,
        GROUP_B,
        RECORD_OBJ,
        CAMERA_OBJ,
        ROUTE_OBJ,
        PRIMARY,
    ] {
        ee.load(obj, &vec![0u8; 0x400]);
    }
    ee.set_u32(VT + SLOT_WRECK, HOOK_ENTRY);
    ee.set_u32(VT + SLOT_RESPAWN, HOOK_RESPAWN);
    ee.set_u32(SCREEN_VT + SLOT_CLOSE_PAGE, HOOK_CLOSE_PAGE);
    ee.set_u32(SCREEN_OBJ + FLAGS, SCREEN_VT);
    ee.set_u32(OBJ + VTABLE, VT);
    ee.set_u32(OBJ + RECORD, RECORD_OBJ);
    ee.set_u32(OBJ + CAMERA, CAMERA_OBJ);
    ee.write(RECORD_OBJ + FLAGS, 8, u128::from(c.flags));
    // The application object the machine reaches the screens and the pages through.
    ee.set_u32(app, ROOT);
    ee.set_u32(ROOT + APP_SCREENS, SCREENS);
    ee.set_u32(ROOT + APP_PAGES, SCREENS);
    ee.set_u32(SCREENS + APP_PAGE_HANDLE, PAGES);
    ee.set_u32(SCREENS + APP_PRIMARY, if c.primary { SCREEN_OBJ } else { PRIMARY });
    ee.set_u32(SCREENS + APP_SPLIT, 0);
    // The page group the machine asks about is the one for *this* rider's screen, which is the second
    // slot unless the rider owns the primary screen. The answers go only in that one: a machine that
    // asked about the other screen would come out differently.
    let (mine, theirs) = if c.primary { (GROUP_A, GROUP_B) } else { (GROUP_B, GROUP_A) };
    ee.set_u32(PAGES + GROUP_SLOTS, GROUP_A);
    ee.set_u32(PAGES + GROUP_SLOTS + 4, GROUP_B);
    ee.set_u32(mine + GROUP_Q1, u32::from(c.q1));
    ee.set_u32(mine + GROUP_Q2, u32::from(c.q2));
    ee.set_u32(theirs + GROUP_Q1, 0);
    ee.set_u32(theirs + GROUP_Q2, 0);
    ee.set_u32(network, 0);
    ee.set_u32(enabled_flag, u32::from(c.enabled));
    // What the frame is told.
    match c.route {
        None => ee.set_u32(OBJ + ROUTE, 0),
        Some(following) => {
            ee.set_u32(OBJ + ROUTE, ROUTE_OBJ);
            ee.set_u32(ROUTE_OBJ + ROUTE_FLAG, u32::from(following));
        }
    }
    match c.owner {
        None => ee.set_u32(OBJ + OWNER_PTR, 0),
        Some((human, screen)) => {
            ee.set_u32(OBJ + OWNER_PTR, OWNER);
            ee.set_u32(OWNER + CONTROL, if human { 1 } else { 2 });
            ee.set_u32(OWNER + SCREEN, if screen { SCREEN_OBJ } else { 0 });
            ee.set_u32(OWNER + PAD, 0);
        }
    }
    ee.set_u32(OBJ + SETTLES, u32::from(c.settles));
    ee.set_u32(OBJ + REASON, u32::from(c.reason));
    ee.set_u32(OBJ + LATCH, u32::from(c.latch));
    ee.set_u32(OBJ + FRESH, u32::from(c.fresh));
    ee.set_f32(OBJ + CLOCK, c.clock);
    ee.set_f32(OBJ + MAX_WRECK, c.max);
    ee.set_f32(OBJ + CANNONBALL_WRECK, c.cannonball);
    ee.set_u32(OBJ + KIND, c.kind);
    ee.set_u32(OBJ + SHOWN, u32::from(c.shown));
    ee.set_u32(OBJ + WRECKS, c.wrecks as u32);
    ee.set_u32(OBJ + ENTRIES, c.entries as u32);
}

/// Read the frame's result off the emulator.
fn harvest(ee: &Ee, seen: Seen) -> Out {
    Out {
        reason: ee.u32_at(OBJ + REASON) as u8,
        latch: ee.u32_at(OBJ + LATCH) != 0,
        clock: ee.f32_at(OBJ + CLOCK).to_bits(),
        kind: ee.u32_at(OBJ + KIND),
        shown: ee.u32_at(OBJ + SHOWN) != 0,
        wrecks: ee.i32_at(OBJ + WRECKS),
        entries: ee.i32_at(OBJ + ENTRIES),
        fresh: ee.u32_at(OBJ + FRESH) != 0,
        flags: ee.read(RECORD_OBJ + FLAGS, 8) as u64,
        reset: match (seen.respawned, seen.page_raised) {
            (true, _) => Some(Reset::Now),
            (false, true) => Some(Reset::Page),
            (false, false) => None,
        },
    }
}

/// The same frame through the port.
fn port(c: &Case) -> (Out, Allowed, Step) {
    let mut ride = Ride {
        wreck: (c.reason != 0).then_some(Wreck(c.reason)),
        counting: c.latch,
        elapsed: F(c.clock),
        kind: if c.kind == 1 { Kind::Wrecked } else { Kind::Plain },
        pending: false,
        fresh: c.fresh,
        wrecking: c.shown,
        wrecks: c.wrecks,
        entries: c.entries,
    };
    let sense = Sense {
        ai_route: c.route == Some(true),
        owner: c.owner.map(|(human, screen)| Owner { human, screen }),
        wrecks_enabled: c.enabled,
        air_wrecks: true,
        settles: c.settles,
    };
    let params = Params { max_wreck_time: F(c.max), cannonball_wreck_time: F(c.cannonball) };
    // The wreck test, told only what the machine hands it — so the gates in front of it are the
    // port's answer and not this test's.
    let was = ride.wreck;
    let gates = Rc::new(Cell::new(Allowed { wrecks: false, landing: false, air: false }));
    let told = Rc::clone(&gates);
    let step = ride.step(
        F(c.dt),
        sense,
        params,
        |allowed| {
            told.set(allowed);
            if !allowed.wrecks {
                None
            } else if c.collision && was != Some(Wreck::CANNONBALL) {
                Some(Wreck::CANNONBALL)
            } else if allowed.landing {
                (c.verdict != 0).then_some(Wreck(c.verdict))
            } else {
                None
            }
        },
        || c.q1 || c.q2,
    );
    let mut flags = c.flags;
    if let Some(riding) = step.riding_bit {
        flags = if riding { flags | RIDING_BIT } else { flags & !RIDING_BIT };
    }
    let out = Out {
        reason: ride.wreck.map_or(0, |w| w.0),
        latch: ride.counting,
        clock: ride.elapsed.to_bits(),
        kind: ride.kind as u32,
        shown: ride.wrecking,
        wrecks: ride.wrecks,
        entries: ride.entries,
        fresh: ride.fresh,
        flags,
        reset: step.reset,
    };
    (out, gates.get(), step)
}

/// One case through both sides, with everything a failure needs printed.
fn check(ee: &mut Ee, machine: u32, c: &Case, seen: &Rc<Cell<Seen>>, label: &str) -> Seen {
    seen.set(Seen::default());
    ee.call(machine, &[u64::from(OBJ), 0], &[c.dt], BUDGET);
    let saw = seen.get();
    let want = harvest(ee, saw);
    let (got, gates, step) = port(c);
    assert_eq!(
        got, want,
        "{label}: {c:?}\n(reason, latch, clock, kind, shown, wrecks, entries, fresh, flags, reset)"
    );
    // The two gates the machine hands the wreck test. They are only decided on the driven path; the
    // other one does not run the test at all.
    if c.route == Some(true) || c.owner.is_some_and(|(human, _)| human) {
        assert_eq!(gates.wrecks, c.enabled, "{label}: whether the test may fire at all");
        assert_eq!(gates.landing, c.reason == 0, "{label}: whether the landing clauses may fire");
    }
    assert_eq!(step.wreck.map(|w| w.0), saw.entered, "{label}: the entry the frame ran");
    // The page is closed before the rider is put back, and only for a reset that had a screen to
    // show it on in the first place.
    assert_eq!(
        saw.closed,
        saw.respawned && c.owner.is_some_and(|(_, screen)| screen),
        "{label}: whether the page was closed first"
    );
    saw
}

// ---------------------------------------------------------------------------------------------------
// 1 — the machine over ten thousand frames
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_machine_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&MACHINE_NAMES) else { return };
    let machine = at[0];

    let case = Rc::new(Cell::new(Case::blank()));
    let seen = Rc::new(Cell::new(Seen::default()));
    arrange(&mut ee, &at, &case, &seen);

    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    let (mut entries, mut resets, mut pages, mut latched, mut rewrecks) = (0, 0, 0, 0, 0);
    for k in 0..10_000 {
        let max = match pick.upto(4) {
            0 => 0.0,
            1 => 1.0 / 60.0,
            _ => pick.float(4.0).abs(),
        };
        let cannonball = if pick.flag() { max } else { pick.float(4.0).abs() };
        let reason = match pick.upto(3) {
            0 => 0,
            1 => 13,
            _ => REASONS[pick.upto(REASONS.len() as u32) as usize],
        };
        let limit = if reason == 13 { cannonball } else { max };
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        // The clock is aimed at the boundaries it is compared against as well as spread, because a
        // uniform draw would almost never land on one.
        let clock = match pick.upto(8) {
            0 => 0.0,
            1 => limit,
            2 => limit - dt,
            3 => SETTLE.to_f32(),
            4 => SETTLE.to_f32() - dt,
            5 => SETTLE_CANNONBALL.to_f32(),
            6 => SETTLE_CANNONBALL.to_f32() - dt,
            _ => pick.float(6.0).abs(),
        };
        let c = Case {
            dt,
            route: match pick.upto(3) {
                0 => None,
                1 => Some(false),
                _ => Some(true),
            },
            owner: match pick.upto(4) {
                0 => None,
                1 => Some((false, pick.flag())),
                _ => Some((true, pick.upto(4) != 0)),
            },
            primary: pick.flag(),
            enabled: pick.upto(8) != 0,
            settles: pick.upto(3) == 0,
            reason,
            latch: pick.flag(),
            clock,
            kind: pick.upto(2),
            shown: pick.flag(),
            wrecks: pick.upto(50) as i32,
            entries: pick.upto(50) as i32,
            fresh: pick.flag(),
            flags: if pick.flag() { RIDING_BIT | 0x21 } else { 0x21 },
            max,
            cannonball,
            verdict: if pick.upto(3) == 0 {
                REASONS[pick.upto(REASONS.len() as u32) as usize]
            } else {
                0
            },
            collision: pick.upto(6) == 0,
            q1: pick.upto(4) == 0,
            q2: pick.upto(4) == 0,
        };
        case.set(c);
        place(&mut ee, &at, &c);
        let saw = check(&mut ee, machine, &c, &seen, &format!("case {k}"));
        if saw.entered.is_some() {
            entries += 1;
            if c.reason != 0 {
                rewrecks += 1;
            }
        }
        if saw.respawned {
            resets += 1;
        }
        if saw.page_raised {
            pages += 1;
        }
        if !c.latch && ee.u32_at(OBJ + LATCH) != 0 {
            latched += 1;
        }
    }
    // A generated set is only a proof of what it reaches, so the branches that are hard to land on
    // say so out loud: a later change to the generator cannot quietly stop covering them.
    assert!(entries > 500, "only {entries} frames entered a wreck");
    assert!(rewrecks > 50, "only {rewrecks} frames wrecked a rider who was already down");
    assert!(resets > 200, "only {resets} frames put the rider back on the spot");
    assert!(pages > 100, "only {pages} frames raised the wreck page");
    assert!(latched > 50, "only {latched} frames armed the reset clock");
    println!(
        "the machine: 10,000 frames, bit-exact ({entries} entries, {rewrecks} re-wrecks, \
         {resets} put back, {pages} pages, {latched} armed)"
    );
}

// ---------------------------------------------------------------------------------------------------
// 2 — the machine over every combination
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_machine_matches_on_every_combination() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&MACHINE_NAMES) else { return };
    let machine = at[0];

    let case = Rc::new(Cell::new(Case::blank()));
    let seen = Rc::new(Cell::new(Seen::default()));
    arrange(&mut ee, &at, &case, &seen);

    let dt = 1.0 / 60.0;
    let mut cases = 0;
    // Everything the machine is told, against each reason it can be in and each verdict it can be
    // handed, with the clock already past the limit so that every one of them reaches the reset.
    for route in [None, Some(false), Some(true)] {
        for owner in
            [None, Some((false, false)), Some((false, true)), Some((true, false)), Some((true, true))]
        {
            for enabled in [false, true] {
                for primary in [false, true] {
                    for reason in [0u8, 4, 13, 14] {
                        for verdict in [0u8, 7, 13] {
                            for collision in [false, true] {
                                let c = Case {
                                    dt,
                                    route,
                                    owner,
                                    enabled,
                                    primary,
                                    reason,
                                    verdict,
                                    collision,
                                    latch: true,
                                    clock: 2.0,
                                    max: 1.0,
                                    cannonball: 1.0,
                                    ..Case::blank()
                                };
                                case.set(c);
                                place(&mut ee, &at, &c);
                                check(&mut ee, machine, &c, &seen, &format!("told {cases}"));
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    // The clock against every limit it is compared against, either side and exactly on it, with and
    // without the settle window, with and without the latch, and with the front end both free and
    // busy. The two limits are different numbers here so that a reason read from the wrong one shows
    // up.
    let mut clocks = 0;
    for settles in [false, true] {
        for latch in [false, true] {
            for reason in [4u8, 13, 14] {
                for (q1, q2) in [(false, false), (true, false), (false, true), (true, true)] {
                    for clock in [
                        0.0,
                        0.5 - dt,
                        0.5,
                        0.5 + dt,
                        1.0 - dt,
                        1.0,
                        1.0 + dt,
                        2.5 - dt,
                        2.5,
                        2.5 + dt,
                        3.0 - dt,
                        3.0,
                        3.0 + dt,
                    ] {
                        let c = Case {
                            dt,
                            settles,
                            latch,
                            reason,
                            clock,
                            q1,
                            q2,
                            max: 1.0,
                            cannonball: 3.0,
                            ..Case::blank()
                        };
                        case.set(c);
                        place(&mut ee, &at, &c);
                        check(&mut ee, machine, &c, &seen, &format!("clock {clocks}"));
                        clocks += 1;
                    }
                }
            }
        }
    }
    println!("the machine: {cases} tellings and {clocks} clocks, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 3 — the wreck entry, both halves
// ---------------------------------------------------------------------------------------------------

/// Lay out what both halves of the entry walk over.
fn place_for_entry(ee: &mut Ee, running: bool) {
    ee.load(OBJ, &vec![0u8; 0x1500]);
    ee.load(VT, &vec![0u8; 0x300]);
    for obj in [OWNER, SCREEN_OBJ, ROOT, SCREENS, RECORD_OBJ, CAMERA_OBJ, ANIM_OBJ, STATE_OBJ] {
        ee.load(obj, &vec![0u8; 0x400]);
    }
    ee.set_u32(VT + SLOT_HAND_HOLD, HOOK_HAND_HOLD);
    ee.set_u32(VT + SLOT_ANIM_STOP, HOOK_ANIM_STOP);
    ee.set_u32(VT + SLOT_LOADS, HOOK_LOADS);
    ee.set_u32(OBJ + VTABLE, VT);
    ee.set_u32(OBJ + ORIENTATION, STATE_OBJ);
    ee.set_u32(OBJ + ANIM, ANIM_OBJ);
    ee.set_u32(OBJ + STATE, STATE_OBJ);
    ee.set_u32(ANIM_OBJ + ANIM_RIG, STATE_OBJ);
    ee.set_u32(ANIM_OBJ + ANIM_TARGET, STATE_OBJ);
    ee.set_u32(OBJ + CAMERA, CAMERA_OBJ);
    ee.set_u32(OBJ + RECORD, RECORD_OBJ);
    ee.set_u32(OBJ + TRICK_RUNNING, u32::from(running));
}

#[test]
fn the_wreck_entry_writes_what_the_machine_reads() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "bike_wreck_entry",
        "player_wreck_entry",
        "orientation_transform_point",
        "trick_queue_clear",
        "jump_record_end",
        "camera_wreck_cut",
        "screen_wreck_notice",
        "app_pointer",
        "network_session",
    ]) else {
        return;
    };
    let (base, player, section_end, queue_clear) = (at[0], at[1], at[2], at[3]);
    let (jump_end, camera_cut, screen_notice) = (at[4], at[5], at[6]);
    let (app, network) = (at[7], at[8]);

    // The base entry does not end where its state writes do: past this point it places the rider
    // through the animation rig, which is another task's. The hook ends the call there, by pointing
    // the return at the sentinel the oracle waits for.
    ee.hooks.insert(section_end, Box::new(|ee| ee.set(31, u64::from(DONE))));
    let stopped = Rc::new(Cell::new(0usize));
    for hook in [HOOK_HAND_HOLD, HOOK_ANIM_STOP, HOOK_LOADS, queue_clear] {
        let seen = Rc::clone(&stopped);
        ee.hooks.insert(hook, Box::new(move |_| seen.set(seen.get() + 1)));
    }

    // The base half: the three fields, for every reason code, with and without a trick running.
    let mut pick = Pick(0x1234_5678_9abc_def1);
    for running in [false, true] {
        for reason in 0..=14u32 {
            let (clock, latch, shown, entries) =
                (pick.float(3.0), pick.flag(), pick.flag(), pick.upto(20) as i32);
            place_for_entry(&mut ee, running);
            ee.set_f32(OBJ + CLOCK, clock);
            ee.set_u32(OBJ + LATCH, u32::from(latch));
            ee.set_u32(OBJ + SHOWN, u32::from(shown));
            ee.set_u32(OBJ + ENTRIES, entries as u32);
            stopped.set(0);
            ee.call(base, &[u64::from(OBJ), u64::from(reason)], &[], BUDGET);
            assert_eq!(
                (
                    ee.u32_at(OBJ + REASON),
                    ee.f32_at(OBJ + CLOCK).to_bits(),
                    ee.u32_at(OBJ + LATCH) != 0,
                    ee.u32_at(OBJ + SHOWN) != 0,
                    ee.i32_at(OBJ + ENTRIES),
                ),
                (reason, 0f32.to_bits(), false, shown, entries),
                "reason {reason}, trick running {running}: the base entry's own writes"
            );
            // A running trick is stopped on the way through, and only then.
            assert_eq!(
                stopped.get(),
                if running { 3 } else { 0 },
                "reason {reason}, trick running {running}: whether the trick was stopped"
            );
        }
    }

    // The player half on top of it, against the port's whole entry. The base half is the stand-in
    // the proof above justifies, so that this call can run the rest of the player's entry for real.
    ee.hooks.insert(
        base,
        Box::new(|ee| {
            let reason = ee.r(5) as u32;
            ee.set_u32(OBJ + REASON, reason);
            ee.set_f32(OBJ + CLOCK, 0.0);
            ee.set_u32(OBJ + LATCH, 0);
        }),
    );
    for hook in [jump_end, camera_cut, screen_notice] {
        ee.hooks.insert(hook, Box::new(|_| {}));
    }
    for screen in [false, true] {
        for human in [false, true] {
            for reason in 0..=14u32 {
                let (clock, latch, shown, entries, wrecks) =
                    (pick.float(3.0), pick.flag(), pick.flag(), pick.upto(20) as i32, 7);
                place_for_entry(&mut ee, false);
                ee.set_u32(app, ROOT);
                ee.set_u32(ROOT + APP_SCREENS, SCREENS);
                ee.set_u32(SCREENS + APP_PLACER, 0);
                ee.set_u32(SCREENS + APP_SPLIT, 0);
                ee.set_u32(network, 0);
                ee.set_u32(OBJ + OWNER_PTR, OWNER);
                ee.set_u32(OWNER + CONTROL, if human { 1 } else { 2 });
                ee.set_u32(OWNER + SCREEN, if screen { SCREEN_OBJ } else { 0 });
                ee.set_u32(OWNER + PAD, 0);
                ee.set_f32(OBJ + CLOCK, clock);
                ee.set_u32(OBJ + LATCH, u32::from(latch));
                ee.set_u32(OBJ + SHOWN, u32::from(shown));
                ee.set_u32(OBJ + ENTRIES, entries as u32);
                ee.set_u32(OBJ + WRECKS, wrecks as u32);
                ee.call(player, &[u64::from(OBJ), u64::from(reason)], &[], BUDGET);
                let want = (
                    ee.u32_at(OBJ + REASON) as u8,
                    ee.f32_at(OBJ + CLOCK).to_bits(),
                    ee.u32_at(OBJ + LATCH) != 0,
                    ee.u32_at(OBJ + SHOWN) != 0,
                    ee.i32_at(OBJ + ENTRIES),
                    ee.i32_at(OBJ + WRECKS),
                );
                let mut ride = Ride {
                    elapsed: F(clock),
                    counting: latch,
                    wrecking: shown,
                    entries,
                    wrecks,
                    ..Ride::default()
                };
                ride.enter(Wreck(reason as u8));
                let got = (
                    ride.wreck.map_or(0, |w| w.0),
                    ride.elapsed.to_bits(),
                    ride.counting,
                    ride.wrecking,
                    ride.entries,
                    ride.wrecks,
                );
                assert_eq!(
                    got, want,
                    "reason {reason}, human {human}, screen {screen}: \
                     (reason, clock, latch, shown, entries, wrecks)"
                );
            }
        }
    }
    println!("the wreck entry: 30 base reasons and 60 player reasons, exact");
}

// ---------------------------------------------------------------------------------------------------
// 4 — the put-back
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_put_back_clears_the_reason_and_keeps_the_clock() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["player_respawn", "trick_queue_clear", "app_pointer"]) else {
        return;
    };
    let (respawn, queue_clear, app) = (at[0], at[1], at[2]);

    let placed = Rc::new(Cell::new(None));
    let seen = Rc::clone(&placed);
    ee.hooks.insert(HOOK_PLACE, Box::new(move |ee| seen.set(Some(ee.r(6) as u32))));
    for hook in [HOOK_HAND_HOLD, HOOK_ANIM_STOP, HOOK_ANIM_RESET, HOOK_SOUND, queue_clear] {
        ee.hooks.insert(hook, Box::new(|_| {}));
    }

    let mut pick = Pick(0x0bad_f00d_dead_beef);
    for human in [false, true] {
        for reason in [1u32, 4, 13, 14] {
            for kind in [0u32, 1] {
                let (clock, latch, flags) =
                    (pick.float(3.0).abs(), pick.flag(), if pick.flag() { 0x21 } else { 0 });
                place_for_entry(&mut ee, false);
                ee.set_u32(VT + SLOT_PLACE, HOOK_PLACE);
                ee.set_u32(ANIM_OBJ + ANIM_VT_PTR, ANIM_VT);
                ee.set_u32(STATE_OBJ + FLAGS, ANIM_VT);
                ee.load(ANIM_VT, &vec![0u8; 0x200]);
                ee.set_u32(ANIM_VT + SLOT_ANIM_RESET, HOOK_ANIM_RESET);
                ee.set_u32(ANIM_VT + SLOT_SOUND, HOOK_SOUND);
                ee.set_u32(app, ROOT);
                ee.set_u32(ROOT + APP_SCREENS, SCREENS);
                ee.set_u32(SCREENS + APP_SPLIT, 0);
                ee.set_u32(OBJ + OWNER_PTR, OWNER);
                ee.set_u32(OWNER + CONTROL, if human { 1 } else { 2 });
                ee.set_u32(OBJ + REASON, reason);
                ee.set_u32(OBJ + KIND, kind);
                ee.set_u32(OBJ + FRESH, 0);
                ee.set_u32(OBJ + SHOWN, 1);
                ee.set_f32(OBJ + CLOCK, clock);
                ee.set_u32(OBJ + LATCH, u32::from(latch));
                ee.set_f32(OBJ + DAMPING, 1.5);
                ee.set_u32(OBJ + ACTION, 99);
                ee.set_u32(OBJ + REQUESTED, 99);
                ee.set_u32(OBJ + RESTART, 1);
                ee.set_u32(OBJ + TRICK_RUNNING, 1);
                ee.write(RECORD_OBJ + FLAGS, 8, u128::from(flags as u64));
                placed.set(None);
                ee.call(respawn, &[u64::from(OBJ)], &[], BUDGET);
                let want = (
                    ee.u32_at(OBJ + REASON) as u8,
                    ee.u32_at(OBJ + FRESH) != 0,
                    ee.u32_at(OBJ + SHOWN) != 0,
                    ee.f32_at(OBJ + CLOCK).to_bits(),
                    ee.u32_at(OBJ + LATCH) != 0,
                    ee.read(RECORD_OBJ + FLAGS, 8) as u64,
                    ee.u32_at(OBJ + KIND),
                );
                let mut ride = Ride {
                    wreck: Some(Wreck(reason as u8)),
                    elapsed: F(clock),
                    counting: latch,
                    wrecking: true,
                    kind: if kind == 1 { Kind::Wrecked } else { Kind::Plain },
                    fresh: false,
                    ..Ride::default()
                };
                let handed = ride.respawn();
                let got = (
                    ride.wreck.map_or(0, |w| w.0),
                    ride.fresh,
                    ride.wrecking,
                    ride.elapsed.to_bits(),
                    ride.counting,
                    flags as u64 | RIDING_BIT,
                    handed as u32,
                );
                assert_eq!(
                    got, want,
                    "reason {reason}, kind {kind}, human {human}: \
                     (reason, fresh, shown, clock, latch, flags, kind)"
                );
                // What the rest of the put-back does, which this module hands to its caller: the kind
                // goes to the placement, the action is the grounded idle again, and a cannonball's
                // damping is put back the way it was halved.
                assert_eq!(placed.get(), Some(kind), "the kind the placement was handed");
                assert_eq!(
                    (ee.u32_at(OBJ + ACTION), ee.u32_at(OBJ + REQUESTED)),
                    (GROUNDED_IDLE, GROUNDED_IDLE),
                    "the action the rider is left in"
                );
                assert_eq!(
                    (ee.u32_at(OBJ + TRICK_RUNNING), ee.u32_at(OBJ + RESTART)),
                    (0, 0),
                    "the trick is forgotten"
                );
                assert_eq!(
                    ee.f32_at(OBJ + DAMPING),
                    if reason == 13 { 3.0 } else { 1.5 },
                    "a cannonball's damping doubled back"
                );
            }
        }
    }
    println!("the put-back: 16 reasons and kinds, exact");
}

// ---------------------------------------------------------------------------------------------------
// 5 — the clocks, in frames
// ---------------------------------------------------------------------------------------------------

/// A frame with nobody wrecking and nothing to ask the front end.
fn quiet(ride: &mut Ride, dt: F, sense: Sense, p: Params) -> Step {
    ride.frame_start();
    ride.step(dt, sense, p, |_| None, || false)
}

#[test]
fn the_clocks_take_the_frames_the_docs_say() {
    let dt = F(1.0 / 60.0);
    let p = Params { max_wreck_time: F(3.0), cannonball_wreck_time: F(5.0) };
    let sense = Sense {
        ai_route: true,
        owner: Some(Owner { human: true, screen: true }),
        wrecks_enabled: true,
        air_wrecks: true,
        settles: true,
    };

    // A vehicle whose class settles: the window is tested before the clock is added to, so the frame
    // the entry lands on is the first of 31 that add, and the 32nd arms the clock and puts it back to
    // zero without adding.
    let mut ride = Ride::default();
    ride.frame_start();
    let first = ride.step(dt, sense, p, |_| Some(Wreck(4)), || false);
    assert_eq!(first.wreck, Some(Wreck(4)));
    assert_eq!(ride.phase(), Phase::Down);
    assert_eq!(ride.elapsed.to_bits(), dt.to_bits(), "the entry frame is also the clock's first");
    let mut frames = 1;
    while ride.phase() == Phase::Down {
        quiet(&mut ride, dt, sense, p);
        frames += 1;
        assert!(frames < 100, "the settle window never ended");
    }
    assert_eq!((frames, ride.elapsed.to_bits()), (32, F::ZERO.to_bits()));
    // And from there the reset limit, which is tested after the add.
    let mut armed = 0;
    let reset = loop {
        let step = quiet(&mut ride, dt, sense, p);
        armed += 1;
        if let Some(reset) = step.reset {
            break reset;
        }
        assert!(armed < 1000, "the reset never came");
    };
    assert_eq!((armed, reset), (181, Reset::Page), "3 seconds at 1/60, then the wreck page");

    // A bike, which is every vehicle in this game: nothing sets the settle gate, so the clock is
    // never armed, no reset ever comes, and the rider stays down until the pad asks.
    let mut ride = Ride::default();
    let sense = Sense { settles: false, ..sense };
    ride.frame_start();
    ride.step(dt, sense, p, |_| Some(Wreck(4)), || false);
    for _ in 0..600 {
        let step = quiet(&mut ride, dt, sense, p);
        assert_eq!(step.reset, None, "a bike reset itself");
        assert_eq!(ride.phase(), Phase::Down);
    }
    assert!(ride.chord_allowed(false), "the chord is the way out");
    ride.take_chord();
    assert!(!ride.chord_allowed(false), "and it is not taken twice");
    assert_eq!(
        ride.request_reset(Kind::Wrecked, false, sense.owner),
        Some(Reset::Now),
        "the chord puts the rider back on the spot"
    );
    ride.respawn();
    assert_eq!(ride.phase(), Phase::Riding);
    ride.frame_start();
    assert!(ride.chord_allowed(false), "the latch lasts one frame");

    // Losing the driver: down and armed at the limit in one frame, no reset on it, and the reset on
    // the first frame somebody drives again.
    let mut ride = Ride::default();
    let nobody = Sense { ai_route: false, owner: None, ..sense };
    let step = quiet(&mut ride, dt, nobody, p);
    assert_eq!(step.wreck, Some(Wreck::NO_CONTROL));
    assert_eq!(step.reset, None, "a rider with nobody to put it back was put back");
    assert_eq!((ride.phase(), ride.elapsed), (Phase::Resetting, p.max_wreck_time));
    let step = quiet(&mut ride, dt, nobody, p);
    assert_eq!((step.wreck, step.reset, ride.entries), (None, None, 1), "and nothing again");
    let step = quiet(&mut ride, dt, sense, p);
    assert_eq!(step.reset, Some(Reset::Now), "a driver appearing puts it back silently");
    assert_eq!(ride.phase(), Phase::Riding);
    assert_eq!(step.riding_bit, Some(true), "and the frame ends with the record's bit back on");
    println!("the clocks: 31 frames settling, 181 waiting, and a bike that waits for the pad");
}
