//! Sound (P42): the original's own engine notes, wrecks, crowd, world emitters and music,
//! read off the user's disc and mixed by the shell.
//!
//! # Where the sound is on the disc
//!
//! All of it is one member of `MX.PSR`, `Aud\audio.res`, which is not the deflate archive every
//! other `.res` is. Its front is three words (`3`, the directory's place less 8, `0x100`), then at that
//! place the count of records and a word nothing here reads, then one record per file:
//! `[u32 name length][name][u32 offset][u32 size]`, the name with no terminator
//! (`Aud\Engine\250A\idle02.VAG`). (P42 read `[offset][size][length][name]`, giving every name the
//! file before it; P49 matched the pairing to the original's own sample lengths.) The files follow the directory, each at a multiple of `0x2000`, the offsets counted from
//! the first such multiple after the directory.
//!
//! Two kinds of file, both Sony's 4-bit ADPCM ([`decode_block`]: 16 bytes in, 28 samples out):
//!
//! - **`.VAG`**, one mono sound: a `0x30`-byte header (its `VAGp` magic zeroed in the loops) with
//!   the big-endian data size at `0x0c` and the rate at `0x10`, nearly always 22050.
//! - **`.vig`**, a stereo stream (the music, the start-line ditties): a `0x800`-byte header with the
//!   per-channel size at `0x0c` and the rate at `0x10` (44100), then the two channels interleaved one
//!   16-byte block at a time.
//!
//! What plays which file is text: the `.aud` files in the bike, rider, machine and track archives
//! (`[Section]` lists of `SoundN = Engine\250A\low07`, paths under `Aud\`, the extension optional and
//! the case anything), and `aud\music\music.pb` in `appstart.res` for the songs' titles and artists.
//!
//! # What plays it
//!
//! A [`Mixer`] of voices — a clip, a pitch, a gain, a place in the world and the distances it fades
//! over — and [`Music`], heard by one or more listeners (split screen hears every local rider in one
//! output), rendered to 48 kHz stereo. [`Output`] hands that to the desktop's own player (`pw-cat`,
//! `pacat` or `aplay`, whichever is there) through a pipe, so the shell keeps its no-outside-crates
//! rule; with none there the shell runs silent and says so. Tests render the same mixer headless.
//!
//! Every sound here is presentation: nothing in this file reads or writes the simulation's generator
//! or its clock. The original's engine note *does* draw the game's generator when it picks a sample
//! (`research/oracle.txt`, `engine_sound_*`); that draw is the simulation's to port, not this file's,
//! and this file picks with its own [`Dice`].

use game_render::disc::Disc;
use game_render::ini::{self, Section};
use ps2_float::F;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

/// The rate everything is mixed at.
pub const RATE: u32 = 48_000;

/// The sound archive's name in `MX.PSR`.
pub const BANK: &str = "Aud\\audio.res";

// --- PS ADPCM -------------------------------------------------------------------------------------

/// The SPU's five prediction filters, in sixty-fourths.
const FILTER: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

/// **One ADPCM block**: byte 0 is the filter (high nibble) and shift (low nibble), byte 1 the flags,
/// bytes 2..16 twenty-eight 4-bit samples low nibble first. `prev` is the channel's last two outputs,
/// carried from block to block. (opentennis' `hst-data/src/snd.rs` `decode`, as it stands.)
pub fn decode_block(b: &[u8], prev: &mut [i32; 2]) -> [i16; 28] {
    let shift = (b[0] & 15) as u32 + 16;
    let (f1, f2) = FILTER.get((b[0] >> 4) as usize).copied().unwrap_or((0, 0));
    std::array::from_fn(|i| {
        let n = (b[2 + i / 2] >> (i % 2 * 4) & 15) as i32;
        let pcm = ((n << 28 >> shift) + ((f1 * prev[0] + f2 * prev[1] + 32) >> 6)).clamp(-0x8000, 0x7fff);
        *prev = [pcm, prev[0]];
        pcm as i16
    })
}

/// A mono ADPCM body, block by block, up to and including the first block flagged as the end.
pub fn decode_mono(data: &[u8]) -> Vec<i16> {
    let mut prev = [0; 2];
    let mut out = Vec::with_capacity(data.len() / 16 * 28);
    for b in data.chunks_exact(16) {
        out.extend_from_slice(&decode_block(b, &mut prev));
        if b[1] & 1 != 0 {
            break;
        }
    }
    out
}

/// A stereo body, the channels interleaved one block each: left, right, left, right. The samples come
/// out interleaved as well.
pub fn decode_stereo(data: &[u8]) -> Vec<i16> {
    let mut prev = [[0; 2]; 2];
    let mut out = Vec::with_capacity(data.len() / 16 * 28);
    for pair in data.chunks_exact(32) {
        let l = decode_block(&pair[..16], &mut prev[0]);
        let r = decode_block(&pair[16..], &mut prev[1]);
        for i in 0..28 {
            out.push(l[i]);
            out.push(r[i]);
        }
    }
    out
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

// --- clips ----------------------------------------------------------------------------------------

/// A decoded sound: 16-bit samples at their own rate, one or two channels interleaved.
#[derive(Clone, Debug, Default)]
pub struct Clip {
    pub rate: u32,
    pub channels: u8,
    pub pcm: Vec<i16>,
}

impl Clip {
    /// Frames (a sample per channel).
    pub fn frames(&self) -> usize {
        self.pcm.len() / self.channels.max(1) as usize
    }

    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.rate.max(1) as f32
    }

    /// A `.VAG` file: the `0x30`-byte header, then mono ADPCM.
    pub fn vag(bytes: &[u8]) -> Result<Clip, String> {
        if bytes.len() < 0x40 {
            return Err(format!("a {}-byte VAG", bytes.len()));
        }
        let size = (be32(bytes, 0x0c) as usize).min(bytes.len() - 0x30);
        let rate = match be32(bytes, 0x10) {
            r @ 4000..=96000 => r,
            _ => 22050,
        };
        Ok(Clip { rate, channels: 1, pcm: decode_mono(&bytes[0x30..0x30 + size]) })
    }

    /// A `.vig` file: the `0x800`-byte header and stereo ADPCM, or a VAG under that name, told apart by
    /// which header's size fits the file (P42 took `Crowd\cheer.vig` for one; it was the archive's
    /// file before it, P49, and every `.vig` on the disc is stereo).
    pub fn vig(bytes: &[u8]) -> Result<Clip, String> {
        if bytes.len() < 0x40 {
            return Err(format!("a {}-byte vig", bytes.len()));
        }
        let size = be32(bytes, 0x0c) as usize;
        // A stream's size runs one block pair past its file (every `.vig` on this disc), so only a
        // file too short for a 0x800 header and one channel is a VAG.
        if bytes.len() < 0x800 + size && 0x30 + size <= bytes.len() {
            return Clip::vag(bytes);
        }
        let rate = match be32(bytes, 0x10) {
            r @ 4000..=96000 => r,
            _ => 44100,
        };
        let end = (0x800 + 2 * size).min(bytes.len());
        Ok(Clip { rate, channels: 2, pcm: decode_stereo(bytes.get(0x800..end).unwrap_or(&[])) })
    }

    /// Either kind, by the name's extension.
    pub fn read(name: &str, bytes: &[u8]) -> Result<Clip, String> {
        if name.to_ascii_lowercase().ends_with(".vig") { Clip::vig(bytes) } else { Clip::vag(bytes) }
    }
}

// --- the sound archive ------------------------------------------------------------------------------

/// One file in `Aud\audio.res`: where its bytes start in the member and how many there are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankEntry {
    pub name: String,
    pub at: u64,
    pub size: u64,
}

/// `Aud\audio.res`'s directory.
#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub entries: Vec<BankEntry>,
}

/// How a name is compared: one slash style, one case, no `Aud\` in front.
fn fold(name: &str) -> String {
    let n = name.trim().replace('/', "\\").to_ascii_lowercase();
    n.strip_prefix("aud\\").map(str::to_string).unwrap_or(n)
}

fn without_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if !name[dot..].contains('\\') => &name[..dot],
        _ => name,
    }
}

impl Bank {
    /// The directory out of the member's first bytes (the directory ends before `0x6000` on this disc;
    /// 64 KiB is plenty).
    pub fn parse(front: &[u8]) -> Result<Bank, String> {
        let dir = le32(front, 4).ok_or("a sound archive with no header")? as usize + 8;
        // The count, a word nothing here reads, then each record: the name's length, the name, then
        // its offset and size. P49: P42 read the offset and size in front of the name as its own,
        // which gave every name the file before it; the original's own sample lengths (`decel01` is
        // 2.219 s in its RAM, the 28000 bytes after `Decel01.vag`'s name) are the pairing here.
        let count = le32(front, dir).ok_or("the sound archive's directory is past its front")? as usize;
        let mut at = dir + 8;
        let mut raw = Vec::with_capacity(count);
        for _ in 0..count {
            let len = le32(front, at).ok_or("the sound archive's directory runs off its front")? as usize;
            let name = front.get(at + 4..at + 4 + len).ok_or("a name past the front")?;
            at += 4 + len;
            let (off, size) = match (le32(front, at), le32(front, at + 4)) {
                (Some(o), Some(s)) => (o, s),
                _ => return Err("the sound archive's directory runs off its front".to_string()),
            };
            raw.push((off as u64, size as u64, ini::text(name)));
            at += 8;
        }
        // The data starts at the first 0x2000 boundary after the directory.
        let base = (at as u64).div_ceil(0x2000) * 0x2000;
        let entries = raw
            .into_iter()
            .map(|(off, size, name)| BankEntry { name, at: base + off, size })
            .collect();
        Ok(Bank { entries })
    }

    /// A file by the name an `.aud` gives it: under `Aud\` or not, any case, with or without its
    /// extension.
    pub fn find(&self, name: &str) -> Option<&BankEntry> {
        let want = fold(name);
        let bare = without_extension(&want).to_string();
        let exact = self.entries.iter().find(|e| fold(&e.name) == want);
        exact.or_else(|| self.entries.iter().find(|e| without_extension(&fold(&e.name)) == bare))
    }
}

/// The disc's sounds, read on demand and kept once read.
pub struct Library {
    disc: Disc,
    pub bank: Bank,
    cache: HashMap<String, Option<Arc<Clip>>>,
}

impl Library {
    pub fn open(mut disc: Disc) -> Result<Library, String> {
        let front = disc.member_part(BANK, 0, 64 << 10)?;
        let bank = Bank::parse(&front)?;
        Ok(Library { disc, bank, cache: HashMap::new() })
    }

    /// One sound by its `.aud` name, or `None` (said once) when the disc has no such file.
    pub fn clip(&mut self, name: &str) -> Option<Arc<Clip>> {
        let key = fold(name);
        if let Some(c) = self.cache.get(&key) {
            return c.clone();
        }
        let got = match self.bank.find(name).cloned() {
            None => Err(format!("no sound {name}")),
            Some(e) => self
                .disc
                .member_part(BANK, e.at, e.size as usize)
                .and_then(|b| Clip::read(&e.name, &b)),
        };
        let got = match got {
            Ok(c) => Some(Arc::new(c)),
            Err(e) => {
                println!("sound: {e}");
                None
            }
        };
        self.cache.insert(key, got.clone());
        got
    }

    /// A member of another `.res` (an `.aud` or `music.pb`) as text.
    pub fn text(&mut self, res: &str, member: &str) -> Option<String> {
        let r = self.disc.res(res).ok()?;
        r.read(member).ok().map(|b| ini::text(&b))
    }
}

// --- .aud lists -------------------------------------------------------------------------------------

/// One `.aud` section's sounds: `NumSounds`, then `SoundN` with an optional `SoundNLooping`, the
/// section's own `Looping` the default.
#[derive(Clone, Debug, Default)]
pub struct List {
    pub sounds: Vec<(Arc<Clip>, bool)>,
}

impl List {
    pub fn read(section: Option<&Section>, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> List {
        let Some(s) = section else { return List::default() };
        let looping = s.get("Looping").is_some_and(truthy);
        let count = s.number("NumSounds").map_or(16, |n| n as usize);
        let mut sounds = Vec::new();
        for i in 1..=count {
            let Some(name) = s.get(&format!("Sound{i}")) else { continue };
            let lp = s.get(&format!("Sound{i}Looping")).map_or(looping, truthy);
            if let Some(c) = lib(name) {
                sounds.push((c, lp));
            }
        }
        List { sounds }
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }

    /// The no-repeat pick the original's lists use, on this file's own dice.
    pub fn pick(&self, dice: &mut Dice, last: &mut i32) -> Option<(Arc<Clip>, bool)> {
        let n = self.sounds.len() as i32;
        if n == 0 {
            return None;
        }
        let mut i = if n < 2 { 0 } else { dice.below(n as u32) as i32 };
        if n >= 2 && i == *last {
            i = (i + 1) % n;
        }
        *last = i;
        self.sounds.get(i as usize).cloned()
    }
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "t" | "true" | "1" | "y" | "yes")
}

fn section<'a>(sections: &'a [Section], name: &str) -> Option<&'a Section> {
    sections.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// The host's own generator, for every choice the sound makes. Never the simulation's.
#[derive(Clone, Debug)]
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Dice {
        Dice(seed | 1)
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next() >> 32) * n as u64 >> 32) as u32
    }

    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

// --- the mixer --------------------------------------------------------------------------------------

/// A voice's handle.
pub type VoiceId = u64;

/// How a sound is to be played.
#[derive(Clone, Copy, Debug)]
pub struct Play {
    pub gain: f32,
    pub pitch: f32,
    pub looping: bool,
    /// Where in the world, and the distances it is at full volume within and silent beyond; `None`
    /// is heard the same by everyone (the crowd's bed, a thunderclap).
    pub at: Option<[f32; 3]>,
    pub near: f32,
    pub far: f32,
    /// Which of the volume sliders it is under.
    pub bus: Bus,
}

impl Default for Play {
    fn default() -> Play {
        Play { gain: 1.0, pitch: 1.0, looping: false, at: None, near: 30.0, far: 150.0, bus: Bus::Effects }
    }
}

/// The sliders a sound can be under besides the music's: the original's Game Audio Options page has
/// Sound FX, Crowd and Engine (P56), and Menu FX, the menus' clicks (P65).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bus {
    #[default]
    Effects,
    Engine,
    Crowd,
    Menu,
}

struct Voice {
    id: VoiceId,
    clip: Arc<Clip>,
    play: Play,
    /// In the clip's frames.
    pos: f64,
    /// What follows this clip without a gap, and where in this one it takes over (its end if `None`).
    then: Option<(Arc<Clip>, bool, Option<f64>)>,
    /// How many clips this voice has moved on to: the engine reads it to know its chain moved.
    segment: u32,
    /// The gains actually applied, eased toward the target so a change does not click.
    applied: [f32; 2],
    stopping: bool,
    /// Held where it is and silent (the engine's free rev under the limiter's loop).
    paused: bool,
    /// Held where it is and silent because its seat's pause is open (P172): a second hold over
    /// [`Voice::paused`], so a seat paused at the limiter comes back with its free rev still held.
    held: bool,
}

/// One ear: a camera's position and its right-hand direction.
#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    pub at: [f32; 3],
    pub right: [f32; 3],
}

/// Every voice and the music, mixed for every listener into one stereo stream.
pub struct Mixer {
    voices: Vec<Voice>,
    next: VoiceId,
    pub listeners: Vec<Listener>,
    pub music: Music,
    /// `0..=1`, the settings' sliders (P56): the music's — the disc's songs and any source but the
    /// folder — the user's own folder's (the original's "User Soundtracks"), and each [`Bus`]'s.
    pub music_volume: f32,
    pub folder_volume: f32,
    pub effects_volume: f32,
    pub engine_volume: f32,
    pub crowd_volume: f32,
    pub menu_volume: f32,
    /// The shootoff's fade (P85): while a rider is near the map's edge, what the music's volume is
    /// scaled by, after it is cut to whole percent as the original's settings keep it.
    pub music_edge: Option<f32>,
    /// Samples rendered since the start.
    pub rendered: u64,
    /// The front end's own music (P65), the original's `UI\UI_001` pair: one stereo loop, under the
    /// music slider, playing while [`Mixer::front_end`] is on in place of the songs.
    pub menu_music: Option<Arc<Clip>>,
    /// The front end is up: the menu's loop plays and the songs hold where they are. Off, the ride's
    /// songs play. A change fades over [`MENU_FADE`].
    pub front_end: bool,
    /// Where the fade is: 0 the songs alone, 1 the menu's loop alone.
    menu_fade: f32,
    /// The menu loop's place, in its frames.
    menu_pos: f64,
}

/// How long the menu's loop and the ride's songs take to fade one into the other (the port's; the
/// original stops one and starts the other).
pub const MENU_FADE: f32 = 1.0;

/// Where the menu's loop goes back to at its end: its second ADPCM block, which carries the SPU's
/// loop-start flag (the first is the usual silent one).
const MENU_LOOP_FROM: f64 = 28.0;

impl Mixer {
    pub fn new(music: Music) -> Mixer {
        Mixer {
            voices: Vec::new(),
            next: 1,
            listeners: vec![Listener::default()],
            music,
            music_volume: 0.8,
            folder_volume: 0.8,
            effects_volume: 0.8,
            engine_volume: 0.8,
            crowd_volume: 0.8,
            menu_volume: 0.8,
            music_edge: None,
            rendered: 0,
            menu_music: None,
            front_end: false,
            menu_fade: 0.0,
            menu_pos: 0.0,
        }
    }

    pub fn play(&mut self, clip: Arc<Clip>, play: Play) -> VoiceId {
        let id = self.next;
        self.next += 1;
        self.voices.push(Voice {
            id,
            clip,
            play,
            pos: 0.0,
            then: None,
            segment: 0,
            applied: [-1.0; 2],
            stopping: false,
            paused: false,
            held: false,
        });
        id
    }

