//! V127: a supercross stadium's camera flashes ([`CameraFlashes`]) held against S13d's live sx01 race
//! (`context/capture/s13d/sx01-finish.jsonl`, research/s13d/run.sh), which keeps the flash object's
//! words every tick from the race's load to past its end.

use game_sim::camera_flash::{BATCH, CameraFlashes};
use game_sim::race::{Gate, PodiumFlashes, Race};
use game_sim::rng::Rng;
use ps2_float::F;
use ps2_oracle::{Kit, skip};

/// One string or number field of a capture's JSON line (the capture's lines are flat).
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(&format!("\"{key}\": "))? + key.len() + 4;
    let rest = &line[at..];
    if let Some(s) = rest.strip_prefix('"') {
        return s.split('"').next();
    }
    rest.split([',', '}']).next()
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).expect("hex")).collect()
}

/// A row: its frame, its dt, the generator's state and the flash object's 0x70 bytes.
struct Row {
    frame: u32,
    dt: F,
    rand: u64,
    flash: Vec<u8>,
}

impl Row {
    fn word(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.flash[o..o + 4].try_into().unwrap())
    }
    fn f(&self, o: usize) -> F {
        F(f32::from_bits(self.word(o)))
    }
    fn burst(&self) -> PodiumFlashes {
        PodiumFlashes { from: self.f(0x5c), length: self.f(0x64), rate: self.f(0x68), half: self.f(0x6c) }
    }
}

/// The capture's rows, and sx01's floor off its world line; `None` (said) without the capture.
fn capture() -> Option<(Vec<Row>, F)> {
    let Ok(kit) = Kit::load() else {
        skip("no kit.toml");
        return None;
    };
    let Ok(text) = std::fs::read_to_string(kit.ctx("capture/s13d/sx01-finish.jsonl")) else {
        skip("capture/s13d/sx01-finish.jsonl is not there: research/s13d/run.sh records it");
        return None;
    };
    let hex32 = |h: &str| u32::from_le_bytes(hex_bytes(h)[..4].try_into().unwrap());
    let mut rows = Vec::new();
    let mut floor = None;
    for line in text.lines() {
        match field(line, "tag") {
            // The track's flash words from +0x530: the floor (`ConstantFlashPercent`) is +0x54c.
            Some("world") => floor = field(line, "world").map(|h| F(f32::from_bits(u32::from_le_bytes(hex_bytes(h)[0x1c..0x20].try_into().unwrap())))),
            Some(_) => {}
            None if line.contains("\"poke\"") => {}
            None => {
                let Some(flash) = field(line, "flash") else { continue };
                rows.push(Row {
                    frame: field(line, "frame").unwrap().parse().unwrap(),
                    dt: F(f32::from_bits(hex32(field(line, "dt").unwrap()))),
                    rand: u64::from_le_bytes(hex_bytes(field(line, "rand").unwrap())[..8].try_into().unwrap()),
                    flash: hex_bytes(flash),
                });
            }
        }
    }
    let Some(floor) = floor else {
        skip("the capture has no world line");
        return None;
    };
    Some((rows, floor))
}

/// The object's words the port keeps, as bits: brightness, length, rate, half, floor, ready, lit,
/// head, hidden.
fn words(f: &CameraFlashes) -> [u32; 9] {
    let b = &f.burst;
    [b.from.0.to_bits(), b.length.0.to_bits(), b.rate.0.to_bits(), b.half.0.to_bits(), f.floor.0.to_bits(), f.ready as u32, f.lit_count() as u32, f.head() as u32, f.hidden() as u32]
}

fn row_words(r: &Row) -> [u32; 9] {
    [r.word(0x5c), r.word(0x64), r.word(0x68), r.word(0x6c), r.word(0x60), r.word(0x58), r.word(0x4c), r.word(0x50), r.word(0x30)]
}

