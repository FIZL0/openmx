//! V127: a supercross stadium's camera flashes ([`CameraFlashes`]) held against S13d's live sx01 race
//! (`context/capture/s13d/sx01-finish.jsonl`, research/s13d/run.sh), which keeps the flash object's
//! words every tick from the race's load to past its end.

use game_sim::camera_flash::{BATCH, CameraFlashes};
use game_sim::race::PodiumFlashes;
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

/// A row: its frame, its dt and the flash object's 0x70 bytes.
struct Row {
    frame: u32,
    dt: F,
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
    let Ok(kit) = Kit::load() else { return skip("no kit.toml") };
    let Ok(text) = std::fs::read_to_string(kit.ctx("capture/s13d/sx01-finish.jsonl")) else {
        return skip("capture/s13d/sx01-finish.jsonl is not there: research/s13d/run.sh records it");
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
                    flash: hex_bytes(flash),
                });
            }
        }
    }
    let Some(floor) = floor else { return skip("the capture has no world line") };
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