    /// Queue the clip that follows `id`'s, butted on with no gap, at `after` seconds into the playing
    /// clip or at its end.
    pub fn chain(&mut self, id: VoiceId, clip: Arc<Clip>, looping: bool, after: Option<f32>) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            let at = after.map(|s| s as f64 * v.clip.rate as f64);
            v.then = Some((clip, looping, at));
        }
    }

    /// `(segment, seconds into it, a follower queued)` for a voice still playing.
    pub fn progress(&self, id: VoiceId) -> Option<(u32, f32, bool)> {
        let v = self.voices.iter().find(|v| v.id == id && !v.stopping)?;
        Some((v.segment, (v.pos / v.clip.rate.max(1) as f64) as f32, v.then.is_some()))
    }

    /// The clip a voice is playing now.
    pub fn clip_of(&self, id: VoiceId) -> Option<Arc<Clip>> {
        self.voices.iter().find(|v| v.id == id && !v.stopping).map(|v| v.clip.clone())
    }

    /// The gain a playing voice was last set to.
    pub fn gain_of(&self, id: VoiceId) -> Option<f32> {
        self.voices.iter().find(|v| v.id == id && !v.stopping).map(|v| v.play.gain)
    }

    pub fn playing(&self, id: VoiceId) -> bool {
        self.progress(id).is_some()
    }

    /// Move a voice, and change its gain and pitch.
    pub fn set(&mut self, id: VoiceId, gain: f32, pitch: f32, at: Option<[f32; 3]>) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.play.gain = gain;
            v.play.pitch = pitch;
            if at.is_some() {
                v.play.at = at;
            }
        }
    }

    /// Hold a voice where it is, silent, or let it go on.
    pub fn pause(&mut self, id: VoiceId, on: bool) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.paused = on;
        }
    }

    /// Hold a voice where it is, silent, for its seat's pause (P172), or let it go on from there.
    /// Apart from [`Mixer::pause`], which the engine note keeps for itself.
    pub fn hold(&mut self, id: VoiceId, on: bool) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.held = on;
        }
    }

    /// Whether a voice is held for its seat's pause.
    pub fn held(&self, id: VoiceId) -> bool {
        self.voices.iter().any(|v| v.id == id && v.held)
    }

    /// The listener nearest a place.
    pub fn nearest(&self, at: [f32; 3]) -> Option<[f32; 3]> {
        let d = |l: &Listener| (0..3).map(|i| (l.at[i] - at[i]).powi(2)).sum::<f32>();
        self.listeners.iter().min_by(|a, b| d(a).total_cmp(&d(b))).map(|l| l.at)
    }

    /// Someone is nearer a place than `far`: a positional sound beyond it is not started.
    pub fn within(&self, at: [f32; 3], far: f32) -> bool {
        self.nearest(at).is_none_or(|l| (0..3).map(|i| (l[i] - at[i]).powi(2)).sum::<f32>() < far * far)
    }

    /// Fade a voice out over a few milliseconds and drop it.
    pub fn stop(&mut self, id: VoiceId) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.stopping = true;
        }
    }

    pub fn stop_all(&mut self) {
        for v in &mut self.voices {
            v.stopping = true;
        }
    }

    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    /// Every voice's clip and gain, in the order they were started.
    pub fn clips(&self) -> Vec<(Arc<Clip>, f32)> {
        self.voices.iter().map(|v| (v.clip.clone(), v.play.gain)).collect()
    }

    /// A voice's left and right gains for the listeners as they stand: the nearest listener's distance
    /// and side, and no side at all when more than one is listening (split screen has no one left or
    /// right).
    fn gains(&self, p: &Play) -> [f32; 2] {
        let g = p.gain * self.bus_volume(p.bus);
        let Some(at) = p.at else { return [g, g] };
        let mut best = (f32::MAX, 0.0);
        for l in &self.listeners {
            let d = [at[0] - l.at[0], at[1] - l.at[1], at[2] - l.at[2]];
            let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if dist < best.0 {
                let side = (d[0] * l.right[0] + d[1] * l.right[1] + d[2] * l.right[2]) / dist.max(1e-3);
                best = (dist, side);
            }
        }
        let fade = falloff(best.0, p.near, p.far);
        let pan = if self.listeners.len() == 1 { best.1 * 0.6 } else { 0.0 };
        [g * fade * (1.0 - pan).min(1.0), g * fade * (1.0 + pan).min(1.0)]
    }

    /// A bus's slider.
    pub fn bus_volume(&self, bus: Bus) -> f32 {
        match bus {
            Bus::Effects => self.effects_volume,
            Bus::Engine => self.engine_volume,
            Bus::Crowd => self.crowd_volume,
            Bus::Menu => self.menu_volume,
        }
    }

    /// The gain the song playing is mixed at: the folder's slider for the user's own files, the
    /// music's for the rest — the original's split, where the songs past the disc's thirteen take the
    /// ripped music's volume and the rest the music's.
    pub fn music_gain(&self) -> f32 {
        let base = match self.music.now() {
            Some(id) if id.starts_with(FOLDER) => self.folder_volume,
            _ => self.music_volume,
        };
        match self.music_edge {
            Some(f) => ((((base * 100.0).round() as i32) as f32 * f) as i32).max(0) as f32 / 100.0,
            None => base,
        }
    }

    /// The fade between the songs and the menu's loop (P65): 0 the songs alone, 1 the loop alone.
    pub fn menu_fade(&self) -> f32 {
        self.menu_fade
    }

    /// The songs, at `gain`, and the menu's loop, faded one into the other as [`Mixer::front_end`]
    /// asks. A song under a full fade holds where it is, for the next ride.
    fn mix_menu_music(&mut self, acc: &mut [f32], gain: f32) {
        let frames = acc.len() / 2;
        let want = if self.front_end && self.menu_music.is_some() { 1.0 } else { 0.0 };
        let from = self.menu_fade;
        let step = frames as f32 / (MENU_FADE * RATE as f32);
        let to = if want > from { (from + step).min(want) } else { (from - step).max(want) };
        self.menu_fade = to;
        let at = |f: usize| from + (to - from) * f as f32 / frames.max(1) as f32;
        if from <= 0.0 && to <= 0.0 {
            // The ride's songs alone, as before the menu had music.
            self.music.mix(acc, gain);
            self.menu_pos = 0.0;
            return;
        }
        if from >= 1.0 && to >= 1.0 {
            self.music.mix(acc, 0.0);
        } else {
            let mut songs = vec![0f32; acc.len()];
            self.music.mix(&mut songs, gain);
            for f in 0..frames {
                let g = 1.0 - at(f);
                acc[f * 2] += songs[f * 2] * g;
                acc[f * 2 + 1] += songs[f * 2 + 1] * g;
            }
        }
        let Some(clip) = self.menu_music.clone() else { return };
        let n = clip.frames();
        if n < 2 {
            return;
        }
        let ch = clip.channels.max(1) as usize;
        let step = clip.rate as f64 / RATE as f64;
        let back = if (MENU_LOOP_FROM as usize) < n - 1 { MENU_LOOP_FROM } else { 0.0 };
        for f in 0..frames {
            let g = self.music_volume * at(f);
            let i = self.menu_pos as usize;
            let t = (self.menu_pos - i as f64) as f32;
            let j = if i + 1 < n { i + 1 } else { back as usize };
            for c in 0..2 {
                let src = c.min(ch - 1);
                let a = clip.pcm[i * ch + src] as f32;
                let b = clip.pcm[j * ch + src] as f32;
                acc[f * 2 + c] += (a + (b - a) * t) * g;
            }
            self.menu_pos += step;
            if self.menu_pos >= n as f64 {
                self.menu_pos -= n as f64 - back;
            }
        }
    }

    /// Mix `out.len() / 2` stereo frames.
    pub fn render(&mut self, out: &mut [i16]) {
        let frames = out.len() / 2;
        let mut acc = vec![0f32; frames * 2];
        // The song can change inside the mix; it is mixed at the gain of the one playing as it starts.
        self.music.tend();
        let gain = self.music_gain();
        self.mix_menu_music(&mut acc, gain);
        // About 5 ms to reach a new gain: no clicks, and a stop is gone well inside a frame.
        let ease = 1.0 - (-1.0 / (0.005 * RATE as f32)).exp();
        let targets: Vec<[f32; 2]> = self
            .voices
            .iter()
            .map(|v| if v.stopping { [0.0; 2] } else { self.gains(&v.play) })
            .collect();
        for (v, target) in self.voices.iter_mut().zip(targets) {
            if (v.paused || v.held) && !v.stopping {
                continue;
            }
            if v.applied[0] < 0.0 {
                v.applied = target;
            }
            for f in 0..frames {
                let n = v.clip.frames();
                let end = match &v.then {
                    Some((_, _, Some(at))) => at.min(n as f64),
                    _ => n as f64,
                };
                if v.pos >= end - 1.0 || n < 2 {
                    if let Some((clip, looping, _)) = v.then.take() {
                        v.clip = clip;
                        v.play.looping = looping;
                        v.pos = 0.0;
                        v.segment += 1;
                    } else if v.play.looping && n >= 2 {
                        v.pos -= (n - 1) as f64;
                    }
                    if v.clip.frames() < 2 || v.pos >= v.clip.frames() as f64 - 1.0 {
                        v.stopping = true;
                        v.applied = [0.0; 2];
                        break;
                    }
                }
                let ch = v.clip.channels.max(1) as usize;
                let step = v.clip.rate as f64 / RATE as f64 * v.play.pitch.max(0.01) as f64;
                let i = v.pos as usize;
                let t = (v.pos - i as f64) as f32;
                for c in 0..2 {
                    let src = c.min(ch - 1);
                    let a = v.clip.pcm[i * ch + src] as f32;
                    let b = v.clip.pcm[(i + 1) * ch + src] as f32;
                    v.applied[c] += (target[c] - v.applied[c]) * ease;
                    acc[f * 2 + c] += (a + (b - a) * t) * v.applied[c];
                }
                v.pos += step;
            }
        }
        self.voices.retain(|v| !(v.stopping && v.applied[0] < 1e-3 && v.applied[1] < 1e-3));
        for (o, a) in out.iter_mut().zip(acc) {
            *o = a.clamp(-32768.0, 32767.0) as i16;
        }
        self.rendered += frames as u64;
    }
}

/// Full volume within `near`, silent beyond `far`, and a straight line between (the `.aud`s give the
/// two distances; the curve between them is this port's).
pub fn falloff(dist: f32, near: f32, far: f32) -> f32 {
    if dist <= near {
        1.0
    } else if dist >= far || far <= near {
        0.0
    } else {
        1.0 - (dist - near) / (far - near)
    }
}

// --- music ------------------------------------------------------------------------------------------

/// A song as a source lists it. `id` is `source:name`, which is what the settings keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
}

impl Track {
    /// What the menu shows.
    pub fn label(&self) -> String {
        if self.artist.is_empty() { self.title.clone() } else { format!("{} - {}", self.title, self.artist) }
    }
}

/// A song, playing: stereo frames at its own rate.
pub trait Stream: Send {
    fn rate(&self) -> u32;
    /// Up to `out.len() / 2` frames into `out`, interleaved; 0 at the end.
    fn read(&mut self, out: &mut [i16]) -> usize;
}

/// **Somewhere songs come from.** The disc's tracks first, the user's own folder second; another
/// source is one more of these handed to [`Music::new`].
pub trait MusicSource: Send + Sync {
    /// The prefix its tracks' ids carry.
    fn name(&self) -> &str;
    fn tracks(&self) -> Vec<Track>;
    /// Start a song by its id. May read a disc or a file, so the player calls it off the mixer.
    fn open(&self, id: &str) -> Result<Box<dyn Stream>, String>;
}

/// A whole song in memory, decoded block by block as it plays.
pub struct Adpcm {
    data: Vec<u8>,
    at: usize,
    rate: u32,
    prev: [[i32; 2]; 2],
    held: Vec<i16>,
    held_at: usize,
}

impl Adpcm {
    /// A `.vig` file's bytes.
    pub fn vig(bytes: Vec<u8>) -> Adpcm {
        let size = if bytes.len() >= 0x40 { be32(&bytes, 0x0c) as usize } else { 0 };
        let rate = if bytes.len() >= 0x40 { be32(&bytes, 0x10) } else { 44100 };
        let end = (0x800 + 2 * size).min(bytes.len());
        let data = bytes.get(0x800..end).unwrap_or(&[]).to_vec();
        Adpcm { data, at: 0, rate: if (4000..=96000).contains(&rate) { rate } else { 44100 }, prev: [[0; 2]; 2], held: Vec::new(), held_at: 0 }
    }
}

impl Stream for Adpcm {
    fn rate(&self) -> u32 {
        self.rate
    }

    fn read(&mut self, out: &mut [i16]) -> usize {
        let mut n = 0;
        while n * 2 + 1 < out.len() {
            if self.held_at >= self.held.len() {
                let Some(pair) = self.data.get(self.at..self.at + 32) else { break };
                self.at += 32;
                self.held.clear();
                self.held_at = 0;
                let l = decode_block(&pair[..16], &mut self.prev[0]);
                let r = decode_block(&pair[16..], &mut self.prev[1]);
                for i in 0..28 {
                    self.held.push(l[i]);
                    self.held.push(r[i]);
                }
            }
            out[n * 2] = self.held[self.held_at];
            out[n * 2 + 1] = self.held[self.held_at + 1];
            self.held_at += 2;
            n += 1;
        }
        n
    }
}

/// A decoded clip as a stream: a WAV off the user's folder.
pub struct Pcm {
    clip: Clip,
    at: usize,
}

impl Stream for Pcm {
    fn rate(&self) -> u32 {
        self.clip.rate
    }

    fn read(&mut self, out: &mut [i16]) -> usize {
        let ch = self.clip.channels.max(1) as usize;
        let mut n = 0;
        while n * 2 + 1 < out.len() && self.at < self.clip.frames() {
            let l = self.clip.pcm[self.at * ch];
            let r = self.clip.pcm[self.at * ch + (ch - 1).min(1)];
            out[n * 2] = l;
            out[n * 2 + 1] = r;
            self.at += 1;
            n += 1;
        }
        n
    }
}

/// **The disc's songs**: `music.pb`'s list, each song's `.vig` read out of the sound archive when it
/// starts. A file too short to be a song is left off (P42 found College Kids one; it was the archive's
/// file before it, P49).
pub struct DiscMusic {
    image: PathBuf,
    bank: Bank,
    songs: Vec<(Track, BankEntry)>,
}

/// Shorter than this is not a song.
const SHORTEST_SONG: u64 = 64 << 10;

impl DiscMusic {
    /// The list off an opened disc; `image` is where the disc is, opened again for each song so the
    /// source can live on another thread.
    pub fn read(disc: &mut Disc, image: PathBuf) -> Result<DiscMusic, String> {
        let pb = disc.res("appstart.res")?.read("aud\\music\\music.pb")?;
        let bank = Bank::parse(&disc.member_part(BANK, 0, 64 << 10)?)?;
        let songs = DiscMusic::list(&ini::text(&pb), &bank);
        Ok(DiscMusic { image, bank, songs })
    }

    /// `music.pb`: `[Artists] ArtistN="…"`, then `[Song N]` sections of `Filename`, `Title` and an
    /// artist number. Songs whose file is missing or too short to be a song are left off.
    pub fn list(pb: &str, bank: &Bank) -> Vec<(Track, BankEntry)> {
        let sections = ini::parse(pb);
        let unquote = |s: &str| s.trim().trim_matches('"').to_string();
        let artists = section(&sections, "Artists");
        let mut out = Vec::new();
        for s in sections.iter().filter(|s| s.name.to_ascii_lowercase().starts_with("song ")) {
            let Some(file) = s.get("Filename") else { continue };
            let Some(e) = bank.find(&format!("{file}.vig")) else { continue };
            if e.size < SHORTEST_SONG {
                continue;
            }
            let stem = file.rsplit('\\').next().unwrap_or(file).to_ascii_lowercase();
            let artist = s
                .get("Artist")
                .and_then(|n| artists?.get(&format!("Artist{}", n.trim())))
                .map(unquote)
                .unwrap_or_default();
            let title = s.get("Title").map(unquote).unwrap_or_else(|| stem.clone());
            out.push((Track { id: format!("disc:{stem}"), title, artist }, e.clone()));
        }
        out
    }
}

impl MusicSource for DiscMusic {
    fn name(&self) -> &str {
        "disc"
    }

    fn tracks(&self) -> Vec<Track> {
        self.songs.iter().map(|(t, _)| t.clone()).collect()
    }

    fn open(&self, id: &str) -> Result<Box<dyn Stream>, String> {
        let (_, e) = self.songs.iter().find(|(t, _)| t.id == id).ok_or_else(|| format!("no song {id}"))?;
        let _ = &self.bank;
        let mut disc = Disc::open(&self.image)?;
        let bytes = disc.member_part(BANK, e.at, e.size as usize)?;
        Ok(Box::new(Adpcm::vig(bytes)))
    }
}

/// **The user's own songs**: every 16-bit PCM `.wav` in one folder, by file name.
pub struct FolderMusic {
    dir: PathBuf,
    files: Vec<(Track, PathBuf)>,
}

impl FolderMusic {
    pub fn scan(dir: &Path) -> FolderMusic {
        let mut files = Vec::new();
        if let Ok(read) = std::fs::read_dir(dir) {
            for e in read.flatten() {
                let path = e.path();
                let is_wav = path.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav"));
                if !is_wav {
                    continue;
                }
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let title = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                files.push((Track { id: format!("folder:{name}"), title, artist: String::new() }, path));
            }
        }
        files.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        FolderMusic { dir: dir.to_path_buf(), files }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl MusicSource for FolderMusic {
    fn name(&self) -> &str {
        "folder"
    }

    fn tracks(&self) -> Vec<Track> {
        self.files.iter().map(|(t, _)| t.clone()).collect()
    }

    fn open(&self, id: &str) -> Result<Box<dyn Stream>, String> {
        let (_, path) = self.files.iter().find(|(t, _)| t.id == id).ok_or_else(|| format!("no song {id}"))?;
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let clip = wav(&bytes).ok_or_else(|| format!("{}: not a 16-bit PCM WAV", path.display()))?;
        Ok(Box::new(Pcm { clip, at: 0 }))
    }
}

/// A RIFF WAV's 16-bit PCM (opentennis' `hst-base/src/audio.rs` `pcm_frames`, which this widens to
/// keep the rate and the channel count).
pub fn wav(b: &[u8]) -> Option<Clip> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let (mut at, mut fmt) = (12, None);
    while at + 8 <= b.len() {
        let len = le32(b, at + 4)? as usize;
        let body = b.get(at + 8..(at + 8 + len).min(b.len()))?;
        match &b[at..at + 4] {
            b"fmt " if body.len() >= 16 => {
                let format = u16::from_le_bytes([body[0], body[1]]);
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let rate = le32(body, 4)?;
                let bits = u16::from_le_bytes([body[14], body[15]]);
                if format != 1 || bits != 16 || !(1..=2).contains(&channels) {
                    return None;
                }
                fmt = Some((channels as u8, rate));
            }
            b"data" => {
                let (channels, rate) = fmt?;
                let pcm = body.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]])).collect();
                return Some(Clip { rate, channels, pcm });
            }
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    None
}

