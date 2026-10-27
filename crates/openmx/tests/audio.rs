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
    // The one stereo stream that is not a song, the stadium cheer, is stereo too (P42 read the file
    // before it, a mono VAG, P49).
    let cheer = lib.clip("Crowd\\cheer.vig").expect("cheer.vig");
    assert_eq!((cheer.rate, cheer.channels), (22050, 2));
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
        // music.pb lists thirteen, College Kids among them (P42's 28 ms stub was the file before it).
        assert_eq!(tracks.len(), 13 + 2, "{ids:?}");
        assert_eq!(music.sources(), ["disc", "folder"]);
        let head = tracks.iter().find(|t| t.id == "disc:headstrong").expect("Headstrong");
        assert_eq!(head.label(), "Headstrong - Trapt");
        assert!(ids.contains(&"disc:collegekids"));
    }

    let mut mixer = Mixer::new(music);
    mixer.music_volume = 1.0;
    mixer.folder_volume = 1.0;
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

    // The music slider — the folder's songs are under User Soundtracks' since P56, as the
    // original's ripped songs are, and the disc's slider leaves them be.
    mixer.folder_volume = 0.0;
    assert_eq!(rms(&listen(&mut mixer)), 0.0);
    mixer.music_volume = 0.0;
    mixer.folder_volume = 1.0;
    assert!(rms(&listen(&mut mixer)) > 3000.0);
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
        overlap: 0.008,
        pitch_variance: 0,
        doppler: 1.3,
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
    mixer.engine_volume = 1.0;
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
                let hands = Hands {
                    grounded: r.air.on_ground,
                    gas: r.held.gas.0,
                    clutch: controls.clutch.0,
                    speed: r.chassis.speed.0,
                    at: [p.x(), p.y(), p.z()],
                    vel: [r.chassis.vel.x(), r.chassis.vel.y(), r.chassis.vel.z()],
                };
                mixer.listeners = vec![audio::Listener { at: [p.x(), p.y() + 5.0, p.z() - 10.0], right: [1.0, 0.0, 0.0] }];
                engine.update(&hands, 1.0 / 60.0, &mut mixer, sounds);
                notes.push(engine.note());
                let mut frame = vec![0i16; 800 * 2];
                mixer.render(&mut frame);
                // The note's own sample (one that ended inside the frame's render is followed at the
                // next frame, as the original's chain is).
                let sounding = sounds.clip(engine.brain.current).map(|c| c.0.clone());
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


// --- P49: the engine note against the original ------------------------------------------------------

/// The player's engine note in the first recording's save state: the object whose class is the
/// note's, the one that revs in the air (+0x2f0) as only the player's does.
const NOTE_STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const NOTE_RUNS: usize = 1_000;
const NOTE_FRAMES: usize = 24;
const NOTE_BUDGET: u64 = 200_000;
/// The pitch every engine sample is loaded with: 22 050 Hz in pitch units.
const OWN_PITCH: i32 = 1881;

/// The note's fields (research/oracle.txt, `engine_sound_decide` and the starters).
mod note_at {
    pub const QUEUE_COUNT: u32 = 0x44;
    pub const QUEUE: u32 = 0x4c;
    pub const GATE: u32 = 0x1f0;
    pub const HANDS: u32 = 0x200;
    /// Low, high, power, sputter, clutch.
    pub const LAST: u32 = 0x2c4;
    pub const NOTE: u32 = 0x2d8;
    pub const NEXT: u32 = 0x2dc;
    pub const OFF_GAS: u32 = 0x2e0;
    pub const AIR_TO_DECEL: u32 = 0x2e4;
    pub const AIR: u32 = 0x2e8;
    pub const HOLD_OFF_GAS: u32 = 0x2ec;
    pub const AIR_REVS: u32 = 0x2f0;
    pub const WAS_GAS: u32 = 0x2f4;
    pub const HELD: u32 = 0x2f8;
    pub const MUTED: u32 = 0x2fc;
    pub const LOW_TORQUE: u32 = 0x300;
    pub const OVERLAP: u32 = 0x304;
    pub const VARIANCE: u32 = 0x308;
    pub const MIDPOINT: u32 = 0x30c;
    /// The lists, in `Note` order from Idle.
    pub const LISTS: [u32; 8] = [0x204, 0x21c, 0x234, 0x264, 0x27c, 0x24c, 0x2ac, 0x294];
    /// One sound's length in seconds, its voice (-1: none) and its looping flag.
    pub const LENGTH: u32 = 0x24;
    pub const VOICE: u32 = 0x34;
    pub const LOOPING: u32 = 0xbc;
}

const NOTES: [Note; 11] = [
    Note::Off,
    Note::Idle,
    Note::LowAccel,
    Note::HighAccel,
    Note::Power,
    Note::Sputter,
    Note::Decel,
    Note::Clutch,
    Note::Rev,
    Note::Panic,
    Note::Unwind,
];
/// The lists' own names, in `note_at::LISTS` order.
const LISTED: [Note; 8] =
    [Note::Idle, Note::LowAccel, Note::HighAccel, Note::Power, Note::Sputter, Note::Decel, Note::Clutch, Note::Rev];

/// What the note did to its sounds, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Did {
    Stop(audio::Sample),
    /// Started, moved off its own pitch by this much.
    Start(audio::Sample, i32),
    Pause(audio::Sample, bool),
    Loop(audio::Sample, bool),
}

/// One frame's sounds as both sides see them: each sample sounding or not, how far into its voice (or
/// none), and whether the 3-D mix hears it; and the emitter's play gate.
#[derive(Clone, Default)]
struct Stage {
    playing: Vec<bool>,
    at: Vec<Option<f32>>,
    heard: Vec<bool>,
    gate: bool,
}

/// The port's side of the stage.
struct Mock<'a> {
    stage: &'a Stage,
    samples: &'a [audio::Sample],
    did: Vec<Did>,
}

impl Mock<'_> {
    fn k(&self, s: audio::Sample) -> usize {
        self.samples.iter().position(|&t| t == s).unwrap_or_else(|| panic!("{s:?} is not one of the bike's"))
    }
}

impl audio::Bed for Mock<'_> {
    fn playing(&self, s: audio::Sample) -> bool {
        self.stage.playing[self.k(s)]
    }
    fn position(&self, s: audio::Sample) -> f32 {
        self.stage.at[self.k(s)].unwrap_or(-1.0)
    }
    fn stop(&mut self, s: audio::Sample) {
        self.did.push(Did::Stop(s));
    }
    fn play(&mut self, s: audio::Sample) -> bool {
        let heard = self.stage.gate && self.stage.heard[self.k(s)];
        if heard {
            self.did.push(Did::Start(s, 0));
        }
        heard
    }
    fn detune(&mut self, s: audio::Sample, by: i32) {
        match self.did.last_mut() {
            Some(Did::Start(t, b)) if *t == s => *b = by,
            other => panic!("{s:?} detuned after {other:?}"),
        }
    }
    fn hold(&mut self, on: bool) {
        self.did.push(Did::Pause(audio::Sample { list: Note::Rev, index: 0 }, on));
        self.did.push(Did::Loop(audio::Sample::PANIC, on));
    }
}

struct Draws(game_sim::rng::Rng);

impl audio::Draw for Draws {
    fn rand(&mut self) -> i32 {
        self.0.rand()
    }
}

/// The original's side: its note, the sounds behind each sample, and what the hooks saw.
struct Original {
    ee: ps2_oracle::Ee,
    note: u32,
    sounds: Vec<u32>,
    stage: std::rc::Rc<std::cell::RefCell<Stage>>,
    did: std::rc::Rc<std::cell::RefCell<Vec<Did>>>,
}

