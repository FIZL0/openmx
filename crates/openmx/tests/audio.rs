//! P42 — the shell's sound: the decoder against a block decoded by hand, the music sources listed and
//! switched, and a headless ride whose engine note follows the throttle.
//!
//! Each has a part that needs no disc (a hand-made block, a folder of generated WAVs, a bike whose
//! sounds are tones) and a part on the disc's own files, which skips without one.

use game_sim::collide::{Bundle, Record};
use game_sim::input::{CONTROL_MAP, Control, PADS, Report};
use game_sim::phy::Phy;
use game_sim::rider::Tables;
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

// The shell is a binary; its sound module is std, `game_sim` and `game_render` only, and is compiled
// in here as it stands.
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use audio::{BikeSounds, Clip, Engine, Hands, List, Mixer, Music, MusicSource, Note, Pick};

fn disc() -> Option<game_render::disc::Disc> {
    match game_render::disc::Disc::open_configured() {
        Ok(d) => Some(d),
        Err(e) => {
            println!("skipped the disc's half: {e}");
            None
        }
    }
}

fn rms(s: &[i16]) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    (s.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>() / s.len() as f64).sqrt() as f32
}

// ---------------------------------------------------------------------------------------------------
// the decoder
// ---------------------------------------------------------------------------------------------------

/// One block worked by hand: filter 2 (115/64, −52/64), shift 10, so a nibble `n` is `n × 4` before
/// the prediction; the channel's last two samples were 100 and 50.
///
/// ```text
///   nibble   n×4   prediction (115·h1 − 52·h2 + 32) >> 6     sample
///      1       4   (115·100 − 52·50  + 32) >> 6 =  8932>>6 = 139   143
///      7      28   (115·143 − 52·100 + 32) >> 6 = 11277>>6 = 176   204
///     −8     −32   (115·204 − 52·143 + 32) >> 6 = 16056>>6 = 250   218
///     −1      −4   (115·218 − 52·204 + 32) >> 6 = 14494>>6 = 226   222
/// ```
#[test]
fn a_block_decodes_as_it_does_by_hand() {
    let mut block = [0u8; 16];
    block[0] = 0x2a;
    block[2] = 0x71; // low nibble first: 1, then 7
    block[3] = 0xf8; // −8, then −1
    let mut prev = [100, 50];
    let out = audio::decode_block(&block, &mut prev);
    assert_eq!(&out[..4], &[143, 204, 218, 222]);
    assert_eq!(prev[1], out[26] as i32, "the history carries the block's last two samples");
    assert_eq!(prev[0], out[27] as i32);

    // Filter 0 is no prediction at all, and shift 12 leaves the nibble as it is.
    let mut flat = [0u8; 16];
    flat[0] = 0x0c;
    flat[2] = 0x9f; // −1, then −7
    let out = audio::decode_block(&flat, &mut [5000, 5000]);
    assert_eq!(&out[..3], &[-1, -7, 0]);

    // The biggest step at the smallest shift saturates rather than wrapping.
    let mut loud = [0u8; 16];
    loud[0] = 0x10; // filter 1, shift 0: 7 << 12
    loud[2] = 0x77;
    let out = audio::decode_block(&loud, &mut [32767, 32767]);
    assert_eq!(out[0], 32767);
}

/// The disc's sound archive parses to its 279 files, and a wreck and a song decode to sound.
#[test]
fn the_discs_sounds_decode() {
    let Some(disc) = disc() else { return };
    let mut lib = audio::Library::open(disc).expect("the sound archive");
    assert_eq!(lib.bank.entries.len(), 279);
    let wreck = lib.clip("Vehicle\\WRECK01").expect("WRECK01");
    assert_eq!((wreck.rate, wreck.channels), (22050, 1));
    assert!(wreck.seconds() > 0.3, "{} s", wreck.seconds());
    assert!(rms(&wreck.pcm) > 300.0, "a wreck is loud: rms {}", rms(&wreck.pcm));
    let idle = lib.clip("Engine\\250A\\idle02.vag").expect("the 250's idle");
    assert!(rms(&idle.pcm) > 100.0);
    // An extension-less, wrong-case name finds the same file.
    let again = lib.clip("engine\\250a\\IDLE02").expect("by any spelling");
    assert_eq!(again.pcm.len(), idle.pcm.len());
    // The one stereo stream that is not a song: the stadium cheer, which is a mono VAG inside.
    let cheer = lib.clip("Crowd\\cheer.vig").expect("cheer.vig");
    assert_eq!(cheer.channels, 1);
}

// ---------------------------------------------------------------------------------------------------
// the music
// ---------------------------------------------------------------------------------------------------