/// The folder source's ids' prefix.
const FOLDER: &str = "folder:";

/// What the settings ask the music to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Off,
    Shuffle,
    /// Every song in the sources' order, from the one after the last (P56): the original's play
    /// list, which runs through `music.pb`'s songs in turn.
    Order,
    /// One song, over and over.
    Track(String),
}

impl Pick {
    /// The settings' word: `off`, `shuffle`, or a track's id.
    pub fn parse(word: &str) -> Pick {
        match word.trim() {
            "off" => Pick::Off,
            "" | "shuffle" => Pick::Shuffle,
            "order" => Pick::Order,
            id => Pick::Track(id.to_string()),
        }
    }

    pub fn word(&self) -> String {
        match self {
            Pick::Off => "off".to_string(),
            Pick::Shuffle => "shuffle".to_string(),
            Pick::Order => "order".to_string(),
            Pick::Track(id) => id.clone(),
        }
    }
}

/// How many songs back the pause's back control can go (P174).
pub const HISTORY: usize = 20;

/// **The music player**: every source's songs, the pick, and the song playing, resampled to the mix.
pub struct Music {
    sources: Vec<Arc<dyn MusicSource>>,
    pick: Pick,
    dice: Dice,
    last: Option<String>,
    /// A song asked for by name (P56's pause), which plays next whatever the pick would choose.
    asked: Option<String>,
    /// The songs started, the newest (the one playing) last, at most [`HISTORY`] (P174): what the
    /// pause's back control goes back through.
    played: Vec<String>,
    /// The song [`Music::asked`] holds is one gone back to, which is not played again on top.
    going_back: bool,
    /// The pause's player has the music held (P174): the song stays where it is, silent.
    held: bool,
    playing: Option<(String, Box<dyn Stream>)>,
    opening: Option<(String, Receiver<Result<Box<dyn Stream>, String>>)>,
    /// Open songs on the mixer's own thread (tests) rather than a helper's.
    pub wait: bool,
    /// The song's frames around the read head, and where between them it is.
    buf: Vec<i16>,
    buf_frames: usize,
    pos: f64,
}

impl Music {
    pub fn new(sources: Vec<Arc<dyn MusicSource>>, seed: u64) -> Music {
        Music {
            sources,
            pick: Pick::Off,
            dice: Dice::new(seed),
            last: None,
            asked: None,
            played: Vec::new(),
            going_back: false,
            held: false,
            playing: None,
            opening: None,
            wait: false,
            buf: Vec::new(),
            buf_frames: 0,
            pos: 0.0,
        }
    }

    /// Every song every source has, sources in order.
    pub fn tracks(&self) -> Vec<Track> {
        self.sources.iter().flat_map(|s| s.tracks()).collect()
    }

    pub fn sources(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.name().to_string()).collect()
    }

    pub fn pick(&self) -> &Pick {
        &self.pick
    }

    /// The song playing (or being opened), by id.
    pub fn now(&self) -> Option<&str> {
        self.playing
            .as_ref()
            .map(|(id, _)| id.as_str())
            .or(self.opening.as_ref().map(|(id, _)| id.as_str()))
    }

    /// Change what plays. The same pick again changes nothing; a different one stops the song.
    pub fn set_pick(&mut self, pick: Pick) {
        if pick == self.pick {
            return;
        }
        let keep = matches!((&pick, self.now()), (Pick::Shuffle | Pick::Order, Some(_)) if self.pick != Pick::Off);
        self.pick = pick;
        if !keep {
            self.playing = None;
            self.opening = None;
        }
    }

    /// Skip to the next song.
    pub fn next(&mut self) {
        self.playing = None;
        self.opening = None;
    }

    /// Back to the song before the one playing (P174), and on from it as the pick says; with none
    /// before it, the one playing from its start. Whether there was a song to go to.
    pub fn back(&mut self) -> bool {
        if self.played.len() >= 2 {
            self.played.pop();
        }
        let Some(id) = self.played.last().cloned() else { return false };
        self.asked = Some(id);
        self.going_back = true;
        self.next();
        true
    }

    /// The songs started, oldest first, the one playing last (P174).
    #[allow(dead_code)]
    pub fn played(&self) -> &[String] {
        &self.played
    }

    /// Hold the music where it is, or let it go on (P174's pause/play).
    pub fn hold(&mut self, on: bool) {
        self.held = on;
    }

    pub fn held(&self) -> bool {
        self.held
    }

    /// Play `id` now, and then go on as the pick says (P56): under Shuffle or In Order the song after
    /// it is the pick's, and under Off nothing follows it.
    pub fn play_now(&mut self, id: &str) {
        self.asked = Some(id.to_string());
        self.next();
    }

    fn choose(&mut self) -> Option<String> {
        if let Some(id) = self.asked.take() {
            return Some(id);
        }
        match &self.pick {
            Pick::Off => None,
            Pick::Track(id) => Some(id.clone()),
            Pick::Order => {
                let all = self.tracks();
                let after = self.last.as_deref().and_then(|l| all.iter().position(|t| t.id == l));
                let i = after.map_or(0, |i| (i + 1) % all.len().max(1));
                all.get(i).map(|t| t.id.clone())
            }
            Pick::Shuffle => {
                let all = self.tracks();
                if all.is_empty() {
                    return None;
                }
                let mut i = self.dice.below(all.len() as u32) as usize;
                if all.len() > 1 && self.last.as_deref() == Some(all[i].id.as_str()) {
                    i = (i + 1) % all.len();
                }
                Some(all[i].id.clone())
            }
        }
    }

    fn source(&self, id: &str) -> Option<Arc<dyn MusicSource>> {
        let prefix = id.split(':').next()?;
        self.sources.iter().find(|s| s.name() == prefix).cloned()
    }

    /// Start the next song if none is playing, and take one that has finished opening.
    pub fn tend(&mut self) {
        if let Some((id, rx)) = self.opening.take() {
            match rx.try_recv() {
                Ok(Ok(stream)) => {
                    self.playing = Some((id, stream));
                    self.buf_frames = 0;
                    self.pos = 0.0;
                }
                Ok(Err(e)) => {
                    println!("music: {e}");
                    // A song that will not open is not asked for again until the pick changes.
                    if matches!(self.pick, Pick::Track(_)) {
                        self.pick = Pick::Off;
                    }
                }
                Err(TryRecvError::Empty) => self.opening = Some((id, rx)),
                Err(TryRecvError::Disconnected) => {}
            }
            return;
        }
        if self.playing.is_some() {
            return;
        }
        let Some(id) = self.choose() else { return };
        let Some(source) = self.source(&id) else {
            println!("music: no source for {id}");
            self.pick = Pick::Off;
            return;
        };
        self.last = Some(id.clone());
        if !std::mem::take(&mut self.going_back) {
            self.played.push(id.clone());
            if self.played.len() > HISTORY {
                self.played.remove(0);
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let want = id.clone();
        if self.wait {
            let _ = tx.send(source.open(&want));
        } else {
            crate::thread::spawn(move || {
                let _ = tx.send(source.open(&want));
            });
        }
        self.opening = Some((id, rx));
        if self.wait {
            self.tend();
        }
    }

    /// Add the music into a mix of `acc.len() / 2` frames.
    pub fn mix(&mut self, acc: &mut [f32], volume: f32) {
        self.tend();
        if volume <= 0.0 || self.held {
            return;
        }
        let Some((_, stream)) = self.playing.as_mut() else { return };
        let step = stream.rate() as f64 / RATE as f64;
        let mut ended = false;
        for f in 0..acc.len() / 2 {
            // Keep two frames ahead of the read head.
            while self.pos as usize + 1 >= self.buf_frames {
                let keep = if self.buf_frames > 0 { 1 } else { 0 };
                if keep == 1 {
                    let last = [self.buf[(self.buf_frames - 1) * 2], self.buf[(self.buf_frames - 1) * 2 + 1]];
                    self.pos -= (self.buf_frames - 1) as f64;
                    self.buf.clear();
                    self.buf.extend_from_slice(&last);
                } else {
                    self.buf.clear();
                }
                let mut more = [0i16; 2048];
                let n = stream.read(&mut more);
                if n == 0 {
                    ended = true;
                    break;
                }
                self.buf.extend_from_slice(&more[..n * 2]);
                self.buf_frames = keep + n;
            }
            if ended {
                break;
            }
            let i = self.pos as usize;
            let t = (self.pos - i as f64) as f32;
            for c in 0..2 {
                let a = self.buf[i * 2 + c] as f32;
                let b = self.buf[(i + 1) * 2 + c] as f32;
                acc[f * 2 + c] += (a + (b - a) * t) * volume;
            }
            self.pos += step;
        }
        if ended {
            // The next song (or the same one again) starts on the next mix, or now when it opens here.
            self.playing = None;
            self.buf_frames = 0;
            self.pos = 0.0;
            self.tend();
        }
    }
}

// --- the bike's engine ------------------------------------------------------------------------------

/// The engine note's states, numbered as the original's bike sound numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Note {
    /// Nothing chosen: a chain that ran out with nowhere to go.
    Off = 0,
    Idle = 1,
    LowAccel = 2,
    HighAccel = 3,
    Power = 4,
    Sputter = 5,
    Decel = 6,
    Clutch = 7,
    /// The free rev, `[Panic]`'s first sample, up to `RevMidpoint`.
    Rev = 8,
    /// Held at the limiter: the rev sample paused at its midpoint, `[Panic]`'s loop playing over it.
    Panic = 9,
    /// Off the gas after the limiter: the rev sample resumed, falling back to the idle.
    Unwind = 10,
}

impl Note {
    /// The four states a throttle held on the ground plays.
    fn pulling(self) -> bool {
        matches!(self, Note::LowAccel | Note::HighAccel | Note::Power | Note::Sputter)
    }
}

/// One bike's `vehicle_*.aud`.
#[derive(Clone, Debug, Default)]
pub struct BikeSounds {
    pub idle: List,
    pub low: List,
    pub high: List,
    pub power: List,
    pub sputter: List,
    pub decel: List,
    pub clutch: List,
    pub panic: List,
    /// `[Wreck]`, the clank (P144): read by the same loader as a machine's `[<prefix>Wreck]`, with no
    /// prefix.
    pub wreck: MachineWreck,
    /// `LowTorqueSpeed` in feet a second (the file's mph, ×1.46666 as the loader does).
    pub low_torque_speed: f32,
    pub airborne_to_decel: f32,
    pub rev_midpoint: f32,
    /// `OverlapTime`: how long before a sample's end the next one starts under it.
    pub overlap: f32,
    /// `PitchVariance`: the most a sample's pitch is moved either way, in the sound chip's pitch units
    /// (4096 is a sample played at 48 kHz).
    pub pitch_variance: i32,
    /// `DopplerFactor`.
    pub doppler: f32,
    pub near: f32,
    pub far: f32,
}

impl BikeSounds {
    pub fn read(text: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> BikeSounds {
        let s = ini::parse(text);
        let params = section(&s, "Params");
        let p = |k: &str, d: f32| params.and_then(|p| p.number(k)).unwrap_or(d);
        BikeSounds {
            idle: List::read(section(&s, "Idle"), lib),
            low: List::read(section(&s, "LowAccel"), lib),
            high: List::read(section(&s, "HighAccel"), lib),
            power: List::read(section(&s, "Power"), lib),
            sputter: List::read(section(&s, "Sputter"), lib),
            decel: List::read(section(&s, "Decel"), lib),
            clutch: List::read(section(&s, "Clutch"), lib),
            panic: List::read(section(&s, "Panic"), lib),
            wreck: MachineWreck::read(text, "", lib),
            low_torque_speed: p("LowTorqueSpeed", 25.0) * 1.46666,
            airborne_to_decel: p("AirborneTimeToDecel", 0.3),
            rev_midpoint: p("RevMidpoint", 0.311),
            overlap: p("OverlapTime", 0.008),
            pitch_variance: p("PitchVariance", 0.0) as i32,
            doppler: p("DopplerFactor", 0.25),
            near: p("MinAttenuationDistance", 20.0),
            far: p("MaxAttenuationDistance", 100.0),
        }
    }

    /// The list a sample is from. `Note::Rev` is `[Panic]`, whose first sample is the free rev and
    /// whose second is the limiter's loop.
    pub fn list(&self, n: Note) -> &List {
        match n {
            Note::Idle => &self.idle,
            Note::LowAccel => &self.low,
            Note::HighAccel => &self.high,
            Note::Power => &self.power,
            Note::Sputter => &self.sputter,
            Note::Decel => &self.decel,
            Note::Clutch => &self.clutch,
            Note::Rev | Note::Panic | Note::Unwind | Note::Off => &self.panic,
        }
    }

    pub fn clip(&self, s: Sample) -> Option<&(Arc<Clip>, bool)> {
        self.list(s.list).sounds.get(s.index)
    }

    /// A sample's own looping flag, and its length in seconds (0 for one the disc lacks).
    pub fn looping(&self, s: Sample) -> bool {
        self.clip(s).is_some_and(|c| c.1)
    }

    pub fn seconds(&self, s: Sample) -> f32 {
        self.clip(s).map_or(0.0, |c| c.0.seconds())
    }
}

/// What the engine note hears from the ride each frame: the original's four (on the ground, the
/// throttle, the speed, the clutch), and where the bike is and how fast it is going for the mix.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hands {
    pub grounded: bool,
    pub gas: f32,
    pub clutch: f32,
    /// Feet a second.
    pub speed: f32,
    pub at: [f32; 3],
    /// Feet a second, the world's axes.
    pub vel: [f32; 3],
}

/// One of a bike's engine samples: the list it is in (as [`BikeSounds::list`] names them) and its
/// place there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub list: Note,
    pub index: usize,
}

impl Sample {
    const IDLE: Sample = Sample { list: Note::Idle, index: 0 };
    const REV: Sample = Sample { list: Note::Rev, index: 0 };
    /// The limiter's loop.
    pub const PANIC: Sample = Sample { list: Note::Rev, index: 1 };
}

/// A source of the original's `rand()`: `0..=0x7fff_ffff`.
pub trait Draw {
    fn rand(&mut self) -> i32;
}

impl Draw for Dice {
    fn rand(&mut self) -> i32 {
        (self.next() >> 33) as i32
    }
}

/// `rand()` as the original scales it, `[0, 1)`.
fn unit(d: &mut impl Draw) -> F {
    F(1.0 / 2_147_483_648.0) * F::from(d.rand())
}

/// **The no-repeat pick** the engine's lists use: a list of one or none answers its first sample and
/// draws nothing; a longer one draws an index, moved on by one (and wrapped) when it is the last pick.
pub fn pick(count: i32, last: &mut i32, d: &mut impl Draw) -> i32 {
    if count < 2 {
        *last = 0;
        return 0;
    }
    let mut i = (F::from(count) * unit(d)).to_i32();
    if i == *last {
        i = (i + 1) % count;
    }
    *last = i;
    i
}

/// **A sample's pitch variance**: a uniform offset in `-variance..variance` pitch units, from one draw.
pub fn jitter(variance: i32, d: &mut impl Draw) -> i32 {
    let r = d.rand();
    (F::from(variance << 1) * F::from(r) * F(1.0 / 2_147_483_648.0)).to_i32() - variance
}

/// What the engine note plays on: the original's emitter and its sounds, the mixer in the shell, the
/// original itself in the test.
pub trait Bed {
    /// The sample is sounding (a paused one is still sounding).
    fn playing(&self, s: Sample) -> bool;
    /// Seconds into the sample's voice, or -1 when it has none.
    fn position(&self, s: Sample) -> f32;
    /// Stop the sample.
    fn stop(&mut self, s: Sample);
    /// Start the sample where the bike is: false, and nothing started, when no one is near enough to
    /// hear it or the bike's sound is gated off.
    fn play(&mut self, s: Sample) -> bool;
    /// Move a sample just started off its own pitch, in pitch units.
    fn detune(&mut self, s: Sample, by: i32);
    /// The limiter: pause the free rev and start `[Panic]`'s loop over it, or stop the loop and let
    /// the rev go on.
    fn hold(&mut self, on: bool);
}

/// **The engine note's state machine**, the original's: the frame's decision from the hands, else the
/// chain to the next sample once the one sounding is about to end. One sample is the note's at a time
/// (`current`); one that is not cut plays out under the next.
#[derive(Clone, Debug, PartialEq)]
pub struct Brain {
    pub note: Note,
    /// The state the chain goes to when the current sample ends.
    pub next: Note,
    /// The sample that is the note's: the emitter's queue.
    pub current: Sample,
    /// Counts down off the throttle from a quarter second; with `hold_off_gas` the accelerations hold
    /// until it runs out.
    pub off_gas: f32,
    /// Counts down in the air from `AirborneTimeToDecel`.
    pub air: f32,
    pub hold_off_gas: bool,
    /// A throttle in the air revs the engine; without it the air is the throttle let go.
    pub air_revs: bool,
    /// The throttle was held last frame.
    pub was_gas: bool,
    /// The limiter's loop is playing.
    pub held: bool,
    pub muted: bool,
    /// The last picks: low, high, power, sputter, clutch.
    pub last: [i32; 5],
}

/// How long the throttle is remembered after it is let go.
const OFF_GAS_HOLD: f32 = 0.25;

impl Brain {
    /// The player's note as the loader leaves it: idle queued and not yet started, nothing to chain to,
    /// nothing picked, the throttle in the air revving the engine.
    pub fn new() -> Brain {
        Brain {
            note: Note::Idle,
            next: Note::Off,
            current: Sample::IDLE,
            off_gas: 0.0,
            air: 0.0,
            hold_off_gas: false,
            air_revs: true,
            was_gas: false,
            held: false,
            muted: false,
            last: [-1; 5],
        }
    }

    /// One frame.
    pub fn frame(&mut self, h: &Hands, dt: f32, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if !self.muted && !self.decide(h, dt, sounds, bed, d) {
            self.chain(h, sounds, bed, d);
        }
    }