fn original() -> Option<Original> {
    use ps2_oracle::{Ee, Kit, addr, skip};
    use std::cell::RefCell;
    use std::rc::Rc;
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(NOTE_STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of first_recording.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let class = addr("engine_sound_vtable").unwrap();
    let notes: Vec<u32> = (0..0x0200_0000u32 - 8).step_by(4).filter(|&a| ee.u32_at(a + 8) == class).collect();
    let note = *notes
        .iter()
        .find(|&&a| ee.i32_at(a + note_at::AIR_REVS) != 0)
        .unwrap_or_else(|| panic!("no note revs in the air among {notes:x?}"));
    let mut sounds = Vec::new();
    for l in note_at::LISTS {
        let n = ee.i32_at(note + l);
        let array = ee.u32_at(note + l + 8);
        sounds.extend((0..n.max(0) as u32).map(|i| ee.u32_at(array + 4 * i)));
    }
    let stage = Rc::new(RefCell::new(Stage::default()));
    let did = Rc::new(RefCell::new(Vec::new()));
    let pitch = Rc::new(RefCell::new(std::collections::HashMap::<u32, i32>::new()));
    let k = {
        let sounds = sounds.clone();
        move |p: u64| sounds.iter().position(|&s| s == p as u32).unwrap_or_else(|| panic!("{p:#x} is not a note's sound"))
    };
    let sample = {
        let samples = samples_of(&ee, note);
        move |i: usize| samples[i]
    };
    let hook = |ee: &mut Ee, name: &str, h: ps2_oracle::ee::Hook| {
        ee.hooks.insert(addr(name).unwrap(), h);
    };
    let (st, kk) = (Rc::clone(&stage), k.clone());
    hook(&mut ee, "sound_playing", Box::new(move |ee: &mut Ee| ee.set(2, u64::from(st.borrow().playing[kk(ee.r(4))]))));
    let (st, kk) = (Rc::clone(&stage), k.clone());
    hook(&mut ee, "sound_3d", Box::new(move |ee: &mut Ee| ee.set(2, u64::from(st.borrow().heard[kk(ee.r(5))]))));
    let st = Rc::clone(&stage);
    hook(&mut ee, "voice_position", Box::new(move |ee: &mut Ee| {
        let v = ee.r(5) as usize - 0x100;
        ee.set_f(0, st.borrow().at[v].expect("a voice the stage gave none"));
    }));
    hook(&mut ee, "sound_pitch", Box::new(|ee: &mut Ee| ee.set(2, OWN_PITCH as u64)));
    let p = Rc::clone(&pitch);
    hook(&mut ee, "sound_set_pitch", Box::new(move |ee: &mut Ee| {
        p.borrow_mut().insert(ee.r(4) as u32, ee.r(5) as i32 - OWN_PITCH);
    }));
    let (d, p, kk, ss) = (Rc::clone(&did), Rc::clone(&pitch), k.clone(), sample.clone());
    hook(&mut ee, "sound_start", Box::new(move |ee: &mut Ee| {
        let by = p.borrow_mut().remove(&(ee.r(4) as u32)).unwrap_or(0);
        d.borrow_mut().push(Did::Start(ss(kk(ee.r(4))), by));
    }));
    let (d, kk, ss) = (Rc::clone(&did), k.clone(), sample.clone());
    hook(&mut ee, "sound_stop", Box::new(move |ee: &mut Ee| d.borrow_mut().push(Did::Stop(ss(kk(ee.r(4)))))));
    let (d, kk, ss) = (Rc::clone(&did), k.clone(), sample.clone());
    hook(&mut ee, "sound_pause", Box::new(move |ee: &mut Ee| d.borrow_mut().push(Did::Pause(ss(kk(ee.r(4))), ee.r(5) != 0))));
    let (d, kk, ss) = (Rc::clone(&did), k.clone(), sample.clone());
    hook(&mut ee, "emitter_play", Box::new(move |ee: &mut Ee| d.borrow_mut().push(Did::Loop(ss(kk(ee.r(5))), true))));
    let (d, kk, ss) = (Rc::clone(&did), k, sample);
    hook(&mut ee, "emitter_stop", Box::new(move |ee: &mut Ee| d.borrow_mut().push(Did::Loop(ss(kk(ee.r(5))), false))));
    hook(&mut ee, "emitter_update", Box::new(|_: &mut Ee| {}));
    Some(Original { ee, note, sounds, stage, did })
}

/// Every sample of the note's lists, in the order `original()` lists their sounds.
fn samples_of(ee: &ps2_oracle::Ee, note: u32) -> Vec<audio::Sample> {
    let mut out = Vec::new();
    for (l, list) in note_at::LISTS.into_iter().zip(LISTED) {
        let n = ee.i32_at(note + l).max(0) as usize;
        out.extend((0..n).map(|index| audio::Sample { list, index }));
    }
    out
}

impl Original {
    fn k(&self, s: audio::Sample) -> usize {
        samples_of(&self.ee, self.note).iter().position(|&t| t == s).unwrap()
    }

    /// The port's note and bike written over the original's.
    fn put(&mut self, b: &audio::Brain, sounds: &BikeSounds) {
        use note_at::*;
        let k = self.k(b.current);
        let (e, ee) = (self.note, &mut self.ee);
        ee.set_u32(e + NOTE, b.note as u32);
        ee.set_u32(e + NEXT, b.next as u32);
        ee.set_f32(e + OFF_GAS, b.off_gas);
        ee.set_f32(e + AIR, b.air);
        ee.set_u32(e + HOLD_OFF_GAS, b.hold_off_gas as u32);
        ee.set_u32(e + AIR_REVS, b.air_revs as u32);
        ee.set_u32(e + WAS_GAS, b.was_gas as u32);
        ee.set_u32(e + HELD, b.held as u32);
        ee.set_u32(e + MUTED, b.muted as u32);
        for (i, &l) in b.last.iter().enumerate() {
            ee.set_u32(e + LAST + 4 * i as u32, l as u32);
        }
        ee.set_f32(e + AIR_TO_DECEL, sounds.airborne_to_decel);
        ee.set_f32(e + LOW_TORQUE, sounds.low_torque_speed);
        ee.set_f32(e + OVERLAP, sounds.overlap);
        ee.set_u32(e + VARIANCE, sounds.pitch_variance as u32);
        ee.set_f32(e + MIDPOINT, sounds.rev_midpoint);
        ee.set_u32(e + QUEUE_COUNT, 1);
        ee.set_u32(ee.u32_at(e + QUEUE), self.sounds[k]);
        let samples = samples_of(ee, e);
        for (s, &p) in samples.iter().zip(&self.sounds) {
            ee.set_f32(p + LENGTH, sounds.seconds(*s));
            ee.set_u32(p + LOOPING, sounds.looping(*s) as u32);
        }
    }

    /// The original's note, read back as the port's.
    fn brain(&self) -> audio::Brain {
        use note_at::*;
        let (e, ee) = (self.note, &self.ee);
        let note = |a: u32| *NOTES.get(ee.i32_at(e + a) as usize).unwrap_or_else(|| panic!("state {}", ee.i32_at(e + a)));
        let flag = |a: u32| ee.i32_at(e + a) != 0;
        let mut last = [0; 5];
        for (i, l) in last.iter_mut().enumerate() {
            *l = ee.i32_at(e + LAST + 4 * i as u32);
        }
        assert_eq!(ee.i32_at(e + QUEUE_COUNT), 1, "the queue is one sound");
        let head = ee.u32_at(ee.u32_at(e + QUEUE));
        let k = self.sounds.iter().position(|&s| s == head).expect("the queue's sound is a list's");
        audio::Brain {
            note: note(NOTE),
            next: note(NEXT),
            current: samples_of(ee, e)[k],
            off_gas: ee.f32_at(e + OFF_GAS),
            air: ee.f32_at(e + AIR),
            hold_off_gas: flag(HOLD_OFF_GAS),
            air_revs: flag(AIR_REVS),
            was_gas: flag(WAS_GAS),
            held: flag(HELD),
            muted: flag(MUTED),
            last,
        }
    }

    /// The hands, the stage and the generator, then one frame.
    fn frame(&mut self, h: &Hands, dt: f32, stage: &Stage, seed: u64) {
        use ps2_oracle::addr;
        let (e, ee) = (self.note, &mut self.ee);
        let hands = ee.u32_at(e + note_at::HANDS);
        let hand = |ee: &ps2_oracle::Ee, k: u32| ee.u32_at(hands + 4 * k);
        let (g, gas, speed, clutch) = (hand(ee, 0), hand(ee, 1), hand(ee, 2), hand(ee, 3));
        ee.set_u32(g, h.grounded as u32);
        ee.set_f32(gas, h.gas);
        ee.set_f32(speed, h.speed);
        ee.set_f32(clutch, h.clutch);
        let gate = ee.u8_at(e + note_at::GATE);
        ee.write(e + note_at::GATE, 1, u128::from(if stage.gate { gate | 2 } else { gate & !2 }));
        for (i, &p) in self.sounds.iter().enumerate() {
            ee.set_u32(p + note_at::VOICE, if stage.at[i].is_some() { 0x100 + i as u32 } else { u32::MAX });
        }
        ee.write(addr("rand_state").unwrap(), 8, u128::from(seed));
        *self.stage.borrow_mut() = stage.clone();
        self.did.borrow_mut().clear();
        ee.call(addr("engine_sound_frame").unwrap(), &[u64::from(e)], &[dt], NOTE_BUDGET);
    }
}

/// The bike in the state, its lists' lengths and looping flags, the parameters drawn.
fn note_bike(o: &Original, x: &mut Xs) -> BikeSounds {
    let samples = samples_of(&o.ee, o.note);
    let mut b = BikeSounds::default();
    for (s, &p) in samples.iter().zip(&o.sounds) {
        let seconds = o.ee.f32_at(p + note_at::LENGTH) * [1.0, 1.0, 0.5, 1.5][x.upto(4) as usize];
        let looping = if x.upto(4) == 0 { x.upto(2) == 1 } else { o.ee.i32_at(p + note_at::LOOPING) != 0 };
        let clip = Arc::new(Clip { rate: 22050, channels: 1, pcm: vec![0; (seconds * 22050.0) as usize] });
        let list = match s.list {
            Note::Idle => &mut b.idle,
            Note::LowAccel => &mut b.low,
            Note::HighAccel => &mut b.high,
            Note::Power => &mut b.power,
            Note::Sputter => &mut b.sputter,
            Note::Decel => &mut b.decel,
            Note::Clutch => &mut b.clutch,
            _ => &mut b.panic,
        };
        list.sounds.push((clip, looping));
    }
    b.low_torque_speed = o.ee.f32_at(o.note + note_at::LOW_TORQUE);
    b.airborne_to_decel = [0.3, 0.0, 0.1][x.upto(3) as usize];
    b.rev_midpoint = [0.216, 0.311, 0.05, 1.0][x.upto(4) as usize];
    b.overlap = [0.008, 0.008, 0.0, 0.1][x.upto(4) as usize];
    b.pitch_variance = [0, 0, 60, 300][x.upto(4) as usize];
    b
}

struct Xs(u64);

impl Xs {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn of<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.upto(v.len() as u64) as usize]
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A note anywhere its machine can be: every state, every next, the timers either side of zero.
fn some_brain(x: &mut Xs, sounds: &BikeSounds, samples: &[audio::Sample]) -> audio::Brain {
    let mut b = audio::Brain::new();
    b.note = x.of(&NOTES);
    b.next = x.of(&NOTES);
    b.current = x.of(samples);
    let u = x.unit() * 0.3;
    b.off_gas = x.of(&[0.25, 0.0, 0.01, -0.02, u]);
    let u = x.unit() * 0.3;
    b.air = x.of(&[0.3, 0.0, 0.01, -0.02, u]);
    b.hold_off_gas = x.upto(2) == 1;
    b.air_revs = x.upto(4) != 0;
    b.was_gas = x.upto(2) == 1;
    b.held = x.upto(4) == 0;
    b.muted = x.upto(16) == 0;
    let counts = [&sounds.low, &sounds.high, &sounds.power, &sounds.sputter, &sounds.clutch];
    for (l, list) in b.last.iter_mut().zip(counts) {
        *l = x.upto(list.sounds.len() as u64 + 2) as i32 - 1;
    }
    b
}

fn some_hands(x: &mut Xs, sounds: &BikeSounds) -> Hands {
    let edge = [0.0, 0.05, 0.1, 0.100_000_01, 0.5, 1.0];
    let lts = sounds.low_torque_speed;
    let u = x.unit() * 80.0;
    Hands {
        grounded: x.upto(4) != 0,
        gas: x.of(&edge),
        clutch: if x.upto(3) == 0 { x.of(&edge) } else { 0.0 },
        speed: x.of(&[0.0, 0.5, 1.0, 1.000_001, lts, lts + 0.01, lts - 0.01, 60.0, u]),
        ..Hands::default()
    }
}

fn some_stage(x: &mut Xs, sounds: &BikeSounds, samples: &[audio::Sample]) -> Stage {
    let mut s = Stage { gate: x.upto(8) != 0, ..Stage::default() };
    for &t in samples {
        let len = sounds.seconds(t);
        s.playing.push(x.upto(3) != 0);
        s.heard.push(x.upto(6) != 0);
        s.at.push(match x.upto(6) {
            0 => None,
            1 => Some(sounds.rev_midpoint + x.of(&[-0.001, 0.0, 0.001])),
            2 => Some(len - sounds.overlap + x.of(&[-0.001, 0.0, 0.001])),
            _ => Some(x.unit() * (len + 0.05)),
        });
    }
    s
}

/// **The note's frame against the original's**, over runs of frames from generated states: the state,
/// its timers and flags, the picks, the sample that is the note's, every sound stopped, started (and
/// its pitch variance), paused and looped, and the generator. A run carries its note on from frame to
/// frame, the hands, the sounds' stage and the bike's parameters drawn afresh.
#[test]
fn the_engine_note_matches_the_original() {
    let Some(mut o) = original() else { return };
    let samples = samples_of(&o.ee, o.note);
    let mut x = Xs(0x5eed_0049_e17e_0000);
    let mut reached = [0usize; 11];
    let (mut started, mut detuned, mut held, mut stopped) = (0, 0, 0, 0);
    for run in 0..NOTE_RUNS {
        let sounds = note_bike(&o, &mut x);
        let mut port = some_brain(&mut x, &sounds, &samples);
        o.put(&port, &sounds);
        assert_eq!(o.brain(), port, "run {run}: the note written");
        let mut h = some_hands(&mut x, &sounds);
        for frame in 0..NOTE_FRAMES {
            if x.upto(3) == 0 {
                h = some_hands(&mut x, &sounds);
            }
            let u = x.unit() * 0.05;
            let dt = x.of(&[1.0 / 60.0, 1.0 / 30.0, u]);
            let stage = some_stage(&mut x, &sounds, &samples);
            let seed = x.next();
            let before = port.clone();
            o.frame(&h, dt, &stage, seed);
            let mut rng = Draws(game_sim::rng::Rng::from_state(seed));
            let mut bed = Mock { stage: &stage, samples: &samples, did: Vec::new() };
            port.frame(&h, dt, &sounds, &mut bed, &mut rng);

            let ctx = format!("run {run} frame {frame}: {h:?} dt {dt}, from {before:?}");
            assert_eq!(*o.did.borrow(), bed.did, "{ctx}: the sounds");
            assert_eq!(o.brain(), port, "{ctx}: the note");
            let state = o.ee.read(ps2_oracle::addr("rand_state").unwrap(), 8) as u64;
            assert_eq!(rng.0.state(), state, "{ctx}: the generator");
            reached[port.note as usize] += 1;
            for d in &bed.did {
                match d {
                    Did::Start(_, by) => {
                        started += 1;
                        detuned += usize::from(*by != 0);
                    }
                    Did::Pause(_, true) => held += 1,
                    Did::Stop(_) => stopped += 1,
                    _ => {}
                }
            }
        }
    }
    eprintln!("reached {reached:?}; started {started} ({detuned} detuned), stopped {stopped}, held {held}");
    assert!(reached.iter().all(|&n| n > 200), "every state reached: {reached:?}");
    assert!(started > 2_000 && detuned > 500 && held > 200 && stopped > 2_000);
}

/// 16-bit PCM as a `.wav` file.
fn wav(pcm: &[i16], rate: u32, channels: u16) -> Vec<u8> {
    let bytes = (pcm.len() * 2) as u32;
    let mut out = Vec::new();
    for part in [&b"RIFF"[..], &(36 + bytes).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &1u16.to_le_bytes(),
        &channels.to_le_bytes(), &rate.to_le_bytes(), &(rate * 2 * channels as u32).to_le_bytes(),
        &(2 * channels).to_le_bytes(), &16u16.to_le_bytes(), b"data", &bytes.to_le_bytes()]
    {
        out.extend_from_slice(part);
    }
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// **The port's half of P49's recording**: the hands the original's note heard every frame of a ride
/// (logged over PINE beside a video capture of the original, `context/p49/original.csv`) played into
/// this engine with the ride's own bike sounds, `vehicle_125a.aud`, the listener standing off the
/// bike. Writes `context/p49/port.wav` and prints how often the port's note is the original's.
#[test]
#[ignore]
fn the_originals_ride_replays() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../context/p49");
    let Ok(csv) = std::fs::read_to_string(dir.join("original.csv")) else {
        println!("skipped: no context/p49/original.csv");
        return;
    };
    let Some(mut d) = disc() else { return };
    let Ok(text) = d.res("Bike\\125sx.res").and_then(|r| r.read("aud\\vehicle_125a.aud")) else {
        println!("skipped: no vehicle_125a.aud");
        return;
    };
    let mut lib = audio::Library::open(d).unwrap();
    let sounds = BikeSounds::read(&game_render::ini::text(&text), &mut |n| lib.clip(n));
    // Each sample alone too, to find in the original's recording.
    std::fs::create_dir_all(dir.join("clips")).unwrap();
    for list in LISTED {
        for (index, (clip, _)) in sounds.list(list).sounds.iter().enumerate() {
            let name = format!("{list:?}{index}.wav");
            std::fs::write(dir.join("clips").join(name), wav(&clip.pcm, clip.rate, clip.channels as u16)).unwrap();
        }
    }
    let mut mixer = Mixer::new(Music::new(Vec::new(), 1));
    mixer.effects_volume = 1.0;
    mixer.engine_volume = 1.0;
    mixer.listeners = vec![audio::Listener { at: [0.0, 5.0, -10.0], right: [1.0, 0.0, 0.0] }];
    let mut engine = Engine::new(7);
    let (mut pcm, mut same, mut rows, mut timeline) = (Vec::new(), 0, 0, String::new());
    let mut was = None;
    for line in csv.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let num = |k: usize| f[k].parse::<f32>().unwrap();
        let hands = Hands { grounded: f[2] == "1", gas: num(3), clutch: num(5), speed: num(4), at: [0.0; 3], vel: [0.0; 3] };
        engine.update(&hands, 1.0 / 30.0, &mut mixer, &sounds);
        let mut frame = vec![0i16; 1600 * 2];
        mixer.render(&mut frame);
        pcm.extend_from_slice(&frame);
        let theirs = NOTES[f[6].parse::<usize>().unwrap()];
        rows += 1;
        same += (engine.note() == theirs) as usize;
        if was != Some((engine.note(), theirs)) {
            timeline += &format!("{:.2}s {:?}/{:?}  ", rows as f32 / 30.0, engine.note(), theirs);
            was = Some((engine.note(), theirs));
        }
    }
    println!("port/original note: {timeline}");
    println!("the port's note is the original's on {same} of {rows} rows");
    std::fs::write(dir.join("port.wav"), wav(&pcm, 48_000, 2)).unwrap();
    assert_eq!(same, rows, "the port's note is the original's all along: {timeline}");
}

