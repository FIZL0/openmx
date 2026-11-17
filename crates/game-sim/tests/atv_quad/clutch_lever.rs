//! MD178 — **the quad's clutch lever is the pad reader's**: SLUS-21104's pad reader for the quad (its
//! vtable's input slot, research/oracle.txt `# MD178`) run in the interpreter on MD37a's spawn image,
//! against [`quad::Ride::controls`] (and, wrecked, the let-go controls [`quad::Ride::step`] lays).
//!
//! The reader looks command 0x10 (L1, [`Command::CLUTCH`]) up in the image's own binding table and
//! stores the clutch lever as a switch: 1.0 for any strength but zero, 0 for zero (either sign), and 0
//! while the quad is wrecked. The port's lever, [`quad::Bike::clutch`], must equal the game's bit for
//! bit in every case.

use super::*;
use game_sim::bindings::Command;
use game_sim::ride::Controls;

const READER: u32 = 0x004e_36c0;
const LIVE: u32 = 0x0024_9330;
const BINDING: u32 = 0x0011_8e10;
const FRONT_END: u32 = 0x003e_ab60;
/// The listeners' message post the reader sends on a fresh press (the clutch cue), hooked to count.
const POST: u32 = 0x0027_2300;
const GP: u64 = 0x003d_baf0;
const BUDGET: u64 = 50_000_000;
/// The quad's words the reader reads and writes: the wreck word, the lever, its press latch.
const WRECKED: u32 = 0xc80;
const LEVER: u32 = 0x12c0;
const LATCH: u32 = 0x17ac;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn unit(&mut self) -> f32 {
        self.next() as f32 / (1u64 << 31) as f32
    }
    /// A control's strength: zero of either sign, a tiny one, a full press, a part press, a negative.
    fn strength(&mut self) -> f32 {
        match self.next() % 8 {
            0 | 1 => 0.0,
            2 => -0.0,
            3 => 1e-30,
            4 => 1.0,
            5 => -self.unit(),
            _ => self.unit(),
        }
    }
    /// A word the reader finds already there: 0, 1.0, or anything.
    fn word(&mut self) -> u32 {
        match self.next() % 3 {
            0 => 0,
            1 => 1.0f32.to_bits(),
            _ => self.unit().to_bits(),
        }
    }
}

#[test]
fn a_held_clutch_reaches_the_quads_lever_as_the_originals() {
    let Some(mut ee) = heap_image_named("atv-r-spawn") else { return };
    let quad = image::IMAGE_QUAD;
    assert_eq!(Command::CLUTCH.0, 0x10, "the reader's clutch command");
    assert_ne!(ee.u32_at(quad + 0xbc4), 0, "the image's quad takes no pad");
    let dt = F::from_bits(0x3d08_850a);
    let (r, _) = image::read_ride(&ee, quad, dt).unwrap_or_else(|e| panic!("{e}"));
    let mut ride = quad::Ride::new(r.s.p, r.s.q, game_sim::atv_body::Body::default(), r.bike.clone(), Vec::<Call>::new());

    ee.hooks.insert(LIVE, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    let posts = Rc::new(Cell::new(0u32));
    let p = posts.clone();
    ee.hooks.insert(POST, Box::new(move |_: &mut Ee| p.set(p.get() + 1)));
    let front = ee.u32_at(ee.u32_at(FRONT_END));
    let (record, controls) = (0x01f8_0000u32, 0x01f8_0100u32);
    let mut pick = Pick(0x6d64_3137_38);
    let mut tally: BTreeMap<&'static str, u32> = BTreeMap::new();
    for case in 0..10_000 {
        let player = pick.next() % 2;
        // Every slot the lookup can land on, a miss (-1) as the first.
        let strengths: Vec<f32> = (0..35).map(|_| pick.strength()).collect();
        for (k, &s) in strengths.iter().enumerate() {
            ee.set_f32(controls + 0x18 * k as u32 + 4, s);
        }
        let (x, y) = (pick.unit() * 2.0 - 1.0, pick.unit() * 2.0 - 1.0);
        for (k, v) in [player, controls + 0x18, x.to_bits(), y.to_bits()].into_iter().enumerate() {
            ee.set_u32(record + 4 * k as u32, v);
        }
        let wrecked = pick.next() % 4 == 0;
        ee.set_u32(quad + WRECKED, u32::from(wrecked));
        let (lever_was, latch_was) = (pick.word(), pick.word());
        ee.set_u32(quad + LEVER, lever_was);
        ee.set_u32(quad + LATCH, latch_was);

        ee.set(28, GP);
        ee.call(BINDING, &[u64::from(front), u64::from(Command::CLUTCH.0), 2, 10, u64::from(player)], &[], BUDGET);
        let slot = ee.r(2) as u32 as i32;
        assert!((-1..34).contains(&slot), "case {case}: the clutch looks up control {slot}");
        let held = strengths[(slot + 1) as usize];

        posts.set(0);
        ee.set(28, GP);
        ee.call(READER, &[u64::from(quad), u64::from(record)], &[], BUDGET);
        let got = ee.u32_at(quad + LEVER);

        ride.bike.clutch = F::from_bits(lever_was);
        let c = if wrecked { Controls::default() } else { Controls { clutch: F(held), ..Default::default() } };
        ride.controls(&c);
        let want = ride.bike.clutch.to_bits();
        assert_eq!(got, want, "case {case}: strength {held:e}, wrecked {wrecked}: the game {got:#010x}, the port {want:#010x}");

        let path = match (wrecked, held != 0.0) {
            (true, _) => "wrecked",
            (false, false) if held.is_sign_negative() => "let go, -0",
            (false, false) => "let go",
            (false, true) if held.abs() < 1e-20 => "held, tiny",
            (false, true) if held < 0.0 => "held, negative",
            (false, true) => "held",
        };
        *tally.entry(path).or_default() += 1;
        // The cue is the listeners', not the ride's: it goes out on a press the latch has not seen.
        let fresh = !wrecked && held != 0.0 && f32::from_bits(latch_was) != 1.0;
        assert_eq!(posts.get(), u32::from(fresh), "case {case}: the press cue");
    }
    for path in ["wrecked", "let go", "let go, -0", "held", "held, tiny", "held, negative"] {
        let n = tally.get(path).copied().unwrap_or(0);
        assert!(n >= 100, "only {n} cases took {path}: {tally:?}");
    }
}

fn heap_image_named(file: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md37a").join(format!("{file}.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there (MD37a's eeMemory.bin; kept-context/md37a)", path.display()));
        return None;
    }
    Some(Ee::from_ram(&path).unwrap())
}