    /// **The frame's decision**: true when it started a note (and the chain waits a frame).
    pub fn decide(&mut self, h: &Hands, dt: f32, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) -> bool {
        let gas = 0.1 < h.gas;
        let mut started = false;
        if !h.grounded {
            self.air = (F(self.air) - F(dt)).0;
            if gas && self.air_revs {
                if !self.was_gas {
                    self.air = 0.0;
                    self.rev(Note::Rev, h, sounds, bed, d);
                    started = true;
                } else if self.air <= 0.0 && self.note.pulling() {
                    self.air = 0.0;
                    self.decel(bed, d, sounds);
                    started = true;
                }
                self.was_gas = true;
                return started;
            }
            if self.note == Note::Panic {
                self.rev(self.next, h, sounds, bed, d);
            } else if self.note.pulling() {
                self.decel(bed, d, sounds);
            } else {
                self.was_gas = false;
                return false;
            }
            self.was_gas = false;
            return true;
        }
        if 0.1 < h.clutch {
            if gas {
                if !self.was_gas {
                    self.rev(Note::Rev, h, sounds, bed, d);
                    started = true;
                } else if 1.0 < h.speed && self.note < Note::Clutch {
                    self.rev(Note::Clutch, h, sounds, bed, d);
                    started = true;
                }
                self.was_gas = true;
            } else {
                if self.note.pulling() {
                    self.decel(bed, d, sounds);
                    started = true;
                } else if self.note == Note::Clutch {
                    self.next = Note::Idle;
                } else if self.note == Note::Panic {
                    self.rev(self.next, h, sounds, bed, d);
                    started = true;
                }
                self.was_gas = false;
            }
        } else if gas {
            self.off_gas = OFF_GAS_HOLD;
            if matches!(self.note, Note::Unwind | Note::Panic | Note::Rev | Note::Idle | Note::Decel | Note::Off) {
                self.accel(h, sounds, bed, d);
                started = true;
            } else if self.note == Note::Clutch {
                self.power(sounds, bed, d);
                started = true;
            }
            self.was_gas = true;
        } else {
            self.off_gas = (F(self.off_gas) - F(dt)).0;
            if !self.hold_off_gas || self.off_gas <= 0.0 {
                if self.note == Note::Panic {
                    self.rev(self.next, h, sounds, bed, d);
                    started = true;
                } else if self.note.pulling() {
                    self.decel(bed, d, sounds);
                    started = true;
                }
            }
            self.was_gas = false;
        }
        self.air = sounds.airborne_to_decel;
        started
    }

    /// **The chain**: once the current sample is within `OverlapTime` of its end (or has ended, or never
    /// started), the next state's sample; the free rev hands on at `RevMidpoint` instead. A looping
    /// sample never ends.
    pub fn chain(&mut self, h: &Hands, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        let cur = self.current;
        let at_midpoint = self.note == Note::Rev && sounds.rev_midpoint < bed.position(cur);
        if !at_midpoint {
            if sounds.looping(cur) {
                return;
            }
            if bed.playing(cur) && (F(sounds.overlap) + F(bed.position(cur))).0 < sounds.seconds(cur) {
                return;
            }
        }
        match self.next {
            Note::Unwind => {}
            Note::Panic | Note::Rev => self.rev(self.next, h, sounds, bed, d),
            Note::Sputter => self.sputter(sounds, bed, d),
            Note::Power => self.power(sounds, bed, d),
            Note::Idle => self.idle(sounds, bed, d),
            Note::Off => self.note = Note::Off,
            _ => {}
        }
    }

    fn limiter(&mut self, on: bool, bed: &mut impl Bed) {
        if self.held != on {
            self.held = on;
            bed.hold(on);
        }
    }

    /// Make `s` the note's sample and start it. `cut` stops the one sounding first; `unless_playing`
    /// leaves a sample that is still sounding alone. The idle is never detuned.
    fn sound(&mut self, s: Sample, cut: bool, unless_playing: bool, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if self.muted {
            return;
        }
        if self.held {
            self.limiter(false, bed);
        }
        if cut {
            bed.stop(self.current);
        }
        self.current = s;
        if unless_playing && bed.playing(s) {
            return;
        }
        if bed.play(s) && s != Sample::IDLE {
            let by = jitter(sounds.pitch_variance, d);
            bed.detune(s, by);
        }
    }

    fn picked(&mut self, list: Note, slot: usize, sounds: &BikeSounds, d: &mut impl Draw) -> Sample {
        let n = sounds.list(list).sounds.len() as i32;
        Sample { list, index: pick(n, &mut self.last[slot], d) as usize }
    }

    fn idle(&mut self, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if self.note != Note::Idle {
            self.note = Note::Idle;
            self.sound(Sample::IDLE, false, true, sounds, bed, d);
            self.next = Note::Idle;
        }
    }

    /// The throttle opened: the low or high acceleration by the speed, cut straight in from the idle,
    /// the deceleration or the rev.
    fn accel(&mut self, h: &Hands, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if matches!(self.note, Note::Unwind | Note::Panic | Note::Rev) {
            let s = self.picked(Note::HighAccel, 1, sounds, d);
            self.note = Note::HighAccel;
            self.sound(s, true, false, sounds, bed, d);
        } else {
            let cut = matches!(self.note, Note::Decel | Note::Idle);
            let s = if h.speed <= sounds.low_torque_speed {
                self.note = Note::LowAccel;
                self.picked(Note::LowAccel, 0, sounds, d)
            } else {
                self.note = Note::HighAccel;
                self.picked(Note::HighAccel, 1, sounds, d)
            };
            self.sound(s, cut, !cut, sounds, bed, d);
        }
        self.next = Note::Power;
    }

    fn power(&mut self, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        let s = self.picked(Note::Power, 2, sounds, d);
        self.note = Note::Power;
        self.sound(s, false, true, sounds, bed, d);
        self.next = Note::Sputter;
    }

    fn sputter(&mut self, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        let s = self.picked(Note::Sputter, 3, sounds, d);
        self.note = Note::Sputter;
        self.sound(s, false, true, sounds, bed, d);
        self.next = Note::Power;
    }

    fn decel(&mut self, bed: &mut impl Bed, d: &mut impl Draw, sounds: &BikeSounds) {
        self.note = Note::Decel;
        self.sound(Sample { list: Note::Decel, index: 0 }, true, false, sounds, bed, d);
        self.next = Note::Idle;
    }

    /// The clutch's blip, the free rev, the limiter and the fall from it, by the state asked for.
    fn rev(&mut self, want: Note, h: &Hands, sounds: &BikeSounds, bed: &mut impl Bed, d: &mut impl Draw) {
        match want {
            Note::Clutch => {
                let s = self.picked(Note::Clutch, 4, sounds, d);
                self.note = Note::Clutch;
                self.sound(s, true, false, sounds, bed, d);
                self.next = Note::Rev;
            }
            Note::Panic if 0.1 < h.gas => {
                self.limiter(true, bed);
                self.note = Note::Panic;
                self.next = Note::Unwind;
            }
            Note::Panic | Note::Unwind => {
                self.limiter(false, bed);
                self.note = Note::Unwind;
                self.next = Note::Idle;
            }
            _ => {
                self.note = Note::Rev;
                self.sound(Sample::REV, true, false, sounds, bed, d);
                self.next = Note::Panic;
            }
        }
    }
}

/// **The Doppler shift** as the original's 3-D sound makes it: the speed the bike closes on the ear
/// along the line between them (level components only), in mph truncated, over 1125 (the speed of
/// sound in feet a second, against a speed in mph: the original's own mix of units), times
/// `DopplerFactor`. Only a closing bike is raised; a shift under 0.01, and so every receding bike, is
/// none.
pub fn doppler(factor: f32, at: [f32; 3], vel: [f32; 3], ear: [f32; 3], ear_vel: [f32; 3]) -> f32 {
    let d = [ear[0] - at[0], ear[1] - at[1], ear[2] - at[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len < 1e-3 {
        return 1.0;
    }
    let closing = ((vel[0] - ear_vel[0]) * d[0] + (vel[2] - ear_vel[2]) * d[2]) / len;
    let shift = factor * (((closing * 0.68182) as i32) as f32 / 1125.0);
    if shift < 0.01 { 1.0 } else { 1.0 + shift }
}

/// A sample this engine started, while it may still sound.
struct Sounding {
    s: Sample,
    voice: VoiceId,
    /// Its pitch variance, as a rate.
    detune: f32,
}

/// The engine's level in the mix: every engine sample is played at the one volume, then the distance.
const ENGINE_GAIN: f32 = 0.7;

/// The mixer as the engine note's [`Bed`].
struct Bench<'a> {
    mixer: &'a mut Mixer,
    sounds: &'a BikeSounds,
    voices: &'a mut Vec<Sounding>,
    limiter: &'a mut Option<VoiceId>,
    at: [f32; 3],
    pitch: f32,
}

impl Bench<'_> {
    fn start(&mut self, s: Sample) -> Option<VoiceId> {
        let (clip, looping) = self.sounds.clip(s).cloned()?;
        if !self.mixer.within(self.at, self.sounds.far) {
            return None;
        }
        let play = Play {
            gain: ENGINE_GAIN,
            pitch: self.pitch,
            looping,
            at: Some(self.at),
            near: self.sounds.near,
            far: self.sounds.far,
            bus: Bus::Engine,
        };
        Some(self.mixer.play(clip, play))
    }
}

impl Bed for Bench<'_> {
    fn playing(&self, s: Sample) -> bool {
        self.voices.iter().any(|v| v.s == s && self.mixer.playing(v.voice))
    }

    fn position(&self, s: Sample) -> f32 {
        let v = self.voices.iter().rev().find(|v| v.s == s);
        v.and_then(|v| self.mixer.progress(v.voice)).map_or(-1.0, |p| p.1)
    }

    fn stop(&mut self, s: Sample) {
        for v in self.voices.iter().filter(|v| v.s == s) {
            self.mixer.stop(v.voice);
        }
    }

    fn play(&mut self, s: Sample) -> bool {
        // A sample is one voice: starting it again starts it over.
        self.stop(s);
        let Some(voice) = self.start(s) else { return false };
        self.voices.push(Sounding { s, voice, detune: 1.0 });
        true
    }

    fn detune(&mut self, s: Sample, by: i32) {
        let rate = self.sounds.clip(s).map_or(22_050, |c| c.0.rate);
        if let Some(v) = self.voices.iter_mut().rev().find(|v| v.s == s) {
            let own = rate as f32 * 4096.0 / 48_000.0;
            v.detune = ((own + by as f32) / own).max(0.0);
        }
    }

    fn hold(&mut self, on: bool) {
        for v in self.voices.iter().filter(|v| v.s == Sample::REV) {
            self.mixer.pause(v.voice, on);
        }
        if let Some(v) = self.limiter.take() {
            self.mixer.stop(v);
        }
        if on {
            *self.limiter = self.start(Sample::PANIC);
        }
    }
}

/// **One bike's engine note**: the original's machine ([`Brain`]) on this file's own dice, each sample
/// it starts a voice of its own that plays out under the next one unless it is cut.
pub struct Engine {
    pub brain: Brain,
    dice: Dice,
    voices: Vec<Sounding>,
    limiter: Option<VoiceId>,
    /// The nearest ear last frame, for its speed.
    ear: Option<[f32; 3]>,
}

impl Engine {
    pub fn new(seed: u64) -> Engine {
        Engine { brain: Brain::new(), dice: Dice::new(seed), voices: Vec::new(), limiter: None, ear: None }
    }

    pub fn note(&self) -> Note {
        self.brain.note
    }

    /// One frame: the machine, then every voice moved to the bike and shifted for its speed.
    pub fn update(&mut self, h: &Hands, dt: f32, mixer: &mut Mixer, sounds: &BikeSounds) {
        let ear = mixer.nearest(h.at);
        let ear_vel = match (ear, self.ear) {
            (Some(e), Some(was)) if dt > 0.0 => [(e[0] - was[0]) / dt, (e[1] - was[1]) / dt, (e[2] - was[2]) / dt],
            _ => [0.0; 3],
        };
        self.ear = ear;
        let pitch = ear.map_or(1.0, |e| doppler(sounds.doppler, h.at, h.vel, e, ear_vel));
        let mut bench = Bench {
            mixer,
            sounds,
            voices: &mut self.voices,
            limiter: &mut self.limiter,
            at: h.at,
            pitch,
        };
        // The idle the loader queues is started by the emitter when it is not sounding.
        if self.brain.current == Sample::IDLE && self.brain.note == Note::Idle && !bench.playing(Sample::IDLE) {
            bench.play(Sample::IDLE);
        }
        self.brain.frame(h, dt, sounds, &mut bench, &mut self.dice);
        for v in &self.voices {
            mixer.set(v.voice, ENGINE_GAIN, v.detune * pitch, Some(h.at));
        }
        if let Some(v) = self.limiter {
            mixer.set(v, ENGINE_GAIN, pitch, Some(h.at));
        }
        self.voices.retain(|v| mixer.playing(v.voice));
    }

    /// Quiet the engine (the rider left the bike for the plane); it comes back at the idle.
    pub fn silence(&mut self, mixer: &mut Mixer) {
        for v in self.voices.drain(..) {
            mixer.stop(v.voice);
        }
        if let Some(v) = self.limiter.take() {
            mixer.stop(v);
        }
        self.brain = Brain::new();
    }

    /// Every voice the engine has going: its samples and the limiter's loop.
    pub fn voices(&self) -> impl Iterator<Item = VoiceId> + '_ {
        self.voices.iter().map(|v| v.voice).chain(self.limiter)
    }

    /// The voice of the note's own sample, while it sounds.
    pub fn voice(&self) -> Option<VoiceId> {
        self.voices.iter().rev().find(|v| v.s == self.brain.current).map(|v| v.voice)
    }
}

// --- the plane's engine -----------------------------------------------------------------------------

/// One of `carengine.aud`'s loops: a sample, and its volume (percent) and rate offset (Hz from the
/// file's `OriginalSampleRatePS2`) keyed on the engine's revolutions.
#[derive(Clone, Debug)]
pub struct Loop {
    pub clip: Arc<Clip>,
    pub keys: Vec<(f32, f32, f32)>,
}

/// A keyed-loop engine (`carengine.aud`'s `[biplane]`).
#[derive(Clone, Debug, Default)]
pub struct LoopEngine {
    pub loops: Vec<Loop>,
    pub max_rpm: f32,
    pub base_rate: f32,
    pub near: f32,
    pub far: f32,
}

impl LoopEngine {
    pub fn read(text: &str, profile: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> LoopEngine {
        let s = ini::parse(text);
        let Some(p) = section(&s, profile) else { return LoopEngine::default() };
        let count = p.number("NumberOfEngineLoops").unwrap_or(0.0) as usize;
        let mut loops = Vec::new();
        for i in 1..=count {
            let Some(l) = section(&s, &format!("{profile}Loop{i}")) else { continue };
            let Some(clip) = l.get("Filename").and_then(&mut *lib) else { continue };
            let n = l.number("NumberOfKeys").unwrap_or(0.0) as usize;
            let keys = (1..=n)
                .filter_map(|k| {
                    Some((
                        l.number(&format!("key{k}RPM"))?,
                        l.number(&format!("key{k}Vol"))?,
                        l.number(&format!("key{k}Rate"))?,
                    ))
                })
                .collect();
            loops.push(Loop { clip, keys });
        }
        LoopEngine {
            loops,
            max_rpm: p.number("MaxRPM").unwrap_or(5000.0),
            base_rate: p.number("OriginalSampleRatePS2").unwrap_or(22050.0),
            near: p.number("AttenuationMinDistance").unwrap_or(80.0),
            far: p.number("AttenuationMaxDistance").unwrap_or(700.0),
        }
    }

    /// One loop's `(volume 0..1, pitch)` at `rpm`, interpolated between its keys; silent outside them.
    pub fn at(&self, l: &Loop, rpm: f32) -> (f32, f32) {
        let k = &l.keys;
        if k.is_empty() || rpm < k[0].0 || rpm > k[k.len() - 1].0 {
            return (0.0, 1.0);
        }
        let i = k.iter().rposition(|key| key.0 <= rpm).unwrap_or(0).min(k.len().saturating_sub(2));
        let (a, b) = (k[i], k[(i + 1).min(k.len() - 1)]);
        let t = if b.0 > a.0 { (rpm - a.0) / (b.0 - a.0) } else { 0.0 };
        let vol = (a.1 + (b.1 - a.1) * t) / 100.0;
        let rate = a.2 + (b.2 - a.2) * t;
        (vol, ((self.base_rate + rate) / self.base_rate).max(0.05))
    }
}

/// The plane's engine playing: one voice per loop.
#[derive(Default)]
pub struct PlaneNote {
    voices: Vec<VoiceId>,
}

impl PlaneNote {
    /// `note` is the flight's engine note, `0..1` of its range.
    pub fn update(&mut self, mixer: &mut Mixer, e: &LoopEngine, note: f32, at: [f32; 3]) {
        if self.voices.is_empty() {
            for l in &e.loops {
                let play = Play { gain: 0.0, looping: true, at: Some(at), near: e.near, far: e.far, bus: Bus::Engine, ..Play::default() };
                self.voices.push(mixer.play(l.clip.clone(), play));
            }
        }
        let rpm = note.clamp(0.0, 1.0) * e.max_rpm;
        for (v, l) in self.voices.iter().zip(&e.loops) {
            let (vol, pitch) = e.at(l, rpm);
            mixer.set(*v, vol * 0.6, pitch, Some(at));
        }
    }