/// **The samples are the original's**: every sound of the ride's bike lists, as the port reads them
/// out of the disc's sound archive, is as long as the one the original loaded (its sound's own
/// length, read off the first_recording state). P42 gave every name the archive's file before it;
/// the lengths are what the note's chain counts, so a wrong file is heard and timed wrong.
#[test]
fn the_ride_bikes_samples_are_the_originals() {
    let Some(o) = original() else { return };
    let Some(mut d) = disc() else { return };
    let Ok(text) = d.res("Bike\\125sx.res").and_then(|r| r.read("aud\\vehicle_125a.aud")) else {
        println!("skipped: no vehicle_125a.aud");
        return;
    };
    let mut lib = audio::Library::open(d).unwrap();
    let sounds = BikeSounds::read(&game_render::ini::text(&text), &mut |n| lib.clip(n));
    let samples = samples_of(&o.ee, o.note);
    assert_eq!(samples.len(), 19, "the 125's lists, Rev's own empty");
    for (&s, &p) in samples.iter().zip(&o.sounds) {
        let theirs = o.ee.f32_at(p + note_at::LENGTH);
        assert!((sounds.seconds(s) - theirs).abs() < 0.002, "{s:?}: {} s here, {theirs} s in the original", sounds.seconds(s));
        assert_eq!(sounds.looping(s), o.ee.i32_at(p + note_at::LOOPING) != 0, "{s:?} loops as the original's does");
    }
}

