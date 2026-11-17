//! MD178a — **a remote quad's hands come off the bars as the original's do**: SLUS-21104's bike
//! vehicle update (research/oracle.txt `# MD178a`) run in the interpreter on MD37a's spawn image, on the
//! image's own quad, against [`quad::Ride::remote_let_go`].
//!
//! With the vehicle state's kind 3 or 4 the update zeroes the stick, the brake and the clutch lever;
//! with no state or any other kind it leaves them. Every call it makes is stubbed (they are the
//! tail's and the wreck's, ported elsewhere): only the update's own stores are compared, word for word.

use super::*;

const VEHICLE_UPDATE: u32 = 0x004e_1b90;
/// The base vehicle update and the listeners' step it calls directly; the rest go through the quad's
/// vtable (the ghost step, node from body, stunt clocks, remote wreck setter, wreck test, stunt idle).
const DIRECT: [u32; 2] = [0x004a_79c0, 0x0012_8af0];
const SLOTS: [u32; 6] = [0x3a4, 0x4f8, 0x4e8, 0x4a0, 0x49c, 0x4f0];
const FRONT_END: u32 = 0x003e_ab60;
const GP: u64 = 0x003d_baf0;
/// The quad's words: its vehicle state, the request countdown and the timer (both kept off so their
/// own branches stay out), and the four the clear writes.
const STATE: u32 = 0xbc4;
const COUNTDOWN: u32 = 0x1770;
const TIMER: u32 = 0x177c;
const WORDS: [u32; 4] = [0x12a8, 0x12ac, 0x310, 0x12c0];
const STATE_AT: u32 = 0x01f8_0000;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn unit(&mut self) -> f32 {
        self.next() as f32 / (1u64 << 31) as f32
    }
    /// A word already there: 0, -0, 1.0, a held stick's or brake's, or anything at all.
    fn word(&mut self) -> u32 {
        match self.next() % 5 {
            0 => 0,
            1 => 0x8000_0000,
            2 => 1.0f32.to_bits(),
            3 => (self.unit() * 2.0 - 1.0).to_bits(),
            _ => self.next(),
        }
    }
}

#[test]
fn a_remote_quads_stick_brake_and_clutch_are_cleared_as_the_originals() {
    let Some(mut ee) = image() else { return };
    let quad = image::IMAGE_QUAD;
    let vt = ee.u32_at(quad + 4);
    assert_eq!(vt, QUAD_VTABLE, "the image's quad");
    let skip = ee.u32_at(FRONT_END) + 0x225;
    assert_eq!(ee.u8_at(skip) & 8, 0, "the still camera's skip bit is set in the image");
    let dt = F::from_bits(0x3d08_850a);
    let (r, _) = image::read_ride(&ee, quad, dt).unwrap_or_else(|e| panic!("{e}"));
    let mut ride = quad::Ride::new(r.s.p, r.s.q, atv_body::Body::default(), r.bike.clone(), Vec::<Call>::new());
    ride.tailed = Some(quad::Tailed::default());

    let ran: Rc<RefCell<Vec<u32>>> = Rc::default();
    for at in DIRECT.into_iter().chain(SLOTS.map(|o| ee.u32_at(vt + o))) {
        let l = ran.clone();
        ee.hooks.insert(at, Box::new(move |_: &mut Ee| l.borrow_mut().push(at)));
    }
    let mut pick = Pick(0x6d64_3137_3861);
    let mut tally: BTreeMap<&'static str, u32> = BTreeMap::new();
    for case in 0..10_000 {
        let kind = match pick.next() % 10 {
            0 => None,
            1 => Some(0),
            2 => Some(2),
            3 | 4 => Some(3),
            5 | 6 => Some(4),
            7 => Some(5),
            8 => Some(pick.next()),
            _ => Some(1),
        };
        ee.set_u32(quad + STATE, if kind.is_some() { STATE_AT } else { 0 });
        ee.set_u32(STATE_AT + 0x28, kind.unwrap_or(0));
        ee.set_u32(quad + COUNTDOWN, 0);
        ee.set_u32(quad + TIMER, 0);
        let was = WORDS.map(|_| pick.word());
        for (o, w) in WORDS.into_iter().zip(was) {
            ee.set_u32(quad + o, w);
        }
        ran.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(VEHICLE_UPDATE, &[u64::from(quad)], &[pick.unit() * 0.05], 50_000);
        let got = WORDS.map(|o| ee.u32_at(quad + o));

        let t = ride.tailed.as_mut().expect("the tail");
        t.tail.vehicle_state = kind;
        // Raw words, not `F::from_bits` (which normalises): a word the update leaves must come back as it went.
        let raw = |w: u32| F(f32::from_bits(w));
        (ride.bike.stick.x, ride.bike.stick.y) = (raw(was[0]), raw(was[1]));
        ride.q.brake = raw(was[2]);
        ride.bike.clutch = raw(was[3]);
        ride.remote_let_go();
        let want = [ride.bike.stick.x, ride.bike.stick.y, ride.q.brake, ride.bike.clutch].map(|f| f.0.to_bits());
        assert_eq!(got, want, "case {case}: kind {kind:?}, words {was:08x?}: the game {got:08x?}, the port {want:08x?}");

        let path = match kind {
            None => "no state",
            Some(3) => "remote, 3",
            Some(4) => "remote, 4",
            Some(1) => "driven",
            Some(_) => "another kind",
        };
        // The update reached its remote branch exactly when the port cleared.
        let remote = ran.borrow().contains(&DIRECT[1]);
        assert_eq!(remote, path.starts_with("remote"), "case {case}: kind {kind:?}: the remote branch ran {:x?}", ran.borrow());
        *tally.entry(path).or_default() += 1;
    }
    for path in ["no state", "remote, 3", "remote, 4", "driven", "another kind"] {
        let n = tally.get(path).copied().unwrap_or(0);
        assert!(n >= 500, "only {n} cases took {path}: {tally:?}");
    }

    // The rider held: the quad's update returns before the vehicle update (the update's own oracle,
    // `the_quads_update_is_the_originals`), so nothing is cleared.
    let t = ride.tailed.as_mut().expect("the tail");
    (t.tail.vehicle_state, t.tail.rider_held) = (Some(3), true);
    ride.bike.clutch = F(1.0);
    ride.remote_let_go();
    assert_eq!(ride.bike.clutch, F(1.0), "a held rider's quad keeps its lever");
}