    pub fn silence(&mut self, mixer: &mut Mixer) {
        for v in self.voices.drain(..) {
            mixer.stop(v);
        }
    }
}

// --- the rider, the world, and no landing ------------------------------------------------------

/// `rider.aud`: the voice's two wreck lists.
#[derive(Clone, Debug, Default)]
pub struct RiderSounds {
    pub fall: List,
    pub impact: List,
    pub near: f32,
    pub far: f32,
    /// `[Params]`' `MinImpulse`: how fast the thrown body must be going for a touch to cry out
    /// ([`RiderVoice`], P180).
    pub min_impulse: f32,
}

impl RiderSounds {
    pub fn read(text: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> RiderSounds {
        let s = ini::parse(text);
        let params = section(&s, "Params");
        RiderSounds {
            fall: List::read(section(&s, "Fall"), lib),
            impact: List::read(section(&s, "IMPACT"), lib),
            near: params.and_then(|p| p.number("MinDistance")).unwrap_or(30.0),
            far: params.and_then(|p| p.number("MaxDistance")).unwrap_or(150.0),
            min_impulse: params.and_then(|p| p.number("MinImpulse")).unwrap_or(12.8),
        }
    }
}

/// What the rider's voice reads of the thrown body in a tick, after the tick (P180).
#[derive(Clone, Copy, Debug, Default)]
pub struct RiderBody {
    /// Wrecked, for any reason but the shootoff's 13.
    pub down: bool,
    /// Something of the body is on the ground.
    pub touching: bool,
    /// How fast the body is going.
    pub speed: f32,
    /// Where the body is.
    pub at: [f32; 3],
}

impl RiderBody {
    /// The thrown body after the tick: the ragdoll's own sensors when the ride runs one, else the one
    /// point the wreck has for the rider — the same two the body's settle test reads.
    pub fn of(rider: &game_sim::rider::Rider) -> RiderBody {
        let reason = rider.state.wreck.map_or(0, |w| w.0 as i32);
        let (touching, vel) = match (rider.ragdoll.as_ref(), rider.crash.as_ref()) {
            (Some(doll), _) => (doll.sensed.touching, doll.sensed.vel),
            (None, Some(down)) => (down.landed, down.rider_vel),
            (None, None) => return RiderBody::default(),
        };
        let at = game_sim::rider::Rider::main_bone(&rider.ride, None, rider.ragdoll.as_ref(), rider.crash.as_ref());
        RiderBody {
            down: reason != 0 && reason != 13,
            touching,
            speed: length([vel.x(), vel.y(), vel.z()]),
            at: [at.x(), at.y(), at.z()],
        }
    }
}

/// **What an online seat's rider said** (P184), read off the ride as it is drawn rather than off the
/// rider's own frames, which online run inside the session — some of them twice, when a late pad
/// rolls the ride back. A wreck entry is the rider's wreck reason going from none to some between
/// two drawn frames, and a landed trick the bike coming down between them, off the ground with a
/// trick asked for before and on it after, with no wreck. A tick rolled back and run again to the
/// same wreck draws the wreck both times, and is heard once; one that runs out differently takes
/// the wreck back, and the wreck it does have is heard when it comes. Presentation only: it reads
/// the ride and writes nothing of it.
#[derive(Clone, Debug, Default)]
pub struct Said {
    /// Each room seat's rider at the last drawn frame: wrecked, off the ground, the trick in flight.
    /// `None` until the seat is first drawn, which says nothing. By room seat, not shell seat, so a
    /// join or a leave that moves the shell's seats round moves nobody's memory onto another rider.
    was: Vec<Option<(bool, bool, game_sim::trick::Trick)>>,
}

impl Said {
    /// What room seat `d`'s rider did since the last drawn frame: its wreck entry and its landed
    /// trick, the rest of the frame left at none.
    pub fn of(&mut self, d: usize, rider: &game_sim::rider::Rider) -> game_sim::rider::Frame {
        let (airborne, flight) = rider.in_flight();
        let now = (rider.state.wreck.is_some(), airborne, flight);
        if self.was.len() <= d {
            self.was.resize(d + 1, None);
        }
        let mut said = game_sim::rider::Frame::default();
        if let Some((wrecked, was_airborne, was_flight)) = self.was[d].replace(now) {
            if !wrecked {
                said.wreck = rider.state.wreck;
            }
            if said.wreck.is_none() && !now.0 && was_airborne && !airborne && was_flight != game_sim::trick::Trick::NONE {
                said.landed = Some(was_flight);
            }
        }
        said
    }
}

/// **The rider's voice** (P180), the original's rider emitter: the wreck's own cry is played through
/// it at the entry ([`Sound::frame`]), and every tick after, while the body is down, touching and
/// faster than `MinImpulse` with nothing of its own still playing, it cries out again — a no-repeat
/// pick of `[IMPACT]` at full level, on the body and following it. So a fall off the back is its
/// short `[Fall]` grunt as the rider leaves the bike and then a scream as they hit the ground, which
/// is what S109's recorded fall has (the grunt at the entry, the scream about 0.1 s after the body's
/// touching flag); a body still sliding fast cries again once the scream is over.
///
/// `[IMPACT]` is streamed like the bike's clank, so "over" keeps [`WRECK_TAIL`] past the clip's end.
///
/// The speed held against `MinImpulse` is the larger of this tick's and the last: the speed the body
/// came down at. The port's bodies can lose it on the very tick they touch — the one point a ride with
/// no ragdoll has for the rider stops dead on it — where the original's body reads it the tick after.
#[derive(Debug, Default)]
pub struct RiderVoice {
    voice: Option<VoiceId>,
    tail: f32,
    /// The body's speed the tick before.
    was: f32,
}

impl RiderVoice {
    /// Whether the voice is still busy, `dt` on; a voice still playing follows the body to `at`.
    fn busy(&mut self, m: &mut Mixer, at: [f32; 3], dt: f32) -> bool {
        let playing = self.voice.filter(|&id| m.playing(id));
        if let Some(id) = playing {
            let gain = m.gain_of(id).unwrap_or(0.0);
            m.set(id, gain, 1.0, Some(at));
        } else if self.voice.is_some() {
            self.tail = WRECK_TAIL;
        } else {
            self.tail = (self.tail - dt).max(0.0);
        }
        self.voice = playing;
        playing.is_some() || self.tail > 0.0
    }

    /// The voice's sound now: the voice playing, if one is.
    pub fn voice(&self) -> Option<VoiceId> {
        self.voice
    }