/// The front end's sounds off the disc (P65): `ui.aud`'s five clicks, and the music's two mono
/// halves as one stereo loop at their 32 kHz.
#[test]
fn the_menus_sounds_come_off_the_disc() {
    let Some(disc) = disc() else { return };
    let sounds = audio::MenuSounds::load(disc).unwrap();
    let music = sounds.music.as_ref().expect("UI_001's halves");
    assert_eq!((music.rate, music.channels, music.frames()), (32000, 2, 1_202_124));
    assert_eq!(sounds.clicks.len(), 5);
    let up_down = sounds.clip(audio::Click::UpDown).expect("up_down");
    assert_eq!((up_down.rate, up_down.channels, up_down.frames()), (44100, 1, 280));
    assert_eq!(sounds.clip(audio::Click::LeftRight).unwrap().rate, 22050);
    for click in [audio::Click::Accept, audio::Click::Cancel, audio::Click::Locked] {
        let c = sounds.clip(click).unwrap_or_else(|| panic!("{click:?}"));
        assert!(rms(&c.pcm) > 100.0, "{click:?} is heard");
    }
    println!("the menus' sounds: {} frames of music, five clicks", music.frames());
}

/// **A landing sounds as the original's does** (P66): over S109's replayed small landing (12.9 ft/s
/// down at tick 2149, after 15 ticks in the air) and big one (41.2 ft/s at 2751, after 211), the
/// port starts no sound but the engine note's. The original's bike has no landing sound: its four
/// emitters are the engine note, the `[Wreck]` clank (only while wrecked), the race events and the
/// drive-on surface loop, and `machine.aud`'s suspension tiers are the buggy's and the trucks'. P42
/// played `buggySuspension` here — `suspen00` at 0.53 for the small one, `suspen03` at 0.85 for the
/// big — which the matched filter in the journal finds nowhere in S109's own recording.
#[test]
fn a_landing_plays_only_the_engine_as_the_originals_does() {
    let Some(d) = disc() else { return };
    let mut lib = audio::Library::open(d).unwrap();
    let (bike, rider, plane) = {
        let mut read = audio::Reading { lib: &mut lib };
        (read.bike("Bike\\125sx.res"), read.rider("Rider\\ff_fox1.res"), read.machines("machine.res"))
    };
    assert!(!bike.high.is_empty(), "the 125's engine read off the disc");
    let thumps: Vec<Arc<Clip>> = ["Vehicle\\suspen00", "Vehicle\\suspen01", "Vehicle\\suspen02", "Vehicle\\suspen03", "Vehicle\\trkbtm_2"]
        .iter()
        .map(|n| lib.clip(n).expect("machine.aud's suspension clips are on the disc"))
        .collect();
    let engine: Vec<Arc<Clip>> = [&bike.idle, &bike.low, &bike.high, &bike.power, &bike.sputter, &bike.decel, &bike.clutch, &bike.panic]
        .iter()
        .flat_map(|l| l.sounds.iter().map(|(c, _)| c.clone()))
        .collect();
    let bike = Arc::new(bike);
    for (name, air, down) in [("small", 15, -12.9f32), ("big", 211, -41.2)] {
        let mixer = Arc::new(std::sync::Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
        let mut sound = audio::Sound::new(mixer.clone());
        sound.rider = rider.clone();
        sound.plane = plane.clone();
        sound.seats.push(audio::SeatSound::new(bike.clone(), 1));
        // Riding in, the jump, the landing tick, and a second after it.
        let ticks = (0..30).map(|_| (true, 0.0)).chain((0..air).map(|i| (false, 30.0 + (down - 30.0) * i as f32 / air as f32))).chain((0..30).map(|i| (true, if i == 0 { down } else { 0.0 })));
        let mut heard = 0;
        for (t, (grounded, vy)) in ticks.enumerate() {
            let hands = Hands { grounded, gas: 1.0, clutch: 0.0, speed: 60.0, at: [0.0; 3], vel: [0.0, vy, 60.0] };
            sound.frame(&[audio::SeatFrame { hands, ..Default::default() }], 1.0 / 30.0);
            let m = mixer.lock().unwrap();
            for (clip, gain) in m.clips() {
                assert!(!thumps.iter().any(|s| s.pcm == clip.pcm), "{name} landing, tick {t}: machine.aud's suspension at gain {gain}");
                assert!(engine.iter().any(|e| Arc::ptr_eq(e, &clip)), "{name} landing, tick {t}: a sound not the engine's, {} s at gain {gain}", clip.seconds());
                heard += 1;
            }
        }
        assert!(heard > 0, "the engine note sounded through the {name} landing");
    }
}

// --- the shootoff (P85) -----------------------------------------------------------------------------

/// The original riding off free09's west edge at 70 music, Video Capture's sound on (context/capture/
/// p85/west.csv, research/journal/2026-10-23-p85-shootoff-sound): per tick, the wreck reason and the
/// chassis' centre after the tick, and what the bike's sound child had done in it: the rumble
/// playing and its volume, the boom playing, and the song's volume. 4242 is the throw, 4601 the
/// put-back; the rows between them are the same.
const WEST: [(u32, i32, f32, f32, bool, i32, bool, f32); 64] = [
    (4187, 0, 353.6556396484375, 3370.264404296875, false, 100, false, 0.699999988079071),
    (4188, 0, 353.6556396484375, 3370.264404296875, false, 100, false, 0.699999988079071),
    (4189, 0, 346.3855895996094, 3370.349365234375, true, 26, false, 0.6800000071525574),
    (4190, 0, 342.7442626953125, 3370.391845703125, true, 27, false, 0.6700000166893005),
    (4191, 0, 339.0985107421875, 3370.434814453125, true, 29, false, 0.6600000262260437),
    (4192, 0, 335.4482116699219, 3370.477783203125, true, 30, false, 0.6399999856948853),
    (4193, 0, 331.79376220703125, 3370.520751953125, true, 31, false, 0.6299999952316284),
    (4194, 0, 331.79376220703125, 3370.520751953125, true, 31, false, 0.6299999952316284),
    (4195, 0, 324.47247314453125, 3370.606689453125, true, 34, false, 0.6100000143051147),
    (4196, 0, 320.80572509765625, 3370.649658203125, true, 35, false, 0.5899999737739563),
    (4197, 0, 317.1352233886719, 3370.692626953125, true, 37, false, 0.5799999833106995),
    (4198, 0, 313.4607849121094, 3370.735595703125, true, 38, false, 0.5699999928474426),
    (4199, 0, 309.7821350097656, 3370.778564453125, true, 40, false, 0.550000011920929),
    (4200, 0, 306.09942626953125, 3370.821533203125, true, 41, false, 0.5400000214576721),
    (4201, 0, 302.412841796875, 3370.864501953125, true, 42, false, 0.5299999713897705),
    (4202, 0, 298.72265625, 3370.907470703125, true, 44, false, 0.5199999809265137),
    (4203, 0, 295.0286560058594, 3370.950927734375, true, 45, false, 0.5),
    (4204, 0, 291.3305969238281, 3370.994384765625, true, 47, false, 0.49000000953674316),
    (4205, 0, 287.62860107421875, 3371.037841796875, true, 48, false, 0.47999998927116394),
    (4206, 0, 283.9229431152344, 3371.081298828125, true, 49, false, 0.46000000834465027),
    (4207, 0, 280.2137145996094, 3371.124755859375, true, 51, false, 0.44999998807907104),
    (4208, 0, 280.2137145996094, 3371.124755859375, true, 51, false, 0.44999998807907104),
    (4209, 0, 272.783935546875, 3371.211669921875, true, 53, false, 0.41999998688697815),
    (4210, 0, 272.783935546875, 3371.211669921875, true, 53, false, 0.41999998688697815),
    (4211, 0, 265.3395080566406, 3371.298583984375, true, 56, false, 0.4000000059604645),
    (4212, 0, 261.61163330078125, 3371.342041015625, true, 58, false, 0.38999998569488525),
    (4213, 0, 257.8801574707031, 3371.385498046875, true, 59, false, 0.3700000047683716),
    (4214, 0, 254.14535522460938, 3371.428955078125, true, 60, false, 0.36000001430511475),
    (4215, 0, 254.14535522460938, 3371.428955078125, true, 60, false, 0.36000001430511475),
    (4216, 0, 246.66505432128906, 3371.515869140625, true, 63, false, 0.33000001311302185),
    (4217, 0, 242.91970825195312, 3371.559814453125, true, 65, false, 0.3199999928474426),
    (4218, 0, 239.1708221435547, 3371.603759765625, true, 66, false, 0.3100000023841858),
    (4219, 0, 239.1708221435547, 3371.603759765625, true, 66, false, 0.3100000023841858),
    (4220, 0, 231.66246032714844, 3371.691650390625, true, 69, false, 0.2800000011920929),
    (4221, 0, 227.90321350097656, 3371.735595703125, true, 70, false, 0.27000001072883606),
    (4222, 0, 224.14059448242188, 3371.779541015625, true, 72, false, 0.25),
    (4223, 0, 220.3746337890625, 3371.823486328125, true, 73, false, 0.23999999463558197),
    (4224, 0, 216.60531616210938, 3371.867431640625, true, 75, false, 0.23000000417232513),
    (4225, 0, 212.83270263671875, 3371.911376953125, true, 76, false, 0.20999999344348907),
    (4226, 0, 209.05677795410156, 3371.955322265625, true, 77, false, 0.20000000298023224),
    (4227, 0, 205.27760314941406, 3371.999267578125, true, 79, false, 0.1899999976158142),
    (4228, 0, 205.27760314941406, 3371.999267578125, true, 79, false, 0.1899999976158142),
    (4229, 0, 197.70948791503906, 3372.087158203125, true, 82, false, 0.1599999964237213),
    (4230, 0, 193.9206085205078, 3372.131103515625, true, 83, false, 0.15000000596046448),
    (4231, 0, 190.12855529785156, 3372.175048828125, true, 84, false, 0.14000000059604645),
    (4232, 0, 190.12855529785156, 3372.175048828125, true, 84, false, 0.14000000059604645),
    (4233, 0, 186.33323669433594, 3372.218994140625, true, 86, false, 0.11999999731779099),
    (4234, 0, 182.5349884033203, 3372.263427734375, true, 87, false, 0.10999999940395355),
    (4235, 0, 174.9291534423828, 3372.352294921875, true, 90, false, 0.07999999821186066),
    (4236, 0, 171.12149047851562, 3372.396728515625, true, 92, false, 0.07000000029802322),
    (4237, 0, 167.31082153320312, 3372.441162109375, true, 93, false, 0.05999999865889549),
    (4238, 0, 163.49713134765625, 3372.485595703125, true, 94, false, 0.03999999910593033),
    (4239, 0, 159.68045043945312, 3372.530029296875, true, 96, false, 0.029999999329447746),
    (4240, 0, 155.8607940673828, 3372.574462890625, true, 97, false, 0.019999999552965164),
    (4241, 0, 152.03819274902344, 3372.618896484375, true, 99, false, 0.0),
    (4242, 13, 148.21263122558594, 3372.663330078125, true, 100, false, 0.0),
    (4243, 13, 148.21263122558594, 3372.663330078125, false, 100, true, 0.0),
    (4244, 13, 148.21263122558594, 3372.663330078125, false, 100, true, 0.0),
    (4245, 13, 148.21263122558594, 3372.663330078125, false, 100, true, 0.0),
    (4599, 13, 148.21263122558594, 3372.663330078125, false, 100, true, 0.0),
    (4600, 13, 148.21263122558594, 3372.663330078125, false, 100, true, 0.0),
    (4601, 0, 148.21263122558594, 3372.663330078125, false, 100, true, 0.0),
    (4602, 0, 2867.028076171875, 3459.937744140625, false, 100, true, 0.699999988079071),
    (4603, 0, 2867.024169921875, 3459.94580078125, false, 100, true, 0.699999988079071),
];

/// free09's extent, feet, both ways.
const FREE09: f32 = 6912.0;

fn shootoff() -> audio::Shootoff {
    audio::Shootoff { cannon: Some((tone(9000.0, 8.0), false)), rumble: Some((tone(3000.0, 1.5), true)) }
}

/// One replayed tick against one captured row: the rumble, its volume, the boom (newly started on
/// `boom`), and the song's volume to the bit.
fn replay_row(
    m: &mut Mixer,
    edge: &mut audio::EdgeSound,
    sounds: &audio::Shootoff,
    row: (u32, i32, f32, f32, bool, i32, f32),
    boom: bool,
) {
    let (frame, reason, x, z, rumbling, volume, song) = row;
    let before = edge.cannon();
    let f = audio::EdgeFrame { armed: true, reason, edge: audio::edge_distance(FREE09, FREE09, [x, 0.0, z]) };
    edge.tick(m, sounds, &f);
    assert_eq!(edge.rumble().is_some_and(|id| m.playing(id)), rumbling, "the rumble at {frame}");
    if rumbling {
        let gain = m.gain_of(edge.rumble().unwrap()).unwrap();
        assert_eq!(gain.to_bits(), (volume as f32 / 100.0).to_bits(), "the rumble's volume at {frame}");
    }
    assert_eq!(edge.cannon() != before, boom, "the boom at {frame}");
    // The original's is a signed whole percent and goes a hair under 0 past the margin; a gain is 0.
    assert_eq!(m.music_gain().to_bits(), song.max(0.0).to_bits(), "the music at {frame}: {}", m.music_gain());
}

/// **A replayed shootoff sounds as the original's**: the rumble from 350 ft out, rising from 25 to 100
/// as the music fades from the settings' 70 to nothing at the margin; the throw stops the rumble and
/// plays the boom once, a tick after the reason turns, unplaced; the music stays down while the
/// rider is off, and is back whole, with no fade in, the tick after the put-back.
#[test]
fn a_replayed_shootoff_booms_and_fades_the_music_as_the_original() {
    let sounds = shootoff();
    let mut m = Mixer::new(Music::new(Vec::new(), 1));
    m.music_volume = 0.7;
    let mut edge = audio::EdgeSound::default();
    let mut booms = 0;
    for (i, &(frame, reason, x, z, rumbling, volume, cannon, song)) in WEST.iter().enumerate() {
        let boom = cannon && !WEST[i.saturating_sub(1)].6;
        booms += boom as u32;
        replay_row(&mut m, &mut edge, &sounds, (frame, reason, x, z, rumbling, volume, song), boom);
    }
    assert_eq!(booms, 1);
    let id = edge.cannon().expect("the boom");
    assert!(m.playing(id), "the boom is 8 s long and still going");
    assert!(Arc::ptr_eq(&m.clip_of(id).unwrap(), sounds.cannon.as_ref().map(|c| &c.0).unwrap()));
    assert_eq!(m.music_edge, None, "the put-back left the music scaled");

    // Off the edge the rumble stops and the music is left as it was: riding back in from 300 ft.
    let mut edge = audio::EdgeSound::default();
    let mut m = Mixer::new(Music::new(Vec::new(), 1));
    m.music_volume = 0.7;
    for (x, rumbling) in [(300.0, true), (360.0, false)] {
        let f = audio::EdgeFrame { armed: true, reason: 0, edge: audio::edge_distance(FREE09, FREE09, [x, 0.0, 3000.0]) };
        edge.tick(&mut m, &sounds, &f);
        assert_eq!(edge.rumble().is_some(), rumbling, "at {x}");
        assert_eq!(m.music_gain().to_bits(), 0.52f32.to_bits(), "at {x}");
    }
    // A bike the edge is not armed on hears nothing of it (S73).
    let mut edge = audio::EdgeSound::default();
    let mut m = Mixer::new(Music::new(Vec::new(), 1));
    for reason in [0, 13, 13, 0] {
        edge.tick(&mut m, &sounds, &audio::EdgeFrame { armed: false, reason, edge: Some(150.0) });
    }
    assert_eq!((edge.rumble(), edge.cannon(), m.music_edge), (None, None, None));
}

/// The query: nothing inside 350 ft of every edge, else the nearest edge of four.
#[test]
fn the_edge_is_asked_as_the_original_asks() {
    assert_eq!(audio::edge_distance(FREE09, FREE09, [3456.0, 0.0, 3456.0]), None);
    assert_eq!(audio::edge_distance(FREE09, FREE09, [350.0, 0.0, 6562.0]), None);
    assert_eq!(audio::edge_distance(FREE09, FREE09, [349.0, 0.0, 3456.0]), Some(349.0));
    assert_eq!(audio::edge_distance(FREE09, FREE09, [3000.0, 0.0, 6800.0]), Some(112.0));
    assert_eq!(audio::edge_distance(FREE09, FREE09, [-10.0, 0.0, 200.0]), Some(-10.0));
    assert_eq!((audio::rumble_volume(350.0), audio::rumble_volume(150.0), audio::rumble_volume(-500.0)), (25, 100, 100));
}

/// The whole of each capture of the original riding off an edge, when this checkout has them: every
/// tick the rumble, its volume, the boom and the music as the fixture's.
#[test]
fn every_captured_shootoff_replays() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../context/capture/p85");
    let Ok(read) = std::fs::read_dir(&dir) else {
        println!("skipped: no {}", dir.display());
        return;
    };
    let mut paths: Vec<_> = read.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "csv")).collect();
    paths.sort();
    let sounds = shootoff();
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no {n} in {}", path.display()));
        let (frame, armed, reason, x, z) = (col("frame_counter"), col("edge_armed"), col("reason"), col("cofg.x"), col("cofg.z"));
        let (rumble, volume, cannon, song) = (col("rumble_voice"), col("rumble_vol"), col("cannon_voice"), col("song0_vol"));
        let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
        let mut m = Mixer::new(Music::new(Vec::new(), 1));
        m.music_volume = rows[0][song].parse().unwrap();
        let mut edge = audio::EdgeSound::default();
        let (mut ticks, mut booms, mut last) = (0, 0, None::<&[&str]>);
        for r in &rows {
            assert_eq!(r[armed], "1", "{}", path.display());
            // A row read twice over one tick (frame_counter-trigger-tears): the tick was replayed.
            let same = |a: &[&str]| head.iter().enumerate().all(|(i, h)| h.starts_with("frame") || a[i] == r[i]);
            let boom = last.is_some_and(|l| l[cannon] != r[cannon]);
            if last.is_some_and(same) {
                continue;
            }
            booms += boom as u32;
            let n = |c: usize| r[c].parse::<f32>().unwrap();
            let row = (r[frame].parse().unwrap(), n(reason) as i32, n(x), n(z), r[rumble] != "-1", n(volume) as i32, n(song));
            replay_row(&mut m, &mut edge, &sounds, row, boom);
            last = Some(r);
            ticks += 1;
        }
        println!("{}: {ticks} ticks, {booms} booms", path.display());
        assert!(booms >= 1, "{}: no shootoff", path.display());
    }
}