/// The flash object run tick by tick on each row's dt, from the loader's object on the load's row
/// (1200 hidden, ready, the track's 0.01 floor), against every row's words bit for bit: the
/// brightness resting at the floor, each batch of 11 lit and put back as its 0.18 s runs out, the
/// queue's head going round, and — after the winner's finish hands it a burst (its words taken from the row,
/// [`game_sim::race::Supercross::race_over`] being S13d's) — the brightness climbing to 0.3 and back,
/// each batch the share of what is hidden. Past the capture's gap in rows it resumes on the first
/// batch picked and runs on until the outro stops the object (some 200 ticks after the race's end,
/// its words still to the capture's end). A late row (the recorder under load) holds the tick after. The draws are not compared: the capture's generator is the one
/// every caller shares.
#[test]
fn the_flashes_hold_s13ds_live_sx01_race() {
    let Some((rows, floor)) = capture() else { return };
    assert_eq!(floor.0, 0.01, "sx01's ConstantFlashPercent");
    let first = &rows[0];
    assert_eq!((first.word(0x58), first.word(0x4c), first.word(0x50), first.word(0x30), first.word(0x54)), (1, 0, u32::MAX, 1200, 1200), "the loader's object");
    let mut fx = CameraFlashes::new(1200, F(f32::from_bits(first.word(0x60)))).unwrap();
    assert_eq!(fx.floor.0.to_bits(), floor.0.to_bits());
    assert_eq!(fx.brightness().0.to_bits(), first.word(0x5c), "the constructor's brightness");
    let mut rng = Rng::from_state(0x1234_5678);
    let (mut held, mut bursts, mut batches, mut late, mut peak) = (0, 0, 0, 0, F::ZERO);
    let mut resume = false;
    let mut stopped = None;
    for (j, w) in rows.windows(2).enumerate() {
        let (before, now) = (&w[0], &w[1]);

        if now.frame != before.frame + 1 {
            resume = true;
            continue;
        }
        if resume {
            // Past the gap: wait for a batch to be picked, and start from it.
            if now.word(0x50) == before.word(0x50) || now.word(0x58) != 0 {
                continue;
            }
            fx = CameraFlashes::resumed(1200, now.f(0x60), now.burst(), now.word(0x50) as usize, now.word(0x4c) as usize);
            resume = false;
            continue;
        }
        let head = fx.head();
        fx.tick(before.dt, &mut rng);
        if fx.head() != head {
            batches += 1;
        }
        // A finish at the line between this tick's flashes and the row: a new burst's words.
        let want = row_words(now);
        if now.word(0x68) != fx.burst.rate.0.to_bits() || now.word(0x6c) != fx.burst.half.0.to_bits() {
            bursts += 1;
            // Its brightness too: a late row has climbed a tick on it already.
            fx.burst.from = now.f(0x5c);
            fx.burst.length = now.f(0x64);
            fx.burst.rate = now.f(0x68);
            fx.burst.half = now.f(0x6c);
        }
        let mut got = words(&fx);
        if got != want {
            // A late row (the recorder under load): it holds the tick after, and the next row
            // repeats it.
            let mut ahead = fx.clone();
            ahead.tick(now.dt, &mut rng.clone());
            if words(&ahead) == want {
                late += 1;
                got = want;
            }
        }
        // The race's outro takes over a few seconds after its end and the object is ticked no more:
        // its words stand still for longer than any batch lasts, to the capture's end.
        if got != want && now.flash == before.flash && rows[j + 1..].len() > 8 && rows[j + 1..].iter().take(8).all(|r| r.flash == now.flash) {
            stopped = Some(now.frame);
            break;
        }
        assert_eq!(got, want, "frame {} (dt {:#x}): brightness, length, rate, half, floor, ready, lit, head, hidden", now.frame, before.dt.0.to_bits());
        if fx.brightness().0 > peak.0 {
            peak = fx.brightness();
        }
        // The lit fade: the batch's time over 0.18, never above 1.
        assert!(fx.shown().all(|(_, f)| f.0 <= 1.0 && f.0 >= 0.0));
        held += 1;
    }
    assert!(held > 2500 && batches > 300, "{held} ticks, {batches} batches");
    assert!((1..=3).contains(&bursts), "{bursts} bursts");
    let stopped = stopped.expect("the outro never stopped the flashes");
    assert!(stopped > 2567 + 150, "stopped on {stopped}, before the race's end and its outro");
    assert!(late < held / 20, "{late} late rows of {held}");
    // Each burst climbs from where the last left it: the second finisher's goes past its 0.3.
    assert!(peak.0 > 0.29 && peak.0 < 0.35, "peak brightness {}", peak.0);
    println!("{held} ticks, {batches} batches, {bursts} bursts, {late} late rows, stopped on {stopped}, peak {}", peak.0);
    assert_eq!(BATCH.0, 0.18);
}