    /// Every voice was stopped.
    pub fn hushed(&mut self) {
        *self = RiderVoice::default();
    }
}

// No landing sound (P66). The original's motocross bike makes four sound emitters — the engine note,
// the bike's `[Wreck]` clank (which plays only while the bike is wrecked), the race events and the
// surface the wheels roll on — and none of them is played by a landing: a landing is heard only as
// the engine note coming back from its airborne decel. `machine.aud`'s `[<machine>Suspension]` tiers
// (`suspen00..03`, `trkbtm_*`) are the buggy's, the baja's and the monster truck's, loaded and played
// only by those machines' classes; P42 played the buggy's on the bike. research/oracle.txt
// `machine_suspension_*` has the machines' rule for when they are ported.

/// A track's `.aud` and the race's `event.aud`: the world's fixed emitters, its random ones, and the
/// crowd where the track has one.
#[derive(Clone, Debug, Default)]
pub struct WorldSounds {
    pub fixed: Vec<(Arc<Clip>, [f32; 3], bool, f32, f32)>,
    /// `(clip, cycle in frames, percent)`.
    pub random: Vec<(Arc<Clip>, f32, f32)>,
    pub crowd: bool,
    pub crowd_bed: List,
    pub cheer: List,
    pub oh: List,
}

impl WorldSounds {
    pub fn read(track: &str, event: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> WorldSounds {
        let t = ini::parse(track);
        let e = ini::parse(event);
        let sounds = section(&t, "Sounds");
        let count = |k: &str| sounds.and_then(|s| s.number(k)).unwrap_or(0.0) as usize;
        let mut fixed = Vec::new();
        for i in 1..=count("NumSoultreeSounds") {
            let Some(s) = section(&t, &format!("SoultreeSound{i}")) else { continue };
            let (Some(name), Some(p)) = (s.get("SoundResourceFile"), s.numbers("Position")) else { continue };
            if p.len() < 3 {
                continue;
            }
            let Some(clip) = lib(name) else { continue };
            let looping = s.get("Looping").is_some_and(truthy);
            let near = s.number("MinDistance").unwrap_or(20.0);
            let far = s.number("MaxDistance").unwrap_or(100.0);
            fixed.push((clip, [p[0], p[1], p[2]], looping, near, far));
        }
        let mut random = Vec::new();
        for i in 1..=count("NumAmbientSounds") {
            let Some(s) = section(&t, &format!("AmbientSound{i}")) else { continue };
            let Some(clip) = s.get("SoundResourceFile").and_then(&mut *lib) else { continue };
            random.push((clip, s.number("CycleTime").unwrap_or(300.0), s.number("RandomTriggerPercent").unwrap_or(0.0)));
        }
        let crowd = sounds.and_then(|s| s.get("CrowdPresent")).is_some_and(truthy);
        let (crowd_bed, cheer, oh) = if crowd {
            (
                List::read(section(&e, "CrowdAmbient"), lib),
                List::read(section(&e, "CrowdCheer"), lib),
                List::read(section(&e, "CrowdOh"), lib),
            )
        } else {
            Default::default()
        };
        WorldSounds { fixed, random, crowd, crowd_bed, cheer, oh }
    }
}

// --- the path cars' loops (V74) ---------------------------------------------------------------------

/// **A toy's sound** (V74): a loop a path car carries, out of the track's `.toy`.
///
/// A `[CarN]` names its sounds (`NumberOfSounds`, `Sound1..n`: `game_render::toy::Car::sounds`). The
/// original's path-car reader makes one sound object per name and hands it the car's model to ride
/// on. That object's reader formats `ToySound_<name>` and, where the `.toy` has the section and the
/// section a `SoundResourceFile`, reads `Looping`, `OneShot`, `ResetWhenInaudible`, `Force2D`,
/// `Stereo`, `OneShotDistance` (20), `RandomTriggerPercent` (100), `CycleTime`, `MinDistance` and
/// `MaxDistance` (0 where missing), `Priority` (10), `IsStream`, `StartEnabled` (on),
/// `AttachToObject` (`body`) and `RolloffFactor` (1); a name with no section is no sound. Its class
/// is the plain world sound's but for that reader: nothing of it follows the car's speed, so the
/// loop plays at its own pitch, only its place moving with the car.
///
/// On the disc every one is an enabled loop with both distances: free06's train (the engine's
/// `trainengine`, the cars behind it `trainloop1..3`), and the `semi` and `car` loops of the trucks of
/// free06, free07, free09, free10 and free11. `[ToySound_Shovel2]` (free06) is named by no car:
/// its shovels are commented out of the file, and it never plays.
#[derive(Clone, Debug)]
pub struct ToySound {
    pub clip: Arc<Clip>,
    pub looping: bool,
    /// `StartEnabled`: off, the sound is made and never started (nothing on the disc turns one off).
    pub enabled: bool,
    pub near: f32,
    pub far: f32,
}

impl ToySound {
    /// `[ToySound_<name>]` of a parsed `.toy`, or `None` where the section or its file is missing.
    pub fn read(toy: &[Section], name: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> Option<ToySound> {
        let s = section(toy, &format!("ToySound_{name}"))?;
        let clip = lib(s.get("SoundResourceFile")?.trim())?;
        Some(ToySound {
            clip,
            looping: s.get("Looping").is_some_and(truthy),
            enabled: s.get("StartEnabled").is_none_or(truthy),
            near: s.number("MinDistance").unwrap_or(0.0),
            far: s.number("MaxDistance").unwrap_or(0.0),
        })
    }

    /// Every car's sounds, `names[k]` being car `k`'s `Sound1..n`: what [`Sound::toys_at`] plays.
    pub fn cars(toy: &str, names: &[Vec<String>], lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> Vec<Vec<ToySound>> {
        let toy = ini::parse(toy);
        names.iter().map(|car| car.iter().filter_map(|n| ToySound::read(&toy, n, lib)).collect()).collect()
    }
}

// --- the shootoff (P85) -----------------------------------------------------------------------------

/// The wreck reason of the cannonball, the throw back off the map's edge (S59).
pub const CANNONBALL: i32 = 13;
/// How near an edge the shootoff's warning starts, feet: the music begins to fade and the rumble to
/// rise here, and both are at their ends at the throw's own 150-ft margin, [`EDGE_RAMP`] further in.
pub const EDGE_WARNING: f32 = 350.0;
pub const EDGE_RAMP: f32 = 200.0;

/// The map-edge query the shootoff's sound asks: `None` while `at` is [`EDGE_WARNING`] or more inside
/// every edge of a `width` × `depth` map, else the distance to the nearest edge (negative past it).
pub fn edge_distance(width: f32, depth: f32, at: [f32; 3]) -> Option<f32> {
    let m = EDGE_WARNING;
    if m <= at[0] && at[0] <= width - m && m <= at[2] && at[2] <= depth - m {
        return None;
    }
    Some(at[0].min(width - at[0]).min(at[2]).min(depth - at[2]))
}

/// The rumble's volume, `0..=100`, at `d` feet from the edge: 25 at the warning's start, 100 at the
/// margin.
pub fn rumble_volume(d: f32) -> i32 {
    (((EDGE_WARNING - d) / EDGE_RAMP * 75.0) as i32 + 25).clamp(0, 100)
}

/// What the music is scaled by at `d` feet from the edge: 1 at the warning's start, 0 at the margin.
pub fn music_scale(d: f32) -> f32 {
    1.0 - (EDGE_WARNING - d) / EDGE_RAMP
}

/// `event.aud`'s `[Cannonball]`: the boom of the throw (`Cannon`) and the rumble that rises as the
/// edge comes near (`Rumble`). Neither is placed in the world: everyone hears them alike.
#[derive(Clone, Debug, Default)]
pub struct Shootoff {
    pub cannon: Option<(Arc<Clip>, bool)>,
    pub rumble: Option<(Arc<Clip>, bool)>,
}

impl Shootoff {
    pub fn read(event: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> Shootoff {
        let e = ini::parse(event);
        let Some(s) = section(&e, "Cannonball") else { return Shootoff::default() };
        let mut one = |key: &str| {
            let clip = s.get(key).map(str::trim).and_then(&mut *lib)?;
            Some((clip, s.get(&format!("{key}Looping")).is_some_and(truthy)))
        };
        Shootoff { cannon: one("Cannon"), rumble: one("Rumble") }
    }
}

/// What one rider's shootoff sound reads in a tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeFrame {
    /// The map edge is armed on this bike (the throw's own gate, S73).
    pub armed: bool,
    /// The ride's wreck reason after the tick, 0 riding.
    pub reason: i32,
    /// [`edge_distance`] of the chassis' centre of gravity after the tick.
    pub edge: Option<f32>,
}

/// **One rider's shootoff sound**, the original's sound child of the player's bike. It runs among
/// the bike's children, after the physics and before the ride's state machine, so it reads the
/// position of this tick and the reason the last one left: the boom comes the tick after the throw,
/// and the music the tick after the put-back.
///
/// - The cannonball newly seen: the rumble stops and the boom plays.
/// - Riding, just back from a cannonball: the music is the settings' own again.
/// - Riding near an edge: the rumble plays at [`rumble_volume`] and the music is scaled by
///   [`music_scale`], cut to the settings' whole percent first. Riding away from it: the rumble
///   stops, and the music is left as the last tick near the edge put it.
///
/// The music is the whole mix's, so in a split every rider's sound writes it in seat order.
#[derive(Debug, Default)]
pub struct EdgeSound {
    /// The reason the last tick left, which this one reads.
    pending: i32,
    /// The reason the last tick read.
    seen: i32,
    rumble: Option<VoiceId>,
    cannon: Option<VoiceId>,
}

impl EdgeSound {
    pub fn tick(&mut self, m: &mut Mixer, sounds: &Shootoff, f: &EdgeFrame) {
        let reason = std::mem::replace(&mut self.pending, f.reason);
        if f.armed {
            if reason == CANNONBALL && self.seen != CANNONBALL {
                self.stop_rumble(m);
                if let Some((clip, looping)) = &sounds.cannon {
                    self.cannon = Some(m.play(clip.clone(), Play { looping: *looping, ..Play::default() }));
                }
                // The original pauses the music's stream here too, and it is going again the next
                // tick: the song plays on, at the volume the last tick near the edge left (0).
            } else if reason == 0 {
                if self.seen == CANNONBALL {
                    m.music_edge = None;
                }
                match f.edge {
                    None => self.stop_rumble(m),
                    Some(d) => {
                        let gain = rumble_volume(d) as f32 / 100.0;
                        match self.rumble.filter(|&id| m.playing(id)) {
                            Some(id) => m.set(id, gain, 1.0, None),
                            None => {
                                if let Some((clip, looping)) = &sounds.rumble {
                                    let play = Play { gain, looping: *looping, ..Play::default() };
                                    self.rumble = Some(m.play(clip.clone(), play));
                                }
                            }
                        }
                        m.music_edge = Some(music_scale(d));
                    }
                }
            }
        }
        self.seen = reason;
    }

    fn stop_rumble(&mut self, m: &mut Mixer) {
        if let Some(id) = self.rumble.take() {
            m.stop(id);
        }
    }

    /// The rumble's voice while it plays.
    pub fn rumble(&self) -> Option<VoiceId> {
        self.rumble
    }

    /// The last boom's voice.
    pub fn cannon(&self) -> Option<VoiceId> {
        self.cannon
    }

    fn voices(&self) -> impl Iterator<Item = VoiceId> {
        self.rumble.into_iter().chain(self.cannon)
    }

    /// Every voice was stopped: nothing of this rider's plays, and the reasons seen are kept.
    pub fn hushed(&mut self) {
        self.rumble = None;
        self.cannon = None;
    }
}

// --- the plane's crash (P164) -----------------------------------------------------------------------

/// A machine's `[<prefix>Wreck]` in `machine.aud` (the plane's is `[biplaneWreck]`, `Vehicle\mchn_wreck1`
/// to `5`): the list, the distances it fades over, and the impulses its level runs between.
#[derive(Clone, Debug, Default)]
pub struct MachineWreck {
    pub list: List,
    pub near: f32,
    pub far: f32,
    pub min_impulse: f32,
    pub max_impulse: f32,
}

impl MachineWreck {
    /// The loader's defaults where a key is missing: distances 1 and 10⁹, impulses 0 and 1.
    pub fn read(machine: &str, prefix: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> MachineWreck {
        let s = ini::parse(machine);
        let Some(w) = section(&s, &format!("{prefix}Wreck")) else { return MachineWreck::default() };
        let n = |k: &str, d: f32| w.number(k).unwrap_or(d);
        MachineWreck {
            list: List::read(Some(w), lib),
            near: n("MinDistance", 1.0),
            far: n("MaxDistance", 1.0e9),
            min_impulse: n("MinImpulse", 0.0),
            max_impulse: n("MaxImpulse", 1.0),
        }
    }
}

/// What the plane's wreck emitter reads in a tick, after the tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlaneCrash {
    /// The wreck word is set (crashed, or thrown back off the map's edge).
    pub wrecked: bool,
    /// The collision object's record touched something this tick.
    pub touching: bool,
    /// The length of the body's shock peak: the largest velocity change the contacts gave it this
    /// tick, per axis.
    pub impulse: f32,
}

impl PlaneCrash {
    /// The plane's, off its flight after the tick: the wreck word (either reason), the collision
    /// record's touched flag and the body's shock peak.
    pub fn of(f: &game_sim::flight::Flight) -> PlaneCrash {
        let p = f.body.shock_peak;
        PlaneCrash { wrecked: f.crash.reason != 0, touching: f.record.touching, impulse: length([p.x(), p.y(), p.z()]) }
    }
}

/// The flight's engine note, `0..1` of its range: what the engine sound reads. The wreck zeroes it
/// and stops the sound, and it stands at zero until the engine steps again after the unwreck (P187).
pub fn plane_note(f: &game_sim::flight::Flight) -> f32 {
    f.engine.note.0 / f.engine.max_rpm.0.max(1.0)
}

/// What the bike's wreck emitter reads in a tick, after the tick (P144). Its words are the bike's:
/// the wreck reason's copy, the riding collision record's touched flag and the bike's impulse
/// (`+0x550`). While the rider is down, the riding record is the wreck body's and the impulse is the
/// wreck body's shock peak, the two equal on every row of P144's capture over S109's fall.
#[derive(Clone, Copy, Debug, Default)]
pub struct BikeCrash {
    /// Down, and the clank's gate: wrecked, the record touched, the wreck body's shock peak.
    pub crash: PlaneCrash,
    /// The riding bike's impulse this tick: the bottom-out closing speed its ground step took up,
    /// the largest over the substeps. What the wreck entry's own call reads.
    pub entry_impulse: f32,
    /// Where the bike is: the wreck body while down.
    pub at: [f32; 3],
}

impl BikeCrash {
    pub fn of(rider: &game_sim::rider::Rider) -> BikeCrash {
        let g = rider.ride.mv.g_peak;
        let p = rider.ride.position();
        let mut out = BikeCrash { entry_impulse: length([g.x(), g.y(), g.z()]), at: [p.x(), p.y(), p.z()], ..BikeCrash::default() };
        if let (Some(_), Some(down)) = (rider.state.wreck, rider.crash.as_ref()) {
            let (b, peak) = (&down.bike, down.bike.body.shock_peak);
            out.crash = PlaneCrash { wrecked: true, touching: b.record.touching, impulse: length([peak.x(), peak.y(), peak.z()]) };
            out.at = [b.body.position.x(), b.body.position.y(), b.body.position.z()];
        }
        out
    }
}

/// A vector's length as the emitter takes it: `sqrt(z² + y² + x²)`, in that order.
pub fn length(v: [f32; 3]) -> f32 {
    (v[2] * v[2] + v[1] * v[1] + v[0] * v[0]).sqrt()
}

/// The clank's level, `0..=100`, at `impulse`: its share of `MaxImpulse`, cut to a whole percent.
pub fn wreck_level(impulse: f32, max_impulse: f32) -> i32 {
    ((impulse / max_impulse * 100.0) as i32).clamp(0, 100)
}

/// **One plane's wreck clank**, the original's wreck emitter (the bike's `[Wreck]` one, P144) on the
/// plane's wreck word, its collision record and its body's shock peak. Each tick, after the
/// simulation: wrecked, touching, nothing of its own still playing, and the impulse at least
/// `MinImpulse` — a no-repeat pick of the list at [`wreck_level`], placed on the plane and following it.
/// So the crash's own contact starts one on the wreck tick, the tumble's later hits more once each
/// has ended, and a landing, never wrecked, none.
///
/// "Ended" is the streamed voice's state on the sound chip, which trails the clip: a 0.89 s clank still
/// read as playing 1.23 s after it started and was over by 1.67 s, so the emitter stays busy
/// [`WRECK_TAIL`] past the clip's end.
#[derive(Debug, Default)]
pub struct WreckClank {
    voice: Option<VoiceId>,
    last: Option<i32>,
    /// Seconds of the tail left.
    tail: f32,
}

/// How long the emitter still reads busy after its clank ended. The capture bounds it to more than
/// 0.34 s and at most 0.78 s after its 0.89 s clip; the pick here is not the original's, and a tail
/// that fits the longest clip (1.18 s) in that window too is at most 0.43 s.
pub const WRECK_TAIL: f32 = 0.4;

impl WreckClank {
    /// One tick, `dt` long. The voice started, if one was.
    pub fn tick(&mut self, m: &mut Mixer, w: &MachineWreck, f: &PlaneCrash, at: [f32; 3], dt: f32, dice: &mut Dice) -> Option<VoiceId> {
        let busy = self.busy(m, at, dt);
        if !(f.wrecked && f.touching) || busy || f.impulse < w.min_impulse {
            return None;
        }
        self.play(m, w, f.impulse, at, dice)
    }

    /// The tick of the player's wreck entry (P144), in place of [`WreckClank::tick`]: the entry's own
    /// call, which asks for nothing but the emitter idle and `impulse` at least `MinImpulse`, and
    /// plays the same pick at the same level. A fall off the back, with no impulse, is silent.
    pub fn entry(&mut self, m: &mut Mixer, w: &MachineWreck, impulse: f32, at: [f32; 3], dt: f32, dice: &mut Dice) -> Option<VoiceId> {
        let busy = self.busy(m, at, dt);
        if busy || impulse < w.min_impulse {
            return None;
        }
        self.play(m, w, impulse, at, dice)
    }

    /// Whether anything of the emitter's own still sounds, its tail counted: the voice that plays
    /// follows `at`, and one that ended starts the tail.
    fn busy(&mut self, m: &mut Mixer, at: [f32; 3], dt: f32) -> bool {
        let playing = self.voice.filter(|&id| m.playing(id));
        if let Some(id) = playing {
            let gain = m.gain_of(id).unwrap_or(0.0);
            m.set(id, gain, 1.0, Some(at));
        } else if self.voice.is_some() {
            self.tail = WRECK_TAIL;
        } else {
            self.tail = (self.tail - dt).max(0.0);
        }
        self.voice = playing;
        playing.is_some() || self.tail > 0.0
    }

    /// A no-repeat pick of the list at [`wreck_level`] of `impulse`, placed at `at`.
    fn play(&mut self, m: &mut Mixer, w: &MachineWreck, impulse: f32, at: [f32; 3], dice: &mut Dice) -> Option<VoiceId> {
        let mut last = self.last.unwrap_or(-1);
        let (clip, looping) = w.list.pick(dice, &mut last)?;
        self.last = Some(last);
        let gain = wreck_level(impulse, w.max_impulse) as f32 / 100.0;
        let id = m.play(clip, Play { gain, looping, at: Some(at), near: w.near, far: w.far, ..Play::default() });
        self.voice = Some(id);
        Some(id)
    }

    /// The clank's voice while it plays.
    pub fn voice(&self) -> Option<VoiceId> {
        self.voice
    }

    /// Every voice was stopped.
    pub fn hushed(&mut self) {
        self.voice = None;
        self.tail = 0.0;
    }
}

// --- the plane's wheels (P146) ----------------------------------------------------------------------

/// The `.aud` speeds are miles an hour; the loader keeps them in feet a second.
pub const MPH: f32 = 1.46666;

/// A machine's wheel touchdown in `machine.aud`: `[<prefix>TireParams]`, which must be there for the
/// rest to load, gives the distances, and `[<prefix>TireTouchdown]` the list (the plane's is one,
/// `Impact\plane_tire`), the speeds its level runs between and the levels at each end. The plane's:
/// 30 ft to 150 ft, level 40 at 60 mph and under, 100 at 150 mph and over.
#[derive(Clone, Debug, Default)]
pub struct MachineTouchdown {
    pub list: List,
    pub near: f32,
    pub far: f32,
    /// `MinLevelVelocity` and `MaxLevelVelocity`, in ft/s.
    pub slow: f32,
    pub fast: f32,
    /// `MinLevelAttenuation`, and `MaxLevelAttenuation` less it.
    pub level: i32,
    pub range: i32,
}

impl MachineTouchdown {
    /// The loader's defaults where a key is missing: distances 0 and 1, speeds and levels 0. No
    /// `[<prefix>TireParams]`, no touchdown.
    pub fn read(machine: &str, prefix: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> MachineTouchdown {
        let s = ini::parse(machine);
        let Some(p) = section(&s, &format!("{prefix}TireParams")) else { return MachineTouchdown::default() };
        let mut t = MachineTouchdown { near: p.number("MinDistance").unwrap_or(0.0), far: p.number("MaxDistance").unwrap_or(1.0), ..Default::default() };
        if let Some(d) = section(&s, &format!("{prefix}TireTouchdown")) {
            let n = |k: &str| d.number(k).unwrap_or(0.0);
            // On the EE's FPU, which chops: an IEEE multiply lands one bit high on both.
            t.slow = (F(n("MinLevelVelocity")) * F(MPH)).to_f32();
            t.fast = (F(n("MaxLevelVelocity")) * F(MPH)).to_f32();
            t.level = n("MinLevelAttenuation") as i32;
            t.range = n("MaxLevelAttenuation") as i32 - t.level;
            t.list = List::read(Some(d), lib);
        }
        t
    }

    /// The level, `0..=100` on the disc's numbers, of a touchdown at `speed` (the body velocity's
    /// length): the speed held between the two, its share of the way across, cut to a whole level.
    pub fn level_at(&self, speed: f32) -> i32 {
        let s = if self.slow < speed { speed } else { self.slow };
        let s = if s < self.fast { s } else { self.fast };
        (self.level as f32 + (s - self.slow) / (self.fast - self.slow) * self.range as f32) as i32
    }
}

/// **One plane's wheels touching down**, the original's machine emitter on the plane's grounded flag:
/// on the tick no wheel was down at the last measure and one is now — a landing, a bounce, the crash's
/// own tick when a wheel is in it, the put-back onto the spawn — the list's pick plays at
/// [`MachineTouchdown::level_at`] the body's speed, placed on the plane and following it. Nothing
/// holds it off: a touchdown while the last one still sounds starts it over, as the original's one
/// sound object does.
#[derive(Debug, Default)]
pub struct TireTouchdown {
    voice: Option<VoiceId>,
    last: Option<i32>,
}

impl TireTouchdown {
    /// One tick: `speed` is the body's on a touchdown tick, `None` on any other. The voice started,
    /// if one was.
    pub fn tick(&mut self, m: &mut Mixer, t: &MachineTouchdown, speed: Option<f32>, at: [f32; 3], dice: &mut Dice) -> Option<VoiceId> {
        self.voice = self.voice.filter(|&id| m.playing(id));
        if let Some(id) = self.voice {
            let gain = m.gain_of(id).unwrap_or(0.0);
            m.set(id, gain, 1.0, Some(at));
        }
        let speed = speed?;
        let mut last = self.last.unwrap_or(-1);
        let (clip, looping) = t.list.pick(dice, &mut last)?;
        self.last = Some(last);
        if let Some(id) = self.voice.take() {
            m.stop(id);
        }
        let gain = t.level_at(speed) as f32 / 100.0;
        let id = m.play(clip, Play { gain, looping, at: Some(at), near: t.near, far: t.far, ..Play::default() });
        self.voice = Some(id);
        Some(id)
    }

    /// The touchdown's voice while it plays.
    pub fn voice(&self) -> Option<VoiceId> {
        self.voice
    }

    /// Every voice was stopped.
    pub fn hushed(&mut self) {
        self.voice = None;
    }
}

// --- the wheels' drive-on loop (P145) ----------------------------------------------------------------

/// One material of a track's `Col\template.cmt`: what a wheel rolling on a prop of it plays. The
/// whole numbers are the loader's whole numbers (a missing one is 0); `FreqSlope` is its one float.
#[derive(Clone, Debug, Default)]
pub struct DriveOnMaterial {
    /// The material's `ID`, the low byte of a collision mesh's surface word.
    pub id: u8,
    /// `DriveOnSoundFile`.
    pub clip: Option<Arc<Clip>>,
    pub sample_speed: i32,
    pub min_volume: i32,
    pub min_speed: i32,
    pub max_speed: i32,
    pub resonant_speed: i32,
    pub freq_slope: f32,
    /// `DriveOnMinDistance` / `DriveOnMaxDistance`: full volume within the one, not started (and
    /// stopped) beyond the other — so a material whose `MaxDistance` is 0 is never heard.
    pub near: f32,
    pub far: f32,
}

/// A track's drive-on materials, from its `Col\template.cmt` (the same file in every track archive on
/// the disc: 22 materials, every one `Impact\drv_drt1.vag`, six of them heard).
#[derive(Clone, Debug, Default)]
pub struct DriveOnTable {
    pub materials: Vec<DriveOnMaterial>,
}

impl DriveOnTable {
    /// Every section with an `ID` but `[Settings]`.
    pub fn read(text: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> DriveOnTable {
        let int = |s: &Section, k: &str| s.get(k).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
        let materials = ini::parse(text)
            .iter()
            .filter(|s| !s.name.eq_ignore_ascii_case("Settings"))
            .filter_map(|s| {
                let id = u8::try_from(s.get("ID")?.trim().parse::<i32>().ok()?).ok()?;
                Some(DriveOnMaterial {
                    id,
                    clip: s.get("DriveOnSoundFile").and_then(|n| lib(n)),
                    sample_speed: int(s, "DriveOnSampleSpeed"),
                    min_volume: int(s, "DriveOnMinVolume"),
                    min_speed: int(s, "DriveOnMinSpeed"),
                    max_speed: int(s, "DriveOnMaxSpeed"),
                    resonant_speed: int(s, "DriveOnResonantFreqSpeed"),
                    freq_slope: s.number("DriveOnFreqSlope").unwrap_or(0.0),
                    near: int(s, "DriveOnMinDistance") as f32,
                    far: int(s, "DriveOnMaxDistance") as f32,
                })
            })
            .collect();
        DriveOnTable { materials }
    }

    pub fn get(&self, id: u8) -> Option<&DriveOnMaterial> {
        self.materials.iter().find(|m| m.id == id)
    }
}

/// A sound's own pitch in the original's units, 4096 a sample played at 48 kHz, as its bank holds it.
pub fn own_pitch(clip: &Clip) -> i32 {
    (clip.rate as u64 * 4096 / RATE as u64) as i32
}

/// **The drive-on speed**: the velocity's length in miles an hour, cut to a whole one.
pub fn drive_on_mph(vel: [f32; 3]) -> i32 {
    let v = vel.map(F);
    (F(0.68182) * F::dot(v, v).sqrt()).to_i32()
}

/// **The pitch slope**, pitch units a mile an hour: `FreqSlope` of the sound's own pitch per
/// `SampleSpeed`, rounded. The emitter keeps it until the sound is stopped.
pub fn drive_on_slope(m: &DriveOnMaterial, own: i32) -> i32 {
    (F(0.5) + F(m.freq_slope) * (F::from(own) / F::from(m.sample_speed))).to_i32()
}

/// **The pitch** at `mph`: the sound's own at `SampleSpeed`, `slope` a mile an hour off it, less a
/// draw of `0..=slope`.
pub fn drive_on_pitch(m: &DriveOnMaterial, own: i32, slope: i32, mph: i32, d: &mut impl Draw) -> i32 {
    let draw = (F::from(slope + 1) * (F(1.0 / 2_147_483_648.0) * F::from(d.rand()))).to_i32();
    let off = (mph - m.sample_speed).wrapping_mul(slope) - draw;
    (F::from(own) + F::from(off)).to_i32()
}

/// **The level** at `mph`, `0..=100`: up a line from `MinVolume` at `MinSpeed` to 100 at
/// `ResonantFreqSpeed`, then down one to `MinVolume` at `MaxSpeed` — each line's slope a whole-number
/// quotient, as the original divides.
pub fn drive_on_level(m: &DriveOnMaterial, mph: i32) -> i32 {
    let rise = 100 - m.min_volume;
    let v = if mph <= m.resonant_speed {
        rise.checked_div(m.resonant_speed - m.min_speed).unwrap_or(0) * (mph - m.min_speed) + m.min_volume
    } else {
        (-rise).checked_div(m.max_speed - m.resonant_speed).unwrap_or(0) * (mph - m.max_speed) + m.min_volume
    };
    v.clamp(0, 100)
}

/// What the drive-on loop reads of a seat after its tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriveOnFrame {
    /// The material under the rear wheel ([`game_sim::ride::Ride::rear_material`]): 0 on the ground.
    pub material: u8,
    /// Where the rear wheel touches.
    pub at: [f32; 3],
    /// The body's velocity, feet a second.
    pub vel: [f32; 3],
}

/// **One wheel's drive-on loop**, the original's drive-on emitter as the bike makes it: one wheel,
/// the rear, its material that of the prop it last touched this tick. Each tick: a still bike or no
/// material stops it; a new material starts its loop where it touches, if someone is near enough to
/// hear; the same one moves it and sets its pitch and level by speed, and stops it, for good, past
/// its `MaxDistance` — it is not started again until the wheel leaves the prop or the bike stops.
#[derive(Clone, Debug)]
pub struct DriveOn {
    /// The material sounding (or meant to be), 0 for none.
    cur: u8,
    voice: Option<VoiceId>,
    /// The pitch slope, worked out on the first pitch after a stop.
    slope: Option<i32>,
    /// The pitch last set: a worked-out one outside `0..0x4000` is not taken, and the sound keeps
    /// what it had (its own, before any).
    pitch: Option<i32>,
    /// The host's draws for the pitch's wobble, the loop's own.
    dice: Dice,
}

impl DriveOn {
    pub fn new(seed: u64) -> DriveOn {
        DriveOn { cur: 0, voice: None, slope: None, pitch: None, dice: Dice::new(seed) }
    }

    pub fn voice(&self) -> Option<VoiceId> {
        self.voice
    }

    /// The material it is on, 0 for none.
    pub fn material(&self) -> u8 {
        self.cur
    }

    pub fn tick(&mut self, m: &mut Mixer, table: &DriveOnTable, f: &DriveOnFrame) {
        // One wheel takes the whole level, while the body moves at all.
        let v = f.vel.map(F);
        if F::dot(v, v).sqrt() == F::ZERO || f.material == 0 {
            self.stop(m);
            return;
        }
        let share = 100;
        let sounding = self.voice.is_some_and(|id| m.playing(id));
        if f.material == self.cur || !sounding {
            if self.cur != 0 {
                // The material it is meant to be on keeps the loop even when the wheel has moved to
                // another one and the loop has gone quiet.
                return self.update(m, table, f, share);
            }
        } else {
            self.stop(m);
        }
        let Some(mat) = table.get(f.material) else { return };
        let Some(clip) = mat.clip.clone() else { return };
        self.cur = f.material;
        let (pitch, level) = self.work_out(mat, &clip, f.vel);
        let gain = (level * share / 100) as f32 / 100.0;
        if m.within(f.at, mat.far) {
            let play = Play {
                gain,
                pitch: pitch as f32 / own_pitch(&clip).max(1) as f32,
                looping: true,
                at: Some(f.at),
                near: mat.near,
                far: mat.far,
                ..Play::default()
            };
            self.voice = Some(m.play(clip, play));
        }
    }

    fn update(&mut self, m: &mut Mixer, table: &DriveOnTable, f: &DriveOnFrame, share: i32) {
        let Some(mat) = table.get(self.cur) else { return };
        let Some(clip) = mat.clip.clone() else { return };
        let (pitch, level) = self.work_out(mat, &clip, f.vel);
        let Some(id) = self.voice else { return };
        let gain = (level * share / 100) as f32 / 100.0;
        m.set(id, gain, pitch as f32 / own_pitch(&clip).max(1) as f32, Some(f.at));
        if !m.within(f.at, mat.far) {
            m.stop(id);
        }
    }

    /// The pitch (in pitch units) and level the loop is set to at the body's speed.
    fn work_out(&mut self, mat: &DriveOnMaterial, clip: &Clip, vel: [f32; 3]) -> (i32, i32) {
        let mph = drive_on_mph(vel);
        let own = own_pitch(clip);
        let slope = *self.slope.get_or_insert_with(|| drive_on_slope(mat, own));
        let pitch = drive_on_pitch(mat, own, slope, mph, &mut self.dice);
        if (pitch as u32) < 0x4000 {
            self.pitch = Some(pitch);
        }
        (self.pitch.unwrap_or(own), drive_on_level(mat, mph))
    }

    /// Stop the loop and forget the material and the slope.
    pub fn stop(&mut self, m: &mut Mixer) {
        if let Some(id) = self.voice.take() {
            m.stop(id);
        }
        self.slope = None;
        self.cur = 0;
    }

    /// The ride is over and the mixer stopped everything.
    pub fn hushed(&mut self) {
        self.voice = None;
        self.slope = None;
        self.cur = 0;
    }
}

// --- the shell's sound ------------------------------------------------------------------------------

/// What one seat hands the sound each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeatFrame {
    pub hands: Hands,
    /// The rider came off, for this reason.
    pub wreck: Option<i32>,
    /// A trick was landed.
    pub landed_trick: bool,
    /// In the plane, and its engine note `0..1`, where it is.
    pub plane: Option<(f32, [f32; 3])>,
    /// What the plane's wreck clank reads (P164), while in the plane.
    pub crash: PlaneCrash,
    /// What the bike's wreck clank reads (P144).
    pub bike: BikeCrash,
    /// The plane's wheels touched down this tick (P146): the body's speed then.
    pub touchdown: Option<f32>,
    /// What the shootoff's sound reads (P85).
    pub edge: EdgeFrame,
    /// What the rider's voice reads of the thrown body (P180).
    pub body: RiderBody,
    /// What the wheels' drive-on loop reads (P145).
    pub drive_on: DriveOnFrame,
    /// This seat's own pause is open (P172): everything of its own holds where it is, silent, and
    /// goes on from there when it closes, while the other seats play on.
    pub paused: bool,
}

/// One rider's sounds: the bike they ride, and what plays for them.
pub struct SeatSound {
    pub bike: Arc<BikeSounds>,
    pub engine: Engine,
    plane: PlaneNote,
    pub clank: WreckClank,
    /// The bike's `[Wreck]` clank (P144), an emitter of its own.
    pub bike_clank: WreckClank,
    /// The plane's wheels (P146).
    pub tires: TireTouchdown,
    /// The rider's own voice (P180).
    pub voice: RiderVoice,
    /// The rear wheel's loop on a prop (P145).
    pub drive_on: DriveOn,
    last: [i32; 4],
    was_grounded: bool,
    dice: Dice,
    pub edge: EdgeSound,
    /// The one-shots this rider started (their wreck's voice and clank), held with the rest by a
    /// pause.
    shots: Vec<VoiceId>,
    /// Whether this seat's voices are held for its pause.
    held: bool,
}

impl SeatSound {
    pub fn new(bike: Arc<BikeSounds>, seed: u64) -> SeatSound {
        SeatSound {
            bike,
            engine: Engine::new(seed),
            plane: PlaneNote::default(),
            clank: WreckClank::default(),
            bike_clank: WreckClank::default(),
            tires: TireTouchdown::default(),
            voice: RiderVoice::default(),
            drive_on: DriveOn::new(seed.wrapping_mul(131).wrapping_add(45)),
            last: [-1; 4],
            was_grounded: true,
            dice: Dice::new(seed.wrapping_mul(31).wrapping_add(7)),
            edge: EdgeSound::default(),
            shots: Vec::new(),
            held: false,
        }
    }

    /// Every voice of this rider's sounding now: the engine note's, the plane's, the shootoff's and
    /// the one-shots'.
    pub fn voices(&self) -> Vec<VoiceId> {
        let mut out: Vec<VoiceId> = self.engine.voices().collect();
        out.extend(&self.plane.voices);
        out.extend(self.tires.voice());
        out.extend(self.edge.voices());
        out.extend(self.drive_on.voice());
        out.extend(&self.shots);
        out
    }

    /// Hold them all where they are for the seat's pause (P172), or let them go on from there.
    fn hold(&mut self, m: &mut Mixer, on: bool) {
        self.shots.retain(|&v| m.playing(v));
        for v in self.voices() {
            m.hold(v, on);
        }
        self.held = on;
    }
}

/// **Everything the shell plays**: the sounds read off the disc for this ride, a [`SeatSound`] per
/// local rider, and the mixer they go into.
pub struct Sound {
    pub mixer: Arc<Mutex<Mixer>>,
    pub seats: Vec<SeatSound>,
    pub rider: RiderSounds,
    pub world: WorldSounds,
    pub plane: LoopEngine,
    /// `machine.aud`'s `[biplaneWreck]` (P164).
    pub plane_wreck: MachineWreck,
    /// `machine.aud`'s `[biplaneTireTouchdown]` (P146).
    pub plane_touchdown: MachineTouchdown,
    pub shootoff: Shootoff,
    /// The track's `Col\template.cmt` (P145).
    pub drive_on: DriveOnTable,
    /// The path cars' loops (V74), by car: [`Sound::set_toys`] reads them, [`Sound::toys_at`] plays them.
    pub toys: Vec<Vec<ToySound>>,
    started: bool,
    world_voices: Vec<VoiceId>,
    /// `(car, voice)` for every toy loop sounding, once [`Sound::toys_at`] has started them.
    toy_voices: Vec<(usize, VoiceId)>,
    toys_started: bool,
    dice: Dice,
    last: [i32; 3],
}

impl Sound {
    pub fn new(mixer: Arc<Mutex<Mixer>>) -> Sound {
        Sound {
            mixer,
            seats: Vec::new(),
            rider: RiderSounds::default(),
            world: WorldSounds::default(),
            plane: LoopEngine::default(),
            plane_wreck: MachineWreck::default(),
            plane_touchdown: MachineTouchdown::default(),
            shootoff: Shootoff::default(),
            drive_on: DriveOnTable::default(),
            toys: Vec::new(),
            started: false,
            world_voices: Vec::new(),
            toy_voices: Vec::new(),
            toys_started: false,
            dice: Dice::new(0x5eed),
            last: [-1; 3],
        }
    }

    /// Set the listeners: one per local view.
    pub fn listen(&mut self, ears: &[Listener]) {
        if let Ok(mut m) = self.mixer.lock() {
            m.listeners = ears.to_vec();
        }
    }

    /// One tick of every seat, `dt` long: the ride's own, which is the original's 30 Hz frame (P52).
    pub fn frame(&mut self, frames: &[SeatFrame], dt: f32) {
        let Ok(mut m) = self.mixer.lock() else { return };
        let m = &mut *m;
        if !self.started {
            self.started = true;
            for (clip, at, looping, near, far) in &self.world.fixed {
                let play = Play { looping: *looping, at: Some(*at), near: *near, far: *far, ..Play::default() };
                self.world_voices.push(m.play(clip.clone(), play));
            }
            if let Some((clip, _)) = self.world.crowd_bed.pick(&mut self.dice, &mut self.last[0]) {
                self.world_voices.push(m.play(clip, Play { gain: 0.5, looping: true, bus: Bus::Crowd, ..Play::default() }));
            }
        }
        for (clip, cycle, percent) in &self.world.random {
            // `CycleTime` read as frames: a roll of `RandomTriggerPercent` every that many.
            if self.dice.unit() * cycle.max(1.0) < 1.0 && self.dice.unit() * 100.0 < *percent {
                m.play(clip.clone(), Play::default());
            }
        }
        for (s, f) in self.seats.iter_mut().zip(frames) {
            let h = &f.hands;
            // A seat's own pause (P172): its sounds hold where they are, and nothing of it moves on —
            // not the note's machine, not the shootoff's reasons, not the landing — until it closes.
            if f.paused {
                s.hold(m, true);
                continue;
            }
            if s.held {
                s.hold(m, false);
            }
            if let Some((note, at)) = f.plane {
                s.engine.silence(m);
                s.edge.stop_rumble(m);
                s.drive_on.stop(m);
                // The wreck stops the engine sound and the unwreck (the respawn, or the L1+R1 reset)
                // starts it again (P187): silent from the wreck tick, back at the note the flight
                // gives it — idle, on the reset's closed throttle — on the first tick after.
                if f.crash.wrecked {
                    s.plane.silence(m);
                } else {
                    s.plane.update(m, &self.plane, note, at);
                }
                s.clank.tick(m, &self.plane_wreck, &f.crash, at, dt, &mut s.dice);
                s.tires.tick(m, &self.plane_touchdown, f.touchdown, at, &mut s.dice);
                continue;
            }
            s.edge.tick(m, &self.shootoff, &f.edge);
            s.plane.silence(m);
            s.engine.update(h, dt, m, &s.bike);
            s.drive_on.tick(m, &self.drive_on, &f.drive_on);
            let near = Play { at: Some(h.at), near: self.rider.near, far: self.rider.far, ..Play::default() };
            let busy = s.voice.busy(m, f.body.at, dt);
            if let Some(reason) = f.wreck {
                // The rider's voice by the wreck reason, as `game_sim::wreck_sound` sorts them: a fall
                // off the front or back is `[Fall]`, the rest `[IMPACT]`.
                let list = match reason {
                    1..=7 | 12 => Some((&self.rider.impact, 0)),
                    8..=11 | 14 => Some((&self.rider.fall, 1)),
                    _ => None,
                };
                if let Some((list, k)) = list {
                    if let Some((clip, _)) = list.pick(&mut s.dice, &mut s.last[k]) {
                        let id = m.play(clip, near);
                        s.voice.voice = Some(id);
                        s.shots.push(id);
                    }
                }
                // The bike's clank, by the entry's own call (P144): only at `MinImpulse` and over.
                if let Some(id) = s.bike_clank.entry(m, &s.bike.wreck, f.bike.entry_impulse, f.bike.at, dt, &mut s.dice) {
                    s.shots.push(id);
                }
                s.shots.retain(|&v| m.playing(v));
                if let Some((clip, _)) = self.world.oh.pick(&mut self.dice, &mut self.last[1]) {
                    m.play(clip, Play { gain: 0.6, bus: Bus::Crowd, ..Play::default() });
                }
            }
            // The bike's clank on the wreck body's hits (P144), every tick but the entry's: the
            // reason's copy it gates on is set a tick after the reason.
            if f.wreck.is_none() {
                if let Some(id) = s.bike_clank.tick(m, &s.bike.wreck, &f.bike.crash, f.bike.at, dt, &mut s.dice) {
                    s.shots.push(id);
                }
            }
            // The rider's voice on the thrown body's touches (P180): `[IMPACT]`'s no-repeat pick, the
            // same one the entry's `[IMPACT]` reasons draw from.
            let b = &f.body;
            let speed = b.speed.max(s.voice.was);
            s.voice.was = if b.down { b.speed } else { 0.0 };
            if f.wreck.is_none() && !busy && b.down && b.touching && self.rider.min_impulse < speed {
                if let Some((clip, _)) = self.rider.impact.pick(&mut s.dice, &mut s.last[0]) {
                    let id = m.play(clip, Play { at: Some(b.at), ..near });
                    s.voice.voice = Some(id);
                    s.shots.push(id);
                }
            }
            if h.grounded && !s.was_grounded && f.wreck.is_none() {
                if f.landed_trick {
                    if let Some((clip, _)) = self.world.cheer.pick(&mut self.dice, &mut self.last[2]) {
                        m.play(clip, Play { gain: 0.6, bus: Bus::Crowd, ..Play::default() });
                    }
                }
            }
            s.was_grounded = h.grounded;
        }
    }

    /// The path cars' loops (V74) where the cars are this tick, `at[k]` car `k`'s place in the order
    /// [`Sound::toys`] has them. The first call after the load (or after [`Sound::quiet`]) starts
    /// every enabled one, as the original's are started when the track loads; each later call moves
    /// them with their cars. A car missing from `at` keeps its loops where they were.
    pub fn toys_at(&mut self, at: &[[f32; 3]]) {
        let Ok(mut m) = self.mixer.lock() else { return };
        if !self.toys_started {
            self.toys_started = true;
            for (k, sounds) in self.toys.iter().enumerate() {
                let Some(&p) = at.get(k) else { continue };
                for t in sounds.iter().filter(|t| t.enabled) {
                    let play = Play { looping: t.looping, at: Some(p), near: t.near, far: t.far, ..Play::default() };
                    self.toy_voices.push((k, m.play(t.clip.clone(), play)));
                }
            }
            return;
        }
        for &(k, id) in &self.toy_voices {
            if let Some(&p) = at.get(k) {
                m.set(id, 1.0, 1.0, Some(p));
            }
        }
    }

    /// A player joined the running ride (P76): a seat of sounds on the first player's bike.
    pub fn seat_joined(&mut self) {
        let Some(bike) = self.seats.first().map(|s| s.bike.clone()) else { return };
        let n = self.seats.len() as u64;
        self.seats.push(SeatSound::new(bike, 0x9e37_79b9 ^ n));
    }

    /// Player `i` left the running ride (P76): their engine stops, and the seats after move up.
    pub fn seat_left(&mut self, i: usize) {
        if i >= self.seats.len() {
            return;
        }
        let mut s = self.seats.remove(i);
        if let Ok(mut m) = self.mixer.lock() {
            s.engine.silence(&mut m);
            s.plane.silence(&mut m);
            for id in s.clank.voice().into_iter().chain(s.bike_clank.voice()) {
                m.stop(id);
            }
            if let Some(id) = s.tires.voice() {
                m.stop(id);
            }
            if let Some(id) = s.voice.voice() {
                m.stop(id);
            }
            s.drive_on.stop(&mut m);
        }
    }

    /// The ride is over: stop every effect, keep the music.
    pub fn quiet(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.stop_all();
        }
        self.started = false;
        self.world_voices.clear();
        self.toys_started = false;
        self.toy_voices.clear();
        for s in &mut self.seats {
            s.engine = Engine::new(s.dice.below(u32::MAX) as u64);
            s.plane = PlaneNote::default();
            s.shots.clear();
            s.held = false;
            s.clank.hushed();
            s.bike_clank.hushed();
            s.tires.hushed();
            s.voice.hushed();
            s.drive_on.hushed();
            // A pause in a cannonball must not boom again on the way out: the rider's reasons stay.
            s.edge.hushed();
        }
    }
}

impl Drop for Sound {
    /// The mixer outlives the ride (the menus' music plays on): leave it at the settings' own volume.
    /// A pause keeps the fade, as the original's does.
    fn drop(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music_edge = None;
        }
    }
}

/// Read a ride's sounds off the disc: the bikes' `.aud`s by archive, the rider, the machines, the
/// track and the race events.
pub struct Reading<'a> {
    pub lib: &'a mut Library,
}

impl Reading<'_> {
    fn text(&mut self, res: &str, member: &str) -> String {
        self.lib.text(res, member).unwrap_or_else(|| {
            println!("sound: no {member} in {res}");
            String::new()
        })
    }