/// A 16-bit WAV of a tone, `seconds` long.
fn tone_wav(hz: f32, seconds: f32, rate: u32) -> Vec<u8> {
    let n = (seconds * rate as f32) as usize;
    let mut data = Vec::with_capacity(n * 4);
    for i in 0..n {
        let s = ((i as f32 / rate as f32 * hz * std::f32::consts::TAU).sin() * 9000.0) as i16;
        data.extend_from_slice(&s.to_le_bytes());
        data.extend_from_slice(&s.to_le_bytes());
    }
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&data);
    b
}

fn folder_of_songs() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("openmx-p42-music-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b-low.wav"), tone_wav(220.0, 0.5, 44100)).unwrap();
    std::fs::write(dir.join("a-high.wav"), tone_wav(880.0, 0.25, 22050)).unwrap();
    std::fs::write(dir.join("not-a-song.txt"), b"no").unwrap();
    dir
}

/// A tenth of a second of the mix.
fn listen(mixer: &mut Mixer) -> Vec<i16> {
    let mut out = vec![0i16; audio::RATE as usize / 10 * 2];
    mixer.render(&mut out);
    out
}

#[test]
fn the_music_sources_are_listed_and_switched() {
    let dir = folder_of_songs();
    let mut sources: Vec<Arc<dyn MusicSource>> = Vec::new();
    let mut on_disc = false;
    if let Some(mut d) = disc() {
        let image = game_render::disc::Disc::configured_path().unwrap();
        sources.push(Arc::new(audio::DiscMusic::read(&mut d, image).expect("the disc's songs")));
        on_disc = true;
    }
    sources.push(Arc::new(audio::FolderMusic::scan(&dir)));
    let mut music = Music::new(sources, 42);
    music.wait = true;
    let tracks = music.tracks();
    let ids: Vec<&str> = tracks.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.ends_with(&["folder:a-high.wav", "folder:b-low.wav"]), "{ids:?}");
    if on_disc {
        // music.pb lists thirteen; College Kids is a 28 ms stub on this disc and is left off.
        assert_eq!(tracks.len(), 12 + 2, "{ids:?}");
        assert_eq!(music.sources(), ["disc", "folder"]);
        let head = tracks.iter().find(|t| t.id == "disc:headstrong").expect("Headstrong");
        assert_eq!(head.label(), "Headstrong - Trapt");
        assert!(!ids.contains(&"disc:collegekids"));
    }

    let mut mixer = Mixer::new(music);
    mixer.music_volume = 1.0;
    // Off is silence.
    assert_eq!(rms(&listen(&mut mixer)), 0.0);
    assert_eq!(mixer.music.now(), None);

    // One song, and then another.
    mixer.music.set_pick(Pick::Track("folder:b-low.wav".into()));
    let low = listen(&mut mixer);
    assert_eq!(mixer.music.now(), Some("folder:b-low.wav"));
    assert!(rms(&low) > 3000.0, "the song plays: rms {}", rms(&low));
    mixer.music.set_pick(Pick::Track("folder:a-high.wav".into()));
    let high = listen(&mut mixer);
    assert_eq!(mixer.music.now(), Some("folder:a-high.wav"));
    // 880 Hz crosses zero four times as often as 220 Hz.
    let crossings = |s: &[i16]| s.chunks(2).collect::<Vec<_>>().windows(2).filter(|w| (w[0][0] < 0) != (w[1][0] < 0)).count();
    assert!(crossings(&high) > 3 * crossings(&low), "{} vs {}", crossings(&high), crossings(&low));

    // The 0.25 s song played over and over: three tenths in, it is still that song.
    for _ in 0..5 {
        listen(&mut mixer);
    }
    assert_eq!(mixer.music.now(), Some("folder:a-high.wav"));

    // The music slider.
    mixer.music_volume = 0.0;
    assert_eq!(rms(&listen(&mut mixer)), 0.0);
    mixer.music_volume = 1.0;

    // Shuffle keeps going from song to song.
    mixer.music.set_pick(Pick::Shuffle);
    let mut heard = std::collections::BTreeSet::new();
    for _ in 0..80 {
        listen(&mut mixer);
        if let Some(id) = mixer.music.now() {
            heard.insert(id.to_string());
        }
        if heard.len() >= 2 {
            break;
        }
    }
    assert!(heard.len() >= 2 || on_disc, "shuffle moved on: {heard:?}");

    if on_disc {
        mixer.music.set_pick(Pick::Track("disc:headstrong".into()));
        let song = listen(&mut mixer);
        assert_eq!(mixer.music.now(), Some("disc:headstrong"));
        // A second in, Headstrong is playing, in stereo.
        let mut second = Vec::new();
        for _ in 0..10 {
            second = listen(&mut mixer);
        }
        assert!(rms(&song) + rms(&second) > 500.0, "rms {} {}", rms(&song), rms(&second));
        assert!(second.chunks(2).any(|f| f[0] != f[1]), "two channels");
    }

    mixer.music.set_pick(Pick::Off);
    assert_eq!(rms(&listen(&mut mixer)), 0.0);
    assert_eq!(Pick::parse("off"), Pick::Off);
    assert_eq!(Pick::parse("shuffle"), Pick::Shuffle);
    assert_eq!(Pick::parse("disc:ooc").word(), "disc:ooc");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------