/// How many `rand()` calls take the generator from `from` to `to`, up to `most`.
fn draws(from: u64, to: u64, most: u32) -> Option<u32> {
    let mut r = Rng::from_state(from);
    (0..=most).find(|_| {
        let at = r.state() == to;
        r.rand();
        at
    })
}

/// S321: the flashes ticked where the original ticks them, last in the race's tick
/// ([`Race::flashes`]), draw their picks on the race's generator — and the capture's shared
/// generator moves with them. Each row's tick starts the race's generator at the row's own `rand`
/// state (on a race long started, gate open, no laps: nothing else in its tick draws), and the race
/// tick's draws are held against how far the capture's generator went by the next row: never further
/// than it went (every batch is `brightness × hidden` draws, nothing between ticks), and on every tick
/// where nothing else in the frame drew (a batch at rest is 11, and the capture moved exactly 11) the
/// race's generator lands on the row's state bit for bit. Other callers (the AI riders, the rest of the
/// frame) move the capture's generator too, which is why the rest only bound it. The object's words are
/// held as [`the_flashes_hold_s13ds_live_sx01_race`] holds them, to the outro.
#[test]
fn the_flashes_draw_on_the_races_generator_as_s13ds_capture_moves() {
    let Some((rows, floor)) = capture() else { return };
    let mut race = Race::new(Gate::new(), &[], F::ZERO, Rng::new());
    race.countdown.running = false;
    race.flashes = Some(Box::new(CameraFlashes::new(1200, floor).unwrap()));
    let (mut batches, mut exact, mut quiet_ticks, mut unbounded, mut skipped) = (0, 0, 0, 0, 0);
    let (mut resume, mut late_before) = (false, false);
    let mut stopped = None;
    for (j, w) in rows.windows(2).enumerate() {
        let (before, now) = (&w[0], &w[1]);
        if now.frame != before.frame + 1 {
            resume = true;
            continue;
        }
        if resume {
            if now.word(0x50) == before.word(0x50) || now.word(0x58) != 0 {
                continue;
            }
            let fx = CameraFlashes::resumed(1200, now.f(0x60), now.burst(), now.word(0x50) as usize, now.word(0x4c) as usize);
            race.flashes = Some(Box::new(fx));
            resume = false;
            continue;
        }
        let kept = race.clone();
        race.rng = Rng::from_state(before.rand);
        let head = race.flashes.as_ref().unwrap().head();
        let frame = race.tick(before.dt, &[]);
        assert_eq!(frame, game_sim::race::Frame::default(), "frame {}: the race's own tick does nothing", now.frame);
        let fx = race.flashes.as_mut().unwrap();
        let batch = fx.head() != head || (head == 0 && fx.lit_count() > 0 && kept.flashes.as_ref().unwrap().lit_count() == 0);
        let took = draws(before.rand, race.rng.state(), 1200).expect("the race's draws");
        if now.word(0x68) != fx.burst.rate.0.to_bits() || now.word(0x6c) != fx.burst.half.0.to_bits() {
            fx.burst = now.burst();
        }
        let (got, want) = (words(fx), row_words(now));
        let late = got != want && {
            let mut ahead = (**fx).clone();
            ahead.tick(now.dt, &mut Rng::new());
            words(&ahead) == want
        };
        if got != want && !late {
            if now.flash == before.flash && rows[j + 1..].len() > 8 && rows[j + 1..].iter().take(8).all(|r| r.flash == now.flash) {
                stopped = Some(now.frame);
                break;
            }
            panic!("frame {}: brightness, length, rate, half, floor, ready, lit, head, hidden {got:x?} want {want:x?}", now.frame);
        }
        // A late row (or the one after it) holds a tick that is not this one's.
        if late || late_before {
            late_before = late;
            skipped += 1;
            continue;
        }
        if batch {
            batches += 1;
            assert!(took > 0 && took as usize <= 1200, "frame {}: a batch of {took}", now.frame);
        } else {
            assert_eq!(took, 0, "frame {}: no batch, no draw", now.frame);
        }
        let Some(moved) = draws(before.rand, now.rand, 4000) else {
            unbounded += 1;
            continue;
        };
        assert!(took <= moved, "frame {}: the flashes drew {took}, the capture's generator moved {moved}", now.frame);
        if batch && moved == took {
            assert_eq!(race.rng.state(), now.rand);
            exact += 1;
        }
        if moved == 0 {
            quiet_ticks += 1;
        }
    }
    let stopped = stopped.expect("the outro never stopped the flashes");
    assert!(batches > 300, "{batches} batches");
    assert!(exact > 50, "{exact} batches the only draws of their frame");
    assert!(unbounded < 10 && skipped < 20, "{unbounded} rows past 4000 draws, {skipped} late rows");
    println!("{batches} batches, {exact} alone in their frame (race generator on the row's state), {quiet_ticks} ticks no one drew, {skipped} late, {unbounded} unbounded, stopped on {stopped}");
}

