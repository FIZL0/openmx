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