// the engine on a headless ride
// ---------------------------------------------------------------------------------------------------

/// Flat ground everywhere.
struct Flat;

impl World for Flat {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

fn tone(amplitude: f32, seconds: f32) -> Arc<Clip> {
    let rate = 22050;
    let pcm = (0..(seconds * rate as f32) as usize)
        .map(|i| ((i as f32 * 0.07).sin() * amplitude) as i16)
        .collect();
    Arc::new(Clip { rate, channels: 1, pcm })
}

/// A bike whose notes are tones, quiet at idle and loud on the throttle.
fn tone_bike() -> BikeSounds {
    let one = |c: Arc<Clip>, looping: bool| List { sounds: vec![(c, looping)] };
    BikeSounds {
        idle: one(tone(600.0, 0.5), true),
        low: one(tone(9000.0, 0.4), false),
        high: one(tone(9000.0, 0.4), false),
        power: List { sounds: vec![(tone(9000.0, 0.3), false), (tone(8500.0, 0.3), false)] },
        sputter: one(tone(7000.0, 0.2), false),
        decel: one(tone(2500.0, 0.6), false),
        clutch: one(tone(6000.0, 0.3), false),
        panic: List { sounds: vec![(tone(6000.0, 0.5), false), (tone(6000.0, 0.3), true)] },
        wreck: List::default(),
        low_torque_speed: 25.0 * 1.4667,
        airborne_to_decel: 0.3,
        rev_midpoint: 0.311,
        near: 20.0,
        far: 200.0,
        wreck_near: 30.0,
        wreck_far: 150.0,
    }
}

/// The pad with Cross (the throttle) held or not, as the shell's own pad sends it: the button word
/// active low and the button's pressure byte full.
fn throttle(on: bool) -> Report {
    let mut report = Report::idle();
    if on {
        let mask = Control::Cross.mask();
        let low = !mask;
        report.0[2] = (low >> 8) as u8;
        report.0[3] = low as u8;
        for binding in CONTROL_MAP {
            if binding.is_analogue() && mask & binding.mask != 0 {
                report.0[binding.pressure as usize] = 0xff;
            }
        }
    }
    report
}

/// What a second of the ride sounded like.
struct Heard {
    gas: bool,
    rms: f32,
    /// The share of the second's frames whose sounding sample is one of the throttle's: the
    /// accelerations, the power and sputter, the free rev.
    throttle_samples: f32,
    notes: Vec<Note>,
}

/// The wreck limits of a bike being ridden: `tests/ride.rs`'s block. Without them every limit is zero
/// and the stock bike is down on its first frame.
const RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// Ride the stock bike: two seconds standing, four on the throttle, three off it, the listener
/// riding along. Every frame the engine hears the ride's own hands and the mixer renders the frame's
/// 800 samples.
fn ride(sounds: &BikeSounds) -> Vec<Heard> {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(format!("{}{RIDE_LIMITS}", stock::BIKE).as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = game_sim::rider::Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    let mut mixer = Mixer::new(Music::new(Vec::new(), 1));
    mixer.effects_volume = 1.0;
    let mut engine = Engine::new(7);
    let dt = F(1.0 / 60.0);
    let mut out = Vec::new();
    for (seconds, gas) in [(2, false), (4, true), (3, false)] {
        for _ in 0..seconds {
            let mut second = Vec::new();
            let mut notes = Vec::new();
            let mut on_throttle = 0;
            for _ in 0..60 {
                let reports: [Report; PADS] = [throttle(gas), Report::missing()];
                let controls = sim.poll(dt, &reports);
                sim.step(dt, &controls, &mut Flat);
                let r = &sim.rider.ride;
                let p = r.position();
                let span = (r.engine.rev_limit.0 - r.engine.min_rpm.0).max(1.0);
                let hands = Hands {
                    grounded: r.air.on_ground,
                    gas: r.held.gas.0,
                    clutch: r.held.clutch.0,
                    speed: r.chassis.speed.0,
                    rev: (r.engine.rpm.0 - r.engine.min_rpm.0) / span,
                    at: [p.x(), p.y(), p.z()],
                };
                mixer.listeners = vec![audio::Listener { at: [p.x(), p.y() + 5.0, p.z() - 10.0], right: [1.0, 0.0, 0.0] }];
                engine.update(&hands, 1.0 / 60.0, &mut mixer, sounds);
                notes.push(engine.note);
                let mut frame = vec![0i16; 800 * 2];
                mixer.render(&mut frame);
                let sounding = engine.voice().and_then(|v| mixer.clip_of(v));
                let lists = [&sounds.low, &sounds.high, &sounds.power, &sounds.sputter, &sounds.panic];
                if sounding.is_some_and(|c| lists.iter().any(|l| l.sounds.iter().any(|(s, _)| Arc::ptr_eq(s, &c)))) {
                    on_throttle += 1;
                }
                second.extend_from_slice(&frame);
            }
            let throttle_samples = on_throttle as f32 / 60.0;
            out.push(Heard { gas, rms: rms(&second), throttle_samples, notes });
        }
    }
    out
}

fn assert_it_follows_the_throttle(heard: &[Heard], loudness: Option<f32>) {
    let line: Vec<String> = heard
        .iter()
        .map(|h| format!("{}{:.0}/{:.2}", if h.gas { "+" } else { "-" }, h.rms, h.throttle_samples))
        .collect();
    println!("rms/throttle samples by second: {}", line.join(" "));
    assert!(heard.iter().all(|h| h.rms > 100.0), "the engine is heard all along: {line:?}");
    // What sounds is the throttle's samples while it is held, and none of them standing or rolling
    // off it at the end.
    assert!(heard[..2].iter().chain(&heard[8..]).all(|h| h.throttle_samples == 0.0), "{line:?}");
    assert!(heard[3..6].iter().all(|h| h.throttle_samples == 1.0), "{line:?}");
    if let Some(ratio) = loudness {
        let idle = heard[1].rms;
        let on = heard[3].rms.min(heard[4].rms).min(heard[5].rms);
        assert!(on > ratio * idle, "the throttle is louder than the idle: {line:?}");
        assert!(heard[8].rms < on, "and off it, it falls again: {line:?}");
    }
    // The note leaves the idle on the throttle's first frames and is accelerating within a second.
    assert!(heard[0].notes.iter().chain(&heard[1].notes).all(|n| *n == Note::Idle), "standing is the idle");
    assert!(heard[2].notes.iter().any(|n| matches!(n, Note::LowAccel | Note::HighAccel)));
    assert!(heard[3].notes.iter().chain(&heard[4].notes).all(|n| !matches!(n, Note::Idle | Note::Decel)));
    // Off the throttle it decelerates, and comes back to the idle.
    assert!(heard[6].notes.contains(&Note::Decel), "{:?}", heard[6].notes);
    assert_eq!(heard[8].notes.last(), Some(&Note::Idle));
}

#[test]
fn a_headless_rides_engine_follows_the_throttle() {
    assert_it_follows_the_throttle(&ride(&tone_bike()), Some(4.0));
}

#[test]
fn the_discs_250_follows_the_throttle() {
    let Some(mut d) = disc() else { return };
    let text = d.res("Bike\\250sx.res").ok().and_then(|r| {
        let name = r.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with("vehicle_250a.aud"))?.name.clone();
        r.read(&name).ok()
    });
    let Some(text) = text else {
        println!("skipped: no vehicle_250a.aud found in Bike\\250sx.res");
        return;
    };
    let mut lib = audio::Library::open(d).unwrap();
    let sounds = BikeSounds::read(&game_render::ini::text(&text), &mut |n| lib.clip(n));
    assert_eq!(
        [&sounds.idle, &sounds.low, &sounds.high, &sounds.power, &sounds.sputter, &sounds.decel, &sounds.clutch, &sounds.panic]
            .map(|l| l.sounds.len()),
        [1, 2, 8, 4, 2, 1, 3, 2]
    );
    // The disc's idle is as loud as its accelerations; what follows the throttle is which samples play.
    assert_it_follows_the_throttle(&ride(&sounds), None);
}

/// Split screen: two riders, two listeners, one output — each rider's engine is heard at full
/// strength by their own listener however far apart they are.
#[test]
fn split_screen_hears_every_rider() {
    let mut mixer = Mixer::new(Music::new(Vec::new(), 1));
    mixer.effects_volume = 1.0;
    let clip = tone(8000.0, 1.0);
    let far = audio::Play { looping: true, at: Some([5000.0, 0.0, 0.0]), near: 20.0, far: 200.0, ..Default::default() };
    mixer.play(clip, far);
    mixer.listeners = vec![audio::Listener { at: [0.0; 3], right: [1.0, 0.0, 0.0] }];
    assert_eq!(rms(&listen(&mut mixer)), 0.0, "one listener at the other end of the map hears nothing");
    mixer.listeners.push(audio::Listener { at: [5000.0, 0.0, 10.0], right: [1.0, 0.0, 0.0] });
    assert!(rms(&listen(&mut mixer)) > 3000.0, "the second screen's listener hears it");
}