/// **In a split, each rider has their own**: the rider at the edge fades the one music for both, and
/// the rider thrown hears the boom while the other rides on, out of their sound.
#[test]
fn a_split_booms_for_the_rider_thrown() {
    let mut s = audio::Sound::new(Arc::new(std::sync::Mutex::new(Mixer::new(Music::new(Vec::new(), 1)))));
    s.mixer.lock().unwrap().music_volume = 0.7;
    s.shootoff = shootoff();
    for i in 0..2 {
        s.seats.push(audio::SeatSound::new(Arc::new(tone_bike()), i));
    }
    let at = |reason: i32, d: Option<f32>| audio::SeatFrame {
        edge: audio::EdgeFrame { armed: true, reason, edge: d },
        ..audio::SeatFrame::default()
    };
    s.frame(&[at(0, Some(250.0)), at(0, None)], 1.0 / 30.0);
    assert_eq!(s.mixer.lock().unwrap().music_gain().to_bits(), 0.35f32.to_bits());
    assert!(s.seats[0].edge.rumble().is_some() && s.seats[1].edge.rumble().is_none());
    s.frame(&[at(13, Some(148.0)), at(0, None)], 1.0 / 30.0);
    s.frame(&[at(13, Some(148.0)), at(0, None)], 1.0 / 30.0);
    assert!(s.seats[0].edge.cannon().is_some() && s.seats[0].edge.rumble().is_none());
    assert_eq!(s.seats[1].edge.cannon(), None);
    // The other rider's wreck reason is no shootoff of theirs.
    s.frame(&[at(13, Some(148.0)), at(4, None)], 1.0 / 30.0);
    s.frame(&[at(13, Some(148.0)), at(4, None)], 1.0 / 30.0);
    assert_eq!(s.seats[1].edge.cannon(), None);
    // A pause in the cannonball: no second boom on the way out, and the music stays down.
    s.quiet();
    assert_eq!(s.seats[0].edge.cannon(), None);
    s.frame(&[at(13, Some(148.0)), at(4, None)], 1.0 / 30.0);
    assert_eq!(s.seats[0].edge.cannon(), None, "the pause boomed again");
    assert_eq!(s.mixer.lock().unwrap().music_gain(), 0.0);
    // The ride over, the music is the settings' again.
    let mixer = s.mixer.clone();
    drop(s);
    assert_eq!(mixer.lock().unwrap().music_gain().to_bits(), 0.7f32.to_bits());
}