    /// A bike's sounds out of its archive (`bikes\…\race250p.res` holds `aud\vehicle_250a.aud`).
    pub fn bike(&mut self, res: &str) -> BikeSounds {
        let Some(member) = self.lib.disc.res(res).ok().and_then(|r| {
            r.entries()
                .iter()
                .find(|e| {
                    let n = e.name.to_ascii_lowercase();
                    n.contains("vehicle_") && n.ends_with(".aud")
                })
                .map(|e| e.name.clone())
        }) else {
            println!("sound: no vehicle_*.aud in {res}");
            return BikeSounds::default();
        };
        let text = self.text(res, &member);
        let lib = &mut *self.lib;
        BikeSounds::read(&text, &mut |n| lib.clip(n))
    }

    /// **The wreck emitters' lengths for the simulation** (S173): the rider's `[Fall]` and `[IMPACT]`
    /// clips and the bike's `[Wreck]` ones, each list's in order, with the two `MinImpulse`s. The
    /// sim's emitters pick only while idle, and how long a pick keeps them busy is its clip's length,
    /// so every machine of a ride reads these whether its sound is on or not.
    pub fn wreck_kit(&mut self, bike: &str, rider: Option<&str>) -> game_sim::wreck_sound::Kit {
        let lengths = |l: &List| l.sounds.iter().map(|(c, _)| F(c.seconds())).collect::<Vec<_>>();
        let mut kit = game_sim::wreck_sound::Kit::default();
        if let Some(res) = rider {
            let r = self.rider(res);
            (kit.fall, kit.crash, kit.voice_min) = (lengths(&r.fall), lengths(&r.impact), F(r.min_impulse));
        }
        let member = self.lib.disc.res(bike).ok().and_then(|r| {
            r.entries()
                .iter()
                .find(|e| {
                    let n = e.name.to_ascii_lowercase();
                    n.contains("vehicle_") && n.ends_with(".aud")
                })
                .map(|e| e.name.clone())
        });
        if let Some(member) = member {
            let text = self.text(bike, &member);
            let lib = &mut *self.lib;
            let w = MachineWreck::read(&text, "", &mut |n| lib.clip(n));
            (kit.clank, kit.clank_min) = (lengths(&w.list), F(w.min_impulse));
        }
        kit
    }

    pub fn rider(&mut self, res: &str) -> RiderSounds {
        let text = self.text(res, "aud\\rider.aud");
        let lib = &mut *self.lib;
        RiderSounds::read(&text, &mut |n| lib.clip(n))
    }

    /// `res` is the machines' archive (`machine.res`).
    pub fn machines(&mut self, res: &str) -> LoopEngine {
        let cars = self.text(res, "aud\\carengine.aud");
        let lib = &mut *self.lib;
        LoopEngine::read(&cars, "biplane", &mut |n| lib.clip(n))
    }

    /// The plane's wreck clank (P164): `machine.aud`'s `[biplaneWreck]` in the machines' archive.
    /// A track's drive-on materials, out of its archive's `Col\template.cmt` (P145).
    pub fn drive_on(&mut self, track_res: &str) -> DriveOnTable {
        let text = self.text(track_res, "col\\template.cmt");
        let lib = &mut *self.lib;
        DriveOnTable::read(&text, &mut |n| lib.clip(n))
    }