/// S321: a Sim racing on an sx track ticks its flashes in the simulation, on the race's generator, and
/// they go with a snapshot and into the state hash: decoded into another Sim, the two step on as one.
#[test]
fn a_sims_race_carries_its_flashes_through_a_snapshot() {
    let mut sim = game_sim::Sim::new(7);
    let mut race = Race::new(Gate::new(), &[], F(3.0), Rng::from_state(0x1234_5678));
    race.flashes = Some(Box::new(CameraFlashes::new(1200, F(0.01)).unwrap()));
    sim.race = Some(Box::new(race));
    let mut bare = game_sim::Sim::new(7);
    bare.race = Some(Box::new(Race::new(Gate::new(), &[], F(3.0), Rng::from_state(0x1234_5678))));
    for _ in 0..20 {
        sim.tick(game_sim::Input::default());
        bare.tick(game_sim::Input::default());
    }
    let r = sim.race.as_ref().unwrap();
    assert!(r.flashes.as_ref().unwrap().lit_count() > 0, "a batch is lit");
    // The bare race draws nothing before its drop; the flashes' batches drew on its generator.
    assert_ne!(r.rng, bare.race.as_ref().unwrap().rng, "the flashes drew on the race's generator");
    assert_ne!(sim.hash(), bare.hash());
    let bytes = game_sim::snapshot::encode(&sim);
    let mut copy = game_sim::Sim::new(7);
    game_sim::snapshot::decode(&mut copy, &bytes).unwrap();
    assert_eq!(copy.hash(), sim.hash());
    for _ in 0..30 {
        sim.tick(game_sim::Input::default());
        copy.tick(game_sim::Input::default());
    }
    assert_eq!(copy.hash(), sim.hash());
    assert_eq!(copy.race.as_ref().unwrap().flashes, sim.race.as_ref().unwrap().flashes);
}