/// The disc's `[Cannonball]`: the 8 s stereo boom, played once, and the 1.5 s mono rumble looped.
#[test]
fn the_shootoff_sounds_are_the_discs() {
    let Some(d) = disc() else { return };
    let mut lib = audio::Library::open(d).unwrap();
    let s = audio::Reading { lib: &mut lib }.shootoff("tracks\\free09_2.res");
    let (cannon, once) = s.cannon.expect("Cannon");
    let (rumble, looped) = s.rumble.expect("Rumble");
    assert!(!once && looped);
    assert_eq!((cannon.rate, cannon.channels, rumble.rate, rumble.channels), (22050, 2, 22050, 1));
    assert!((cannon.seconds() - 8.0).abs() < 0.05, "{}", cannon.seconds());
    assert!((rumble.seconds() - 1.5).abs() < 0.05, "{}", rumble.seconds());
}

/// **P172: a seat's pause holds its own sounds, and the other seat's play on.** 1P at the idle, 2P
/// on the throttle; 1P pauses. Every voice of 1P's (the engine note's samples, the limiter's loop if it is
/// up) is held where it was — still there, not moving on, its note's machine where it was — while
/// 2P's go on being mixed. On Resume they go on from where they were, the note not started over.
#[test]
fn a_seats_pause_holds_its_engine_and_the_others_play_on() {
    let mixer = Arc::new(std::sync::Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
    let mut s = audio::Sound::new(mixer.clone());
    for i in 0..2 {
        s.seats.push(audio::SeatSound::new(Arc::new(tone_bike()), i));
    }
    // 1P sits at the idle, whose loop always sounds; 2P is on the throttle.
    let riding = |x: f32, paused: bool| audio::SeatFrame {
        hands: match x == 0.0 {
            true => Hands { grounded: true, gas: 0.0, clutch: 0.0, speed: 0.0, at: [x, 0.0, 0.0], vel: [0.0; 3] },
            false => Hands { grounded: true, gas: 1.0, clutch: 0.0, speed: 30.0, at: [x, 0.0, 0.0], vel: [0.0, 0.0, 30.0] },
        },
        paused,
        ..audio::SeatFrame::default()
    };
    let tick = |s: &mut audio::Sound, one_paused: bool| {
        s.frame(&[riding(0.0, one_paused), riding(10.0, false)], 1.0 / 30.0);
        let mut out = vec![0i16; audio::RATE as usize / 30 * 2];
        mixer.lock().unwrap().render(&mut out);
    };
    for _ in 0..12 {
        tick(&mut s, false);
    }
    let where_ = |s: &audio::Sound, seat: usize| -> Vec<(u32, f32, bool)> {
        let m = mixer.lock().unwrap();
        s.seats[seat].voices().iter().filter_map(|&v| m.progress(v)).collect::<Vec<_>>()
    };
    let note = s.seats[0].engine.note();
    // The voices sounding: one that ran out in the last tick's mix is the engine's to let go.
    let sounding = |s: &audio::Sound, seat: usize| -> Vec<u64> {
        let m = mixer.lock().unwrap();
        s.seats[seat].voices().into_iter().filter(|&v| m.playing(v)).collect()
    };
    let ones = sounding(&s, 0);
    assert!(!ones.is_empty(), "1P's engine sounds before the pause");
    tick(&mut s, true);
    let held = where_(&s, 0);
    assert_eq!(held.len(), ones.len(), "the pause stopped a voice of 1P's");
    let mut twos_moved = false;
    for k in 0..45 {
        let twos = where_(&s, 1);
        tick(&mut s, true);
        let m = mixer.lock().unwrap();
        for &v in &s.seats[0].voices() {
            assert!(m.held(v), "paused tick {k}: 1P's voice {v} is not held");
        }
        for &v in &s.seats[1].voices() {
            assert!(!m.held(v), "paused tick {k}: 2P's voice {v} is held");
        }
        drop(m);
        assert_eq!(sounding(&s, 0), ones, "paused tick {k}: 1P's voices changed");
        assert_eq!(where_(&s, 0), held, "paused tick {k}: 1P's voices moved on");
        assert_eq!(s.seats[0].engine.note(), note, "paused tick {k}: 1P's note moved on");
        twos_moved |= where_(&s, 1) != twos;
    }
    assert!(twos_moved, "2P's engine stood still through 1P's pause");
    // Resume: the same voices, let go, and on from where they were held.
    tick(&mut s, false);
    let m = mixer.lock().unwrap();
    let held_at: Vec<(u64, (u32, f32, bool))> = ones.iter().copied().zip(held.iter().copied()).collect();
    let mut moved = false;
    for &(v, a) in &held_at {
        assert!(!m.held(v), "1P's voice {v} still held after Resume");
        // On from the held place (or into the next clip of its chain, or run out), not started over.
        let Some(b) = m.progress(v) else { continue };
        assert!(b.0 > a.0 || (b.0 == a.0 && b.1 >= a.1 && b.1 - a.1 < 0.05), "voice {v} held at {a:?}, after Resume {b:?}");
        moved |= b != a;
    }
    assert!(moved, "1P's voices did not go on after Resume: {held:?}");
}

/// The disc's `[biplaneWreck]` clips' lengths, `Vehicle\mchn_wreck1` to `5`, in seconds.
const WRECK_SECONDS: [f32; 5] = [1.0006, 0.8927, 0.8902, 1.1784, 1.1162];

/// `[biplaneWreck]` with tones as long as the disc's: heard from 30 ft, gone at 150, from impulse 5,
/// full at 20.
fn plane_wreck() -> audio::MachineWreck {
    audio::MachineWreck {
        list: List { sounds: WRECK_SECONDS.iter().map(|&s| (tone(8000.0, s), false)).collect() },
        near: 30.0,
        far: 150.0,
        min_impulse: 5.0,
        max_impulse: 20.0,
    }
}

/// The original's biplane, ridden off `biplane.p2m2` (P164's capture): the ticks the plane was wrecked,
/// from the crash to the put-back. Twice: a nose-in, and a cartwheel.
const PLANE_WRECKED: [(u32, u32); 2] = [(11664, 11746), (12498, 12648)];

/// Every captured tick, wrecked, whose collision record touched with a shock peak (x, y, z); the rest
/// of the wrecked and touching ticks had none.
const PLANE_HITS: [(u32, [f32; 3]); 15] = [
    (11664, [2.715177059173584, 13.51820182800293, -8.048867225646973]),
    (11665, [5.882635593414307, 17.36620330810547, -8.01076889038086]),
    (11670, [4.043940544128418, 13.015371322631836, -7.335544586181641]),
    (11740, [0.0, 14.497215270996094, -0.011274831369519234]),
    (12498, [0.2400200515985489, 9.219368934631348, -0.2956899404525757]),
    (12499, [0.2400200515985489, 9.219368934631348, -0.2956899404525757]),
    (12500, [-8.65113353729248, 0.0, 8.745697975158691]),
    (12508, [-6.099507808685303, 0.0, 5.394533634185791]),
    (12509, [-3.86954927444458, 0.0, 3.3627052307128906]),
    (12511, [-4.465231418609619, -7.362081728750844e-14, 3.7496721744537354]),
    (12521, [-1.9541597366333008, 9.98044204711914, 1.3886011838912964]),
    (12526, [-3.065109968185425, 0.0, 2.0929362773895264]),
    (12535, [-0.013712122105062008, 6.7844929695129395, 0.007083669770509005]),
    (12548, [0.9610393643379211, 22.42465591430664, -0.33531492948532104]),
    (12555, [0.13668060302734375, 5.186065673828125, -0.03312097117304802]),
];

/// The clanks the original started (the tick its sound's level was set) and their levels. The rest of
/// the hits came while one still played, or under `MinImpulse`.
const PLANE_CLANKS: [(u32, i32); 4] = [(11664, 79), (11740, 72), (12498, 46), (12548, 100)];

/// The plane's landing in the same capture: not wrecked, and its peak a hard touchdown's.
const PLANE_LANDING: (u32, [f32; 3]) = (12465, [0.8841849565505981, 10.293110847473145, -7.551541805267334]);

/// What one captured tick hands the sound, in the plane at `at`.
fn plane_tick(frame: u32, at: [f32; 3]) -> audio::SeatFrame {
    let wrecked = PLANE_WRECKED.iter().any(|&(a, b)| (a..=b).contains(&frame));
    let hit = PLANE_HITS.iter().find(|h| h.0 == frame).map(|h| h.1);
    audio::SeatFrame {
        plane: Some((0.0, at)),
        crash: audio::PlaneCrash { wrecked, touching: hit.is_some(), impulse: hit.map_or(0.0, audio::length) },
        ..audio::SeatFrame::default()
    }
}

/// Replays the capture's ticks through the sound, the mixer run a tick's worth after each, and
/// returns each clank started: its tick, its level and which of the list it was.
fn replay_plane(s: &mut audio::Sound, frames: impl Fn(u32) -> Vec<audio::SeatFrame>) -> Vec<(u32, usize, i32, usize)> {
    let mut started = Vec::new();
    let mut last = vec![None; s.seats.len()];
    let mut out = vec![0i16; audio::RATE as usize / 30 * 2];
    for frame in 11600..12700 {
        s.frame(&frames(frame), 1.0 / 30.0);
        let mut m = s.mixer.lock().unwrap();
        for (seat, ss) in s.seats.iter().enumerate() {
            let id = ss.clank.voice();
            if id.is_some() && id != last[seat] {
                let id = id.unwrap();
                let clip = m.clip_of(id).unwrap();
                let which = s.plane_wreck.list.sounds.iter().position(|c| Arc::ptr_eq(&c.0, &clip)).unwrap();
                let level = m.gain_of(id).unwrap();
                assert_eq!(level.to_bits(), ((level * 100.0).round() / 100.0).to_bits(), "a whole percent at {frame}");
                started.push((seat, frame, (level * 100.0).round() as i32, which));
            }
            last[seat] = id;
        }
        m.render(&mut out);
    }
    started.into_iter().map(|(seat, frame, level, which)| (frame, which, level, seat)).collect()
}

fn plane_sound(seats: u64) -> audio::Sound {
    let mut s = audio::Sound::new(Arc::new(std::sync::Mutex::new(Mixer::new(Music::new(Vec::new(), 1)))));
    s.plane_wreck = plane_wreck();
    for i in 0..seats {
        s.seats.push(audio::SeatSound::new(Arc::new(tone_bike()), i));
    }
    s
}

/// **A replayed biplane crash clanks as the original's**: the capture's two wrecks, every tick, start
/// a clank on each wreck tick and on the two later hits the original clanked on, at the original's
/// level, and on no other — the hits while one plays (and its tail) and the ones under `MinImpulse`
/// are quiet. No clip twice in a row.
#[test]
fn a_replayed_biplane_crash_clanks_as_the_original() {
    for seed in 0..32 {
        let mut s = plane_sound(1);
        s.seats[0] = audio::SeatSound::new(Arc::new(tone_bike()), seed);
        let started = replay_plane(&mut s, |f| vec![plane_tick(f, [100.0, 0.0, 100.0])]);
        let got: Vec<(u32, i32)> = started.iter().map(|c| (c.0, c.2)).collect();
        assert_eq!(got, PLANE_CLANKS, "seed {seed}");
        assert!(started.windows(2).all(|w| w[0].1 != w[1].1), "seed {seed}: the same clip twice {started:?}");
    }
    // The original's levels by its own rule, to the percent.
    for (frame, level) in PLANE_CLANKS {
        let hit = PLANE_HITS.iter().find(|h| h.0 == frame).unwrap().1;
        assert_eq!(audio::wreck_level(audio::length(hit), 20.0), level, "at {frame}");
    }
}

/// **A plain landing plays none**: the capture's landing, a hard touchdown never wrecked, is quiet
/// even touching; and so is a wreck with nothing touched, or touched under `MinImpulse`.
#[test]
fn a_plain_landing_plays_no_clank() {
    let mut s = plane_sound(1);
    let landing = |touching: bool, impulse: f32, wrecked: bool| audio::SeatFrame {
        plane: Some((0.0, [0.0; 3])),
        crash: audio::PlaneCrash { wrecked, touching, impulse },
        ..audio::SeatFrame::default()
    };
    let hard = audio::length(PLANE_LANDING.1);
    assert!(hard > 5.0, "{} at {}", hard, PLANE_LANDING.0);
    for f in [landing(true, hard, false), landing(true, 40.0, false), landing(false, 40.0, true), landing(true, 4.9, true)] {
        s.frame(&[f], 1.0 / 30.0);
        assert_eq!(s.seats[0].clank.voice(), None, "{:?}", f.crash);
    }
    assert_eq!(s.mixer.lock().unwrap().voices(), 0);
    // Out of the plane, a bike's wreck word is no plane crash.
    let bike = audio::SeatFrame { crash: audio::PlaneCrash { wrecked: true, touching: true, impulse: 40.0 }, ..Default::default() };
    s.frame(&[bike], 1.0 / 30.0);
    assert_eq!(s.seats[0].clank.voice(), None);
}

/// **In a split (and online, where every seat is one of these), each plane has its own**: the crashing
/// seat clanks on its ticks, the other, flying on, never; a pause hushes it.
#[test]
fn a_split_clanks_for_the_plane_crashed() {
    let mut s = plane_sound(2);
    let started = replay_plane(&mut s, |f| {
        let other = audio::SeatFrame { plane: Some((0.0, [3000.0, 0.0, 0.0])), crash: audio::PlaneCrash::default(), ..Default::default() };
        vec![plane_tick(f, [100.0, 0.0, 100.0]), other]
    });
    assert!(started.iter().all(|c| c.3 == 0), "{started:?}");
    assert_eq!(started.iter().map(|c| (c.0, c.2)).collect::<Vec<_>>(), PLANE_CLANKS);
    // Swapped: the second seat's crash, the first one's peace.
    let mut s = plane_sound(2);
    let started = replay_plane(&mut s, |f| vec![audio::SeatFrame { plane: Some((0.0, [0.0; 3])), ..Default::default() }, plane_tick(f, [0.0; 3])]);
    assert!(started.iter().all(|c| c.3 == 1) && started.len() == PLANE_CLANKS.len(), "{started:?}");
    // A clank going, then the pause: stopped, and the next hit after it clanks again.
    let mut s = plane_sound(1);
    s.frame(&[plane_tick(11664, [0.0; 3])], 1.0 / 30.0);
    let first = s.seats[0].clank.voice().expect("the crash's clank");
    s.quiet();
    assert!(!s.mixer.lock().unwrap().playing(first));
    s.frame(&[plane_tick(11665, [0.0; 3])], 1.0 / 30.0);
    assert!(s.seats[0].clank.voice().is_some_and(|id| id != first));
}

/// The disc's `[biplaneWreck]`: five mono clips, once each, heard from 30 ft to 150, from impulse 5,
/// full at 20; their lengths the tones' above.
#[test]
fn the_plane_crash_sounds_are_the_discs() {
    let Some(d) = disc() else { return };
    let mut lib = audio::Library::open(d).unwrap();
    let w = audio::Reading { lib: &mut lib }.plane_wreck("machine.res");
    assert_eq!((w.near, w.far, w.min_impulse, w.max_impulse), (30.0, 150.0, 5.0, 20.0));
    assert_eq!(w.list.sounds.len(), 5);
    for ((clip, looping), seconds) in w.list.sounds.iter().zip(WRECK_SECONDS) {
        assert!(!looping);
        assert_eq!((clip.rate, clip.channels), (22050, 1));
        assert!((clip.seconds() - seconds).abs() < 0.001, "{} against {seconds}", clip.seconds());
    }
}

// --- the rider's wreck (P180) -----------------------------------------------------------------------

/// `game-sim`'s ridden test bike, and its wreck fixture (`tests/ride.rs`' `WRECK_LIMITS`, `WRECK_PART`
/// and `RIDER_WRECKS`): a vertical limit low enough that a drop onto flat ground wrecks, a wreck body
/// and the rider's half of the wreck. Plausible numbers, not the disc's.
const WRECK_BIKE: &str = include_str!("../../game-sim/tests/freeride-bike.phy.txt");
const DROP_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 400
FreestyleDownWreckSpeedLimit = 5
GroundPitchDownWreckAngle = 85
GroundPitchUpWreckAngle = 85
AirbornePitchDownWreckAngle = 175
AirbornePitchUpWreckAngle = 175
AirborneRollWreckAngle = 175
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";
const DROP_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";
const DROP_RIDER: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
";

/// `rider.aud`'s shape in tones: a short `[Fall]` grunt, two `[IMPACT]` screams, `MinImpulse` 12.8.
/// The screams are short, so that the wreck's own is over before the rider comes down.
fn tone_rider() -> audio::RiderSounds {
    audio::RiderSounds {
        fall: List { sounds: vec![(tone(5000.0, 0.3), false)] },
        impact: List { sounds: vec![(tone(5000.0, 0.2), false), (tone(5000.0, 0.25), false)] },
        near: 30.0,
        far: 150.0,
        min_impulse: 12.8,
    }
}

/// One sound a wreck ride started: the tick, which list it is of, and the body's state then.
#[derive(Debug)]
struct Cry {
    tick: usize,
    list: &'static str,
    touching: bool,
    speed: f32,
}

/// **A wreck ride**: the bike dropped 30 ft onto flat ground with nobody on the pad, wrecked by the
/// landing, the rider thrown and lying there until the clock puts them back — `game_sim` stepping
/// at the recording's 30 Hz and the sound hearing what the shell hands it: the tick's own wreck and
/// the thrown body ([`audio::RiderBody::of`]). The mixer renders each tick's samples. Returns every
/// sound started, sorted to its list, and the wreck's reason and tick.
fn ride_a_wreck(rider: &audio::RiderSounds, bike: BikeSounds) -> (Vec<Cry>, i32, usize) {
    use game_sim::phy;
    use game_sim::ride::Controls;
    let tables = Tables::build();
    let p = Phy::read_bytes(format!("{WRECK_BIKE}{DROP_LIMITS}").as_bytes(), &tables.ride.curve);
    let mut r = game_sim::rider::Rider::new(&p, stock::MOUNTS, Vf::new(0.0, 30.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(DROP_PART.as_bytes(), "PhysicsPart1");
    r.carries_wreck(part, phy::Wrecks::read_bytes(DROP_RIDER.as_bytes()), part.inertia(Vf::new(2.5, 4.0, 7.0, 1.0)));
    let mut world = game_sim::world::Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let mixer = Arc::new(std::sync::Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
    let mut sound = audio::Sound::new(mixer.clone());
    sound.rider = rider.clone();
    let clank = bike.wreck.clone();
    sound.seats.push(audio::SeatSound::new(Arc::new(bike), 1));
    let lists: [(&'static str, &List); 3] = [("fall", &rider.fall), ("impact", &rider.impact), ("clank", &clank)];
    let mut out = vec![0i16; audio::RATE as usize / 30 * 2];
    let mut cries = Vec::new();
    let mut wreck = None;
    let mut sounding: Vec<Arc<Clip>> = Vec::new();
    for tick in 0..400 {
        let f = r.frame(&p, &tables, &Controls::default(), F(0.03333), &mut world);
        let at = r.ride.position();
        let hands = Hands { grounded: r.ride.air.on_ground, at: [at.x(), at.y(), at.z()], ..Hands::default() };
        let body = audio::RiderBody::of(&r);
        let said = f.wreck.map(|w| w.0 as i32);
        if let Some(w) = said.filter(|_| wreck.is_none()) {
            wreck = Some((w, tick));
        }
        sound.frame(&[audio::SeatFrame { hands, wreck: said, body, ..Default::default() }], 1.0 / 30.0);
        let mut m = mixer.lock().unwrap();
        let now: Vec<Arc<Clip>> = m.clips().into_iter().map(|c| c.0).collect();
        for clip in &now {
            if sounding.iter().any(|c| Arc::ptr_eq(c, clip)) {
                continue;
            }
            if let Some((list, _)) = lists.iter().find(|(_, l)| l.sounds.iter().any(|(c, _)| Arc::ptr_eq(c, clip))) {
                cries.push(Cry { tick, list, touching: body.touching, speed: body.speed });
            }
        }
        sounding = now;
        m.render(&mut out);
        if f.respawned && wreck.is_some() {
            break;
        }
    }
    let (reason, at) = wreck.expect("the drop never wrecked");
    (cries, reason, at)
}

/// A bike whose `[Wreck]` is one 1.3 s clank, the rest [`tone_bike`]'s.
fn clanking_bike() -> BikeSounds {
    BikeSounds { wreck: List { sounds: vec![(tone(7000.0, 1.3), false)] }, ..tone_bike() }
}

/// What a wreck ride's sounds should be: the wreck's own cry and the bike's clank on the wreck's tick
/// — `[IMPACT]` for the drop's reason 5 — and after it, only `[IMPACT]` screams, each on a tick the
/// body touched. With `touchdown`, at least one: the thrown rider hitting the ground once the
/// wreck's own cry is over (the disc's 0.9 s screams still hold the voice when the drop's rider lands).
fn assert_a_wreck_is_heard(cries: &[Cry], reason: i32, at: usize, touchdown: bool) {
    println!("wreck {reason} at tick {at}: {cries:?}");
    assert_eq!(reason, 5, "the drop's wreck is the vertical one");
    let entry: Vec<&str> = cries.iter().filter(|c| c.tick == at).map(|c| c.list).collect();
    assert!(entry.contains(&"impact"), "no voice on the wreck's tick: {cries:?}");
    assert!(entry.contains(&"clank"), "no clank on the wreck's tick: {cries:?}");
    assert!(cries.iter().all(|c| c.tick >= at), "a sound before the wreck: {cries:?}");
    let after: Vec<&Cry> = cries.iter().filter(|c| c.tick > at).collect();
    assert!(!touchdown || !after.is_empty(), "the rider hit the ground in silence: {cries:?}");
    for c in &after {
        assert_eq!(c.list, "impact", "after the wreck's tick, at {}: {cries:?}", c.tick);
        assert!(c.touching, "a scream off the ground, at {}: {cries:?}", c.tick);
    }
}

/// **A wreck ride hears the voice and clank** (P180): see [`assert_a_wreck_is_heard`].
#[test]
fn a_wreck_ride_hears_the_voice_and_clank() {
    let rider = tone_rider();
    let (cries, reason, at) = ride_a_wreck(&rider, clanking_bike());
    assert_a_wreck_is_heard(&cries, reason, at, true);
}

/// The same ride on the disc's own `rider.aud` and the 125's `[Wreck]`.
#[test]
fn a_wreck_ride_hears_the_discs_voice_and_clank() {
    let Some(d) = disc() else { return };
    let mut lib = audio::Library::open(d).unwrap();
    let mut read = audio::Reading { lib: &mut lib };
    let (bike, rider) = (read.bike("Bike\\125sx.res"), read.rider("Rider\\ff_fox1.res"));
    assert_eq!((rider.fall.sounds.len(), rider.impact.sounds.len(), bike.wreck.sounds.len()), (5, 5, 4));
    assert_eq!((rider.near, rider.far, rider.min_impulse), (30.0, 150.0, 12.8));
    let (cries, reason, at) = ride_a_wreck(&rider, bike);
    assert_a_wreck_is_heard(&cries, reason, at, false);
}

/// The original's rider emitter cries out only while the body is down for a reason that is not the
/// shootoff's, touching, faster than `MinImpulse`, and with nothing of its own still playing — the
/// entry's own cry included, and a short tail past it.
#[test]
fn the_rider_cries_only_down_touching_and_fast() {
    let rider = tone_rider();
    let mixer = Arc::new(std::sync::Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
    let mut sound = audio::Sound::new(mixer.clone());
    sound.rider = rider.clone();
    sound.seats.push(audio::SeatSound::new(Arc::new(clanking_bike()), 1));
    let body = |down: bool, touching: bool, speed: f32| audio::RiderBody { down, touching, speed, at: [0.0; 3] };
    let mut out = vec![0i16; audio::RATE as usize / 30 * 2];
    let mut tick = |sound: &mut audio::Sound, wreck: Option<i32>, b: audio::RiderBody| {
        sound.frame(&[audio::SeatFrame { wreck, body: b, ..Default::default() }], 1.0 / 30.0);
        let v = sound.seats[0].voice.voice();
        mixer.lock().unwrap().render(&mut out);
        v
    };
    // Slow is slow on this tick and the last: the speed it came down at.
    for b in [body(true, true, 12.8), body(false, true, 40.0), body(true, false, 40.0)] {
        assert_eq!(tick(&mut sound, None, b), None, "{b:?}");
    }
    // The fall's grunt (0.3 s) holds the voice, and then its tail; then a touch screams.
    let grunt = tick(&mut sound, Some(11), body(true, false, 40.0)).expect("the wreck's own cry");
    let mut k = 1;
    let scream = loop {
        match tick(&mut sound, None, body(true, true, 40.0)) {
            Some(v) if v != grunt => break v,
            _ => k += 1,
        }
        assert!(k < 60, "no scream in two seconds");
    };
    assert!((9..=23).contains(&k), "the scream {k} ticks after the grunt: past the 0.3 s clip and its 0.4 s tail");
    assert!(scream != grunt);
    // A shootoff's throw (13) is the edge's own sound, not this one.
    let mut sound = audio::Sound::new(mixer.clone());
    sound.rider = rider;
    sound.seats.push(audio::SeatSound::new(Arc::new(clanking_bike()), 1));
    let b = audio::RiderBody { down: false, ..body(true, true, 40.0) };
    for _ in 0..30 {
        assert_eq!(tick(&mut sound, None, b), None);
    }
}