    pub fn plane_wreck(&mut self, res: &str) -> MachineWreck {
        let text = self.text(res, "aud\\machine.aud");
        let lib = &mut *self.lib;
        MachineWreck::read(&text, "biplane", &mut |n| lib.clip(n))
    }

    /// The plane's wheels (P146): `machine.aud`'s `[biplaneTireParams]` and `[biplaneTireTouchdown]`.
    pub fn plane_touchdown(&mut self, res: &str) -> MachineTouchdown {
        let text = self.text(res, "aud\\machine.aud");
        let lib = &mut *self.lib;
        MachineTouchdown::read(&text, "biplane", &mut |n| lib.clip(n))
    }

    /// `track` is the track's name (`free10`): its `.aud` is in `tracks\free10.res`, the race's events
    /// in `tracks\free10_2.res`.
    pub fn world(&mut self, track_res: &str, events_res: &str, track: &str) -> WorldSounds {
        let t = self.text(track_res, &format!("tracks\\{track}\\{track}.aud"));
        let e = self.text(events_res, "aud\\event.aud");
        let lib = &mut *self.lib;
        WorldSounds::read(&t, &e, &mut |n| lib.clip(n))
    }

    /// A track's own `.aud` alone, its emitters without the race events' (P71's other maps).
    pub fn track_only(&mut self, track_res: &str, track: &str) -> WorldSounds {
        let t = self.text(track_res, &format!("tracks\\{track}\\{track}.aud"));
        let lib = &mut *self.lib;
        WorldSounds::read(&t, "", &mut |n| lib.clip(n))
    }

    /// The path cars' loops (V74): `names[k]` is car `k`'s `Sound1..n`, looked up in the track's own
    /// `.toy` beside its `.aud`.
    pub fn toys(&mut self, track_res: &str, track: &str, names: &[Vec<String>]) -> Vec<Vec<ToySound>> {
        if names.iter().all(|n| n.is_empty()) {
            return vec![Vec::new(); names.len()];
        }
        let t = self.text(track_res, &format!("tracks\\{track}\\{track}.toy"));
        let lib = &mut *self.lib;
        ToySound::cars(&t, names, &mut |n| lib.clip(n))
    }

    /// The shootoff's boom and rumble, out of the race events' `aud\event.aud`.
    pub fn shootoff(&mut self, events_res: &str) -> Shootoff {
        let e = self.text(events_res, "aud\\event.aud");
        let lib = &mut *self.lib;
        Shootoff::read(&e, &mut |n| lib.clip(n))
    }
}

// --- the device ---------------------------------------------------------------------------------------

/// The desktop's own players, tried in order; each takes raw 48 kHz 16-bit stereo on its stdin.
const PLAYERS: [(&str, &[&str]); 3] = [
    ("pw-cat", &["--playback", "--raw", "--format", "s16", "--rate", "48000", "--channels", "2", "-"]),
    ("pacat", &["--raw", "--format=s16le", "--rate=48000", "--channels=2"]),
    ("aplay", &["-q", "-f", "S16_LE", "-r", "48000", "-c", "2", "-t", "raw"]),
];

/// How far ahead of the wall clock the mix is written: the pipe and the player's own buffer hold
/// this much, which is the effects' latency.
const LEAD: f64 = 0.06;

/// **The sound going out**: a thread that renders the mixer as the wall clock goes and writes it to a
/// player's stdin. Dropping it stops both.
pub struct Output {
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    pub player: String,
}

impl Output {
    #[cfg_attr(target_arch = "wasm32", allow(unreachable_code))]
    pub fn start(mixer: Arc<Mutex<Mixer>>) -> Result<Output, String> {
        // The browser (P196): no players and no thread; the page's WebAudio, kept fed a frame at a
        // time ([`crate::web_host::next_frame`]) for as long as the mixer lives.
        #[cfg(target_arch = "wasm32")]
        {
            crate::web_host::sound_from(Arc::downgrade(&mixer));
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            return Ok(Output { stop, thread: None, player: "the page's WebAudio".into() });
        }
        let mut child = None;
        for (name, args) in PLAYERS {
            let spawned = std::process::Command::new(name)
                .args(args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if let Ok(c) = spawned {
                child = Some((name.to_string(), c));
                break;
            }
        }
        let (player, mut child) = child.ok_or("no pw-cat, pacat or aplay to play sound through")?;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let quit = stop.clone();
        let mut pipe = child.stdin.take().ok_or("the player has no stdin")?;
        let thread = std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let mut written: u64 = 0;
            let mut buf = vec![0i16; 1024 * 2];
            let mut bytes = Vec::with_capacity(buf.len() * 2);
            while !quit.load(std::sync::atomic::Ordering::Relaxed) {
                let due = ((t0.elapsed().as_secs_f64() + LEAD) * RATE as f64) as u64;
                // Fell far behind (a stall): skip ahead rather than play the backlog.
                if due > written + RATE as u64 / 2 {
                    written = due - (LEAD * RATE as f64) as u64;
                }
                while written < due {
                    let n = ((due - written) as usize).min(1024);
                    let out = &mut buf[..n * 2];
                    match mixer.lock() {
                        Ok(mut m) => m.render(out),
                        Err(_) => return,
                    }
                    bytes.clear();
                    for s in out.iter() {
                        bytes.extend_from_slice(&s.to_le_bytes());
                    }
                    if pipe.write_all(&bytes).is_err() {
                        return;
                    }
                    written += n as u64;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            drop(pipe);
            let _ = child.kill();
            let _ = child.wait();
        });
        Ok(Output { stop, thread: Some(thread), player })
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The user's own music folder: `music/` beside the settings.
pub fn folder(settings_dir: &Path) -> PathBuf {
    settings_dir.join("music")
}

/// Every music source this shell has: the disc's (when there is a disc), then the folder's.
pub fn sources(disc: Option<(&mut Disc, PathBuf)>, settings_dir: &Path) -> Vec<Arc<dyn MusicSource>> {
    let mut out: Vec<Arc<dyn MusicSource>> = Vec::new();
    if let Some((disc, image)) = disc {
        match DiscMusic::read(disc, image) {
            Ok(m) => out.push(Arc::new(m)),
            Err(e) => println!("music: the disc's songs: {e}"),
        }
    }
    out.push(Arc::new(FolderMusic::scan(&folder(settings_dir))));
    out
}

/// A slider's value (`0..=100`) as a gain: the original's, which hands the sound system `value / 100`.
pub fn volume(value: u8) -> f32 {
    value.min(100) as f32 / 100.0
}

/// Every slider of the original's Game Audio Options page (P56), `0..=100` each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Levels {
    pub music: u8,
    /// The user's own folder: the original's "User Soundtracks" (its ripped music).
    pub folder: u8,
    pub effects: u8,
    pub crowd: u8,
    pub engine: u8,
    pub menu: u8,
}

/// **The shell's sound**, which outlives a ride under the front end: the mixer with the music in it,
/// and the player it goes out through, if there is one (none when `out` is false — a headless run, or
/// `--no-sound` — and then nothing is rendered at all).
pub struct Speakers {
    pub mixer: Arc<Mutex<Mixer>>,
    _out: Option<Output>,
    /// The menus' clicks (P65), once read off the disc.
    menu: Option<MenuSounds>,
}

impl Speakers {
    pub fn start(music: Music, out: bool) -> Speakers {
        let mixer = Arc::new(Mutex::new(Mixer::new(music)));
        let out = match out {
            false => None,
            true => match Output::start(mixer.clone()) {
                Ok(o) => {
                    println!("sound: through {}", o.player);
                    Some(o)
                }
                Err(e) => {
                    println!("sound: {e} — the shell is silent");
                    None
                }
            },
        };
        Speakers { mixer, _out: out, menu: None }
    }

    /// The front end's sounds off the disc (P65): its loop into the mixer, its clicks kept here.
    pub fn menu_sounds(&mut self, sounds: MenuSounds) {
        if let Ok(mut m) = self.mixer.lock() {
            m.menu_music = sounds.music.clone();
        }
        self.menu = Some(sounds);
    }

    /// Whether the front end is up (P65): the menu's loop in place of the ride's songs.
    pub fn front_end(&self, on: bool) {
        if let Ok(mut m) = self.mixer.lock()
            && m.front_end != on
        {
            m.front_end = on;
        }
    }

    /// One of the menus' clicks, when anything renders the mix.
    pub fn click(&self, click: Click) {
        let (Some(menu), Some(mixer)) = (&self.menu, self.live()) else { return };
        if let Ok(mut m) = mixer.lock() {
            menu.play(&mut m, click);
        }
    }

    /// The mixer, if anything renders it: a voice started in one nobody renders never ends.
    pub fn live(&self) -> Option<Arc<Mutex<Mixer>>> {
        self._out.as_ref().map(|_| self.mixer.clone())
    }

    /// The settings' music pick (`off`, `shuffle`, `order` or a track's id) and the sliders.
    pub fn apply(&self, pick: &str, levels: Levels) {
        let Ok(mut m) = self.mixer.lock() else { return };
        let want = Pick::parse(pick);
        if *m.music.pick() != want {
            m.music.set_pick(want);
        }
        m.music_volume = volume(levels.music);
        m.folder_volume = volume(levels.folder);
        m.effects_volume = volume(levels.effects);
        m.crowd_volume = volume(levels.crowd);
        m.engine_volume = volume(levels.engine);
        m.menu_volume = volume(levels.menu);
    }

    /// The mixer whether or not anything renders it, for a test to listen to.
    #[allow(dead_code)]
    pub fn mixer(&self) -> &Arc<Mutex<Mixer>> {
        &self.mixer
    }

    /// Play a song now (the pause's song row, P56), the music held or not (P174).
    pub fn play_now(&self, id: &str) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music.hold(false);
            m.music.play_now(id);
        }
    }

    /// On to the next song (the pause's Skip Song), playing if it was held (P174).
    pub fn skip(&self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music.hold(false);
            m.music.next();
        }
    }

    /// Back to the song before (P174's pause player), playing if it was held.
    pub fn back(&self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music.hold(false);
            m.music.back();
        }
    }

    /// Hold the music where it is, or let it play on (P174's pause player).
    pub fn hold(&self, on: bool) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music.hold(on);
        }
    }

    /// Whether the music is held.
    pub fn held(&self) -> bool {
        self.mixer.lock().is_ok_and(|m| m.music.held())
    }

    /// The song playing, by id.
    pub fn now(&self) -> Option<String> {
        self.mixer.lock().ok()?.music.now().map(str::to_string)
    }

    /// Every track the sources have, as the settings' choices: `(id, label)`.
    pub fn choices(&self) -> Vec<(String, String)> {
        match self.mixer.lock() {
            Ok(m) => m.music.tracks().iter().map(|t| (t.id.clone(), t.label())).collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Sound {
    /// A ride's sounds off the disc: one bike `.aud` for each seat's bike archive (`bikes`), the
    /// rider's voice, the machines, and the track's emitters with the race events'.
    pub fn read(
        mixer: Arc<Mutex<Mixer>>,
        disc: Disc,
        bikes: &[String],
        rider: Option<&str>,
        machines: &str,
        track_res: &str,
        events_res: Option<&str>,
        track: &str,
    ) -> Result<Sound, String> {
        let mut lib = Library::open(disc)?;
        let mut read = Reading { lib: &mut lib };
        let mut sound = Sound::new(mixer);
        let mut by_archive: Vec<(String, Arc<BikeSounds>)> = Vec::new();
        for (i, res) in bikes.iter().enumerate() {
            let bike = match by_archive.iter().find(|(r, _)| r == res) {
                Some((_, b)) => b.clone(),
                None => {
                    let b = Arc::new(read.bike(res));
                    by_archive.push((res.clone(), b.clone()));
                    b
                }
            };
            sound.seats.push(SeatSound::new(bike, 0x9e37_79b9 ^ i as u64));
        }
        if let Some(rider) = rider {
            sound.rider = read.rider(rider);
        }
        sound.plane = read.machines(machines);
        sound.plane_wreck = read.plane_wreck(machines);
        sound.plane_touchdown = read.plane_touchdown(machines);
        sound.world = read.world(track_res, events_res.unwrap_or(track_res), track);
        sound.shootoff = read.shootoff(events_res.unwrap_or(track_res));
        sound.drive_on = read.drive_on(track_res);
        Ok(sound)
    }
}

/// [`Reading::wreck_kit`] off the disc: what each sim rider's wreck emitters are loaded with (S173).
pub fn wreck_kit(disc: Disc, bike: &str, rider: Option<&str>) -> Result<game_sim::wreck_sound::Kit, String> {
    let mut lib = Library::open(disc)?;
    Ok(Reading { lib: &mut lib }.wreck_kit(bike, rider))
}

/// One map of a world of disc tracks (P71), as its sound emitters are read: its archive
/// (`tracks\\free07.res`), its name (`free07`), its offset in the world, and the world rectangle it
/// owns, `[x0, z0, x1, z1]`.
pub struct MapSounds {
    pub res: String,
    pub track: String,
    pub at: [f32; 2],
    pub owned: [f32; 4],
}

impl Sound {
    /// The path cars' loops (V74), read off the disc for the track's cars: `names[k]` is car `k`'s
    /// `Sound1..n` (`game_render::toy::Car::sounds`), in the order the ride drives them.
    pub fn set_toys(&mut self, disc: Disc, track_res: &str, track: &str, names: &[Vec<String>]) -> Result<(), String> {
        let mut lib = Library::open(disc)?;
        let mut read = Reading { lib: &mut lib };
        self.toys = read.toys(track_res, track, names);
        let n: usize = self.toys.iter().map(Vec::len).sum();
        if n > 0 {
            println!("sound: {n} toy loops on {} cars", self.toys.iter().filter(|t| !t.is_empty()).count());
        }
        Ok(())
    }

    /// A world of disc tracks' fixed emitters (P71): every map's own `.aud` ones, each moved by its
    /// map's offset, and those standing on ground the map no longer owns left out. They take the
    /// place of the home map's, which is among `maps`. The random ones and the crowd stay the home
    /// map's: they are not placed, and every map's at once would sound each one five times as often.
    pub fn set_map_emitters(&mut self, disc: Disc, maps: &[MapSounds]) -> Result<(), String> {
        let mut lib = Library::open(disc)?;
        let mut read = Reading { lib: &mut lib };
        let mut fixed = Vec::new();
        let mut each = Vec::new();
        for m in maps {
            let w = read.track_only(&m.res, &m.track);
            let had = fixed.len();
            let [x0, z0, x1, z1] = m.owned;
            fixed.extend(w.fixed.into_iter().map(|(clip, p, looping, near, far)| (clip, [p[0] + m.at[0], p[1], p[2] + m.at[1]], looping, near, far)).filter(
                |(_, p, ..)| p[0] >= x0 && p[0] < x1 && p[2] >= z0 && p[2] < z1,
            ));
            each.push(format!("{} {}", m.track, fixed.len() - had));
        }
        println!("sound: {} fixed emitters on {} maps ({})", fixed.len(), maps.len(), each.join(", "));
        self.world.fixed = fixed;
        Ok(())
    }
}

// --- the front end's sounds (P65) -------------------------------------------------------------------

/// The menus' clicks, in the order `aud\ui\ui.aud`'s `[UIResponseEmitter]` lists them (`Sound1` to
/// `Sound5`), which is the number the original's menus play them by: a row moved up or down, a
/// sideways step (a choice, a slider, a tab), a confirm, a back, and a press refused (a slider at its
/// end, a row with nothing to choose).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    UpDown = 1,
    LeftRight = 2,
    Accept = 3,
    Cancel = 4,
    Locked = 5,
}

/// The `.res` and member the menus' `.aud` is in (the ride UI archives carry the same file).
pub const UI_RES: &str = "UI\\ui.res";
pub const UI_AUD: &str = "aud\\ui\\ui.aud";

/// The front end's music: two mono halves under `Aud\UI\`, the name the original's menu manager
/// asks for with `l` and `r` after it.
pub const MENU_MUSIC: &str = "UI\\UI_001";

/// **The front end's sounds**: its music loop and its clicks.
#[derive(Clone, Debug, Default)]
pub struct MenuSounds {
    pub music: Option<Arc<Clip>>,
    /// [`Click`]'s sounds, `Sound1` first.
    pub clicks: Vec<Option<Arc<Clip>>>,
}

impl MenuSounds {
    /// Off the sound archive: `ui.aud`'s `[UIResponseEmitter]` (`text`) and the music's two halves.
    pub fn read(text: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> MenuSounds {
        let sections = ini::parse(text);
        let clicks = match section(&sections, "UIResponseEmitter") {
            Some(s) => (1..=Click::Locked as usize).map(|i| s.get(&format!("Sound{i}")).and_then(|n| lib(n))).collect(),
            None => Vec::new(),
        };
        let music = match (lib(&format!("{MENU_MUSIC}l")), lib(&format!("{MENU_MUSIC}r"))) {
            (Some(l), Some(r)) => Some(Arc::new(stereo(&l, &r))),
            (Some(one), None) | (None, Some(one)) => Some(one),
            (None, None) => None,
        };
        MenuSounds { music, clicks }
    }

    /// Off the disc.
    pub fn load(disc: Disc) -> Result<MenuSounds, String> {
        let mut lib = Library::open(disc)?;
        let text = lib.text(UI_RES, UI_AUD).ok_or_else(|| format!("no {UI_AUD} in {UI_RES}"))?;
        Ok(MenuSounds::read(&text, &mut |n| lib.clip(n)))
    }

    /// The click's sound.
    pub fn clip(&self, click: Click) -> Option<&Arc<Clip>> {
        self.clicks.get(click as usize - 1)?.as_ref()
    }

    /// Start a click under the Menu FX slider, heard the same by everyone.
    pub fn play(&self, mixer: &mut Mixer, click: Click) -> Option<VoiceId> {
        let clip = self.clip(click)?.clone();
        Some(mixer.play(clip, Play { bus: Bus::Menu, ..Play::default() }))
    }
}

/// Two mono clips as one stereo one, at the left's rate, the shorter padded with silence.
pub fn stereo(l: &Clip, r: &Clip) -> Clip {
    let n = l.frames().max(r.frames());
    let at = |c: &Clip, i: usize| c.pcm.get(i * c.channels.max(1) as usize).copied().unwrap_or(0);
    let mut pcm = Vec::with_capacity(n * 2);
    for i in 0..n {
        pcm.push(at(l, i));
        pcm.push(at(r, i));
    }
    Clip { rate: l.rate, channels: 2, pcm }
}
