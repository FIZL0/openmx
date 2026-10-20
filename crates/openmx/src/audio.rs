//! Sound (P42): the original's own engine notes, wrecks, landings, crowd, world emitters and music,
//! read off the user's disc and mixed by the shell.
//!
//! # Where the sound is on the disc
//!
//! All of it is one member of `MX.PSR`, `Aud\audio.res`, which is not the deflate archive every
//! other `.res` is. Its front is three words (`3`, the directory's place less 8, `0x100`), then at that
//! place one record per file: `[u32 offset][u32 size][u32 name length][name]`, the name with no
//! terminator (`Aud\Engine\250A\idle02.VAG`). The first record's offset word is the count of
//! records. The files follow the directory, each at a multiple of `0x2000`, the offsets counted from
//! the first such multiple after the directory.
//!
//! Two kinds of file, both Sony's 4-bit ADPCM ([`decode_block`]: 16 bytes in, 28 samples out):
//!
//! - **`.VAG`**, one mono sound: a `0x30`-byte header (its `VAGp` magic is zeroed on this disc) with
//!   the big-endian data size at `0x0c` and the rate at `0x10`, nearly always 22050.
//! - **`.vig`**, a stereo stream (the music, the start-line ditties): a `0x800`-byte header with the
//!   per-channel size at `0x0c` and the rate at `0x10` (44100), then the two channels interleaved one
//!   16-byte block at a time. `Crowd\cheer.vig` is the odd one: a mono VAG under the other extension.
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

    /// A `.vig` file: the `0x800`-byte header and stereo ADPCM, or (one file on the disc) a VAG under
    /// that name, told apart by which header's size fits the file.
    pub fn vig(bytes: &[u8]) -> Result<Clip, String> {
        if bytes.len() < 0x40 {
            return Err(format!("a {}-byte vig", bytes.len()));
        }
        let size = be32(bytes, 0x0c) as usize;
        if 0x800 + 2 * size > bytes.len() && 0x30 + size <= bytes.len() {
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
        // The count is the first record's offset word: that record (`SFX\RumbleMono.VAG`, which
        // nothing here plays) has no offset of its own.
        let count = le32(front, dir).ok_or("the sound archive's directory is past its front")? as usize;
        let mut at = dir;
        let mut raw = Vec::with_capacity(count);
        for _ in 0..count {
            let (off, size, len) = match (le32(front, at), le32(front, at + 4), le32(front, at + 8)) {
                (Some(o), Some(s), Some(l)) => (o, s, l as usize),
                _ => return Err("the sound archive's directory runs off its front".to_string()),
            };
            let name = front.get(at + 12..at + 12 + len).ok_or("a name past the front")?;
            raw.push((off as u64, size as u64, ini::text(name)));
            at += 12 + len;
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
}

impl Default for Play {
    fn default() -> Play {
        Play { gain: 1.0, pitch: 1.0, looping: false, at: None, near: 30.0, far: 150.0 }
    }
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
    /// `0..=1`, the settings' two sliders.
    pub music_volume: f32,
    pub effects_volume: f32,
    /// Samples rendered since the start.
    pub rendered: u64,
}

impl Mixer {
    pub fn new(music: Music) -> Mixer {
        Mixer {
            voices: Vec::new(),
            next: 1,
            listeners: vec![Listener::default()],
            music,
            music_volume: 0.8,
            effects_volume: 0.8,
            rendered: 0,
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

    /// A voice's left and right gains for the listeners as they stand: the nearest listener's distance
    /// and side, and no side at all when more than one is listening (split screen has no one left or
    /// right).
    fn gains(&self, p: &Play) -> [f32; 2] {
        let g = p.gain * self.effects_volume;
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

    /// Mix `out.len() / 2` stereo frames.
    pub fn render(&mut self, out: &mut [i16]) {
        let frames = out.len() / 2;
        let mut acc = vec![0f32; frames * 2];
        self.music.mix(&mut acc, self.music_volume);
        // About 5 ms to reach a new gain: no clicks, and a stop is gone well inside a frame.
        let ease = 1.0 - (-1.0 / (0.005 * RATE as f32)).exp();
        let targets: Vec<[f32; 2]> = self
            .voices
            .iter()
            .map(|v| if v.stopping { [0.0; 2] } else { self.gains(&v.play) })
            .collect();
        for (v, target) in self.voices.iter_mut().zip(targets) {
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
/// starts. The list's first song, College Kids, is a 28 ms stub on this disc and is left off.
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

/// What the settings ask the music to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Off,
    Shuffle,
    /// One song, over and over.
    Track(String),
}

impl Pick {
    /// The settings' word: `off`, `shuffle`, or a track's id.
    pub fn parse(word: &str) -> Pick {
        match word.trim() {
            "off" => Pick::Off,
            "" | "shuffle" => Pick::Shuffle,
            id => Pick::Track(id.to_string()),
        }
    }

    pub fn word(&self) -> String {
        match self {
            Pick::Off => "off".to_string(),
            Pick::Shuffle => "shuffle".to_string(),
            Pick::Track(id) => id.clone(),
        }
    }
}

/// **The music player**: every source's songs, the pick, and the song playing, resampled to the mix.
pub struct Music {
    sources: Vec<Arc<dyn MusicSource>>,
    pick: Pick,
    dice: Dice,
    last: Option<String>,
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
        let keep = matches!((&pick, self.now()), (Pick::Shuffle, Some(_)) if self.pick != Pick::Off);
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

    fn choose(&mut self) -> Option<String> {
        match &self.pick {
            Pick::Off => None,
            Pick::Track(id) => Some(id.clone()),
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
    fn tend(&mut self) {
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
        let (tx, rx) = std::sync::mpsc::channel();
        let want = id.clone();
        if self.wait {
            let _ = tx.send(source.open(&want));
        } else {
            std::thread::spawn(move || {
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
        if volume <= 0.0 {
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

/// The engine note's states, as the original's bike sound numbers them (1..=10; 10 is not used).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    Idle,
    LowAccel,
    HighAccel,
    Power,
    Sputter,
    Decel,
    Clutch,
    /// The free rev, the first half of `[Panic]`, cut at `RevMidpoint` into the panic loop.
    Rev,
    Panic,
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
    pub wreck: List,
    /// `LowTorqueSpeed` in feet a second (the file's mph, ×1.4667 as the loader does).
    pub low_torque_speed: f32,
    pub airborne_to_decel: f32,
    pub rev_midpoint: f32,
    pub near: f32,
    pub far: f32,
    pub wreck_near: f32,
    pub wreck_far: f32,
}

impl BikeSounds {
    pub fn read(text: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> BikeSounds {
        let s = ini::parse(text);
        let params = section(&s, "Params");
        let p = |k: &str, d: f32| params.and_then(|p| p.number(k)).unwrap_or(d);
        let wreck = section(&s, "Wreck");
        let w = |k: &str, d: f32| wreck.and_then(|p| p.number(k)).unwrap_or(d);
        BikeSounds {
            idle: List::read(section(&s, "Idle"), lib),
            low: List::read(section(&s, "LowAccel"), lib),
            high: List::read(section(&s, "HighAccel"), lib),
            power: List::read(section(&s, "Power"), lib),
            sputter: List::read(section(&s, "Sputter"), lib),
            decel: List::read(section(&s, "Decel"), lib),
            clutch: List::read(section(&s, "Clutch"), lib),
            panic: List::read(section(&s, "Panic"), lib),
            wreck: List::read(wreck, lib),
            low_torque_speed: p("LowTorqueSpeed", 25.0) * 1.466_666_6,
            airborne_to_decel: p("AirborneTimeToDecel", 0.3),
            rev_midpoint: p("RevMidpoint", 0.311),
            near: p("MinAttenuationDistance", 20.0),
            far: p("MaxAttenuationDistance", 200.0),
            wreck_near: w("MinDistance", 30.0),
            wreck_far: w("MaxDistance", 150.0),
        }
    }

    fn list(&self, n: Note) -> &List {
        match n {
            Note::Idle => &self.idle,
            Note::LowAccel => &self.low,
            Note::HighAccel => &self.high,
            Note::Power => &self.power,
            Note::Sputter => &self.sputter,
            Note::Decel => &self.decel,
            Note::Clutch => &self.clutch,
            Note::Rev | Note::Panic => &self.panic,
        }
    }
}

/// What the engine note hears from the ride each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hands {
    pub grounded: bool,
    pub gas: f32,
    pub clutch: f32,
    /// Feet a second.
    pub speed: f32,
    /// `0..1` between idle and the rev limit.
    pub rev: f32,
    pub at: [f32; 3],
}

/// **One bike's engine note**: a chain of whole samples, the next queued behind the one playing, the
/// state machine choosing which list it comes from — the original's, as the journal reads it.
pub struct Engine {
    pub note: Note,
    voice: Option<VoiceId>,
    segment: u32,
    /// What the queued sample is, once queued.
    queued: Option<Note>,
    last: [i32; 9],
    off_gas: f32,
    in_air: f32,
    was_gas: bool,
    was_grounded: bool,
    dice: Dice,
}

/// How long off the throttle before an accelerating note falls to the deceleration.
const OFF_GAS_TO_DECEL: f32 = 0.25;

fn slot(n: Note) -> usize {
    n as usize
}

fn accelerating(n: Note) -> bool {
    matches!(n, Note::LowAccel | Note::HighAccel | Note::Power | Note::Sputter)
}

impl Engine {
    pub fn new(seed: u64) -> Engine {
        Engine {
            note: Note::Idle,
            voice: None,
            segment: 0,
            queued: None,
            last: [-1; 9],
            off_gas: 0.0,
            in_air: 0.0,
            was_gas: false,
            was_grounded: true,
            dice: Dice::new(seed),
        }
    }

    /// What follows a sample that ends on its own.
    fn after(&self, n: Note, h: &Hands, sounds: &BikeSounds) -> Note {
        let next = match n {
            Note::LowAccel | Note::HighAccel | Note::Sputter | Note::Clutch => Note::Power,
            Note::Power => Note::Sputter,
            Note::Decel | Note::Idle => Note::Idle,
            Note::Rev | Note::Panic => {
                if h.gas > 0.1 { Note::Panic } else { Note::Decel }
            }
        };
        next.min_with(sounds)
    }

    /// The state the hands ask for now, if it is not the one playing.
    fn decide(&mut self, h: &Hands, dt: f32, sounds: &BikeSounds) -> Option<Note> {
        let gas = h.gas > 0.1;
        let clutch = h.clutch > 0.1;
        let pressed = gas && !self.was_gas;
        let took_off = !h.grounded && self.was_grounded;
        self.off_gas = if gas { 0.0 } else { self.off_gas + dt };
        self.in_air = if h.grounded { 0.0 } else { self.in_air + dt };
        let n = self.note;
        let free = matches!(n, Note::Rev | Note::Panic | Note::Clutch);
        if h.grounded && gas && clutch {
            let want = if h.speed > 1.0 { Note::Clutch } else { Note::Rev };
            return (!free).then_some(want);
        }
        if !h.grounded {
            if gas && (pressed || took_off) && !free {
                return Some(Note::Rev);
            }
            if !gas && self.in_air > sounds.airborne_to_decel && (accelerating(n) || free) {
                return Some(Note::Decel);
            }
            return None;
        }
        if gas {
            if matches!(n, Note::Idle | Note::Decel) || (free && !clutch) {
                return Some(if h.speed <= sounds.low_torque_speed { Note::LowAccel } else { Note::HighAccel });
            }
            return None;
        }
        if (accelerating(n) && self.off_gas > OFF_GAS_TO_DECEL) || free {
            return Some(Note::Decel);
        }
        None
    }

    fn start(&mut self, n: Note, mixer: &mut Mixer, sounds: &BikeSounds, play: Play) {
        if let Some(v) = self.voice.take() {
            mixer.stop(v);
        }
        self.queued = None;
        let n = n.min_with(sounds);
        let Some((clip, looping)) = self.choose(n, sounds) else {
            self.note = n;
            return;
        };
        self.voice = Some(mixer.play(clip, Play { looping, ..play }));
        self.segment = 0;
        self.note = n;
    }

    fn choose(&mut self, n: Note, sounds: &BikeSounds) -> Option<(Arc<Clip>, bool)> {
        match n {
            Note::Rev => sounds.panic.sounds.first().cloned(),
            Note::Panic => sounds.panic.sounds.get(1).or(sounds.panic.sounds.first()).cloned(),
            _ => sounds.list(n).pick(&mut self.dice, &mut self.last[slot(n)]),
        }
    }

    /// One frame: choose, queue, and move the voice to the bike.
    pub fn update(&mut self, h: &Hands, dt: f32, mixer: &mut Mixer, sounds: &BikeSounds) {
        // The pitch follows the revs a little — this port's, not the original's (whose bike note has
        // no pitch but its samples'): a chain of whole samples cannot follow a throttle blip inside
        // one.
        let pitch = 0.94 + 0.12 * h.rev.clamp(0.0, 1.0);
        let play = Play { gain: 0.7, pitch, looping: false, at: Some(h.at), near: sounds.near, far: sounds.far };
        if let Some(want) = self.decide(h, dt, sounds) {
            if want.min_with(sounds) != self.note {
                self.start(want, mixer, sounds, play);
            }
        }
        self.was_gas = h.gas > 0.1;
        self.was_grounded = h.grounded;
        let progress = self.voice.and_then(|v| mixer.progress(v));
        match progress {
            None => {
                // Nothing playing (a list the disc lacks, or a sample just ended with nothing queued).
                let next = if self.voice.is_some() { self.after(self.note, h, sounds) } else { self.note };
                self.voice = None;
                self.start(next, mixer, sounds, play);
            }
            Some((segment, _, queued)) => {
                if segment != self.segment {
                    self.segment = segment;
                    if let Some(n) = self.queued.take() {
                        self.note = n;
                    }
                }
                if !queued && self.queued.is_none() && !(self.note == Note::Idle || self.note == Note::Panic) {
                    let next = self.after(self.note, h, sounds);
                    if let Some((clip, looping)) = self.choose(next, sounds) {
                        let v = self.voice.unwrap_or_default();
                        let cut = (self.note == Note::Rev).then_some(sounds.rev_midpoint);
                        mixer.chain(v, clip, looping, cut);
                        self.queued = Some(next);
                    }
                }
                if let Some(v) = self.voice {
                    mixer.set(v, play.gain, pitch, play.at);
                }
            }
        }
    }

    /// Quiet the engine (the rider left the bike for the plane).
    pub fn silence(&mut self, mixer: &mut Mixer) {
        if let Some(v) = self.voice.take() {
            mixer.stop(v);
        }
        self.note = Note::Idle;
        self.queued = None;
    }

    pub fn voice(&self) -> Option<VoiceId> {
        self.voice
    }
}

impl Note {
    /// A state whose list the disc lacks falls back to the idle.
    fn min_with(self, sounds: &BikeSounds) -> Note {
        if sounds.list(self).is_empty() { Note::Idle } else { self }
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
                let play = Play { gain: 0.0, looping: true, at: Some(at), near: e.near, far: e.far, ..Play::default() };
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

// --- the rider, the landing, the world --------------------------------------------------------------

/// `rider.aud`: the voice's two wreck lists.
#[derive(Clone, Debug, Default)]
pub struct RiderSounds {
    pub fall: List,
    pub impact: List,
    pub near: f32,
    pub far: f32,
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
        }
    }
}

/// `machine.aud`'s suspension: a sound per tier of landing speed, louder the harder.
#[derive(Clone, Debug, Default)]
pub struct Landing {
    pub tiers: Vec<(f32, Arc<Clip>)>,
    pub quiet: (f32, f32),
    pub loud: (f32, f32),
}

impl Landing {
    pub fn read(text: &str, lib: &mut impl FnMut(&str) -> Option<Arc<Clip>>) -> Landing {
        let s = ini::parse(text);
        let Some(p) = s.iter().find(|s| s.name.to_ascii_lowercase().ends_with("suspension")) else {
            return Landing::default();
        };
        let n = p.number("NumSounds").unwrap_or(0.0) as usize;
        let tiers = (1..=n)
            .filter_map(|i| {
                let from = p.number(&format!("Sound{i}VelocityTierStart"))?;
                Some((from, lib(p.get(&format!("Sound{i}"))?)?))
            })
            .collect();
        Landing {
            tiers,
            quiet: (p.number("MinLevelVelocity").unwrap_or(10.0), p.number("MinLevelAttenuation").unwrap_or(50.0)),
            loud: (p.number("MaxLevelVelocity").unwrap_or(55.0), p.number("MaxLevelAttenuation").unwrap_or(100.0)),
        }
    }

    /// The sound and its gain for a landing at `speed` feet a second down, or `None` below the first
    /// tier.
    pub fn pick(&self, speed: f32) -> Option<(Arc<Clip>, f32)> {
        let (_, clip) = self.tiers.iter().rev().find(|(from, _)| speed >= *from)?;
        let t = ((speed - self.quiet.0) / (self.loud.0 - self.quiet.0).max(1e-3)).clamp(0.0, 1.0);
        Some((clip.clone(), (self.quiet.1 + (self.loud.1 - self.quiet.1) * t) / 100.0))
    }
}

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

// --- the shell's sound ------------------------------------------------------------------------------

/// What one seat hands the sound each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeatFrame {
    pub hands: Hands,
    /// Vertical speed, feet a second (down is negative).
    pub vy: f32,
    /// The rider came off, for this reason.
    pub wreck: Option<i32>,
    /// A trick was landed.
    pub landed_trick: bool,
    /// In the plane, and its engine note `0..1`, where it is.
    pub plane: Option<(f32, [f32; 3])>,
}

/// One rider's sounds: the bike they ride, and what plays for them.
pub struct SeatSound {
    pub bike: Arc<BikeSounds>,
    pub engine: Engine,
    plane: PlaneNote,
    last: [i32; 4],
    was_grounded: bool,
    last_vy: f32,
    dice: Dice,
}

impl SeatSound {
    pub fn new(bike: Arc<BikeSounds>, seed: u64) -> SeatSound {
        SeatSound {
            bike,
            engine: Engine::new(seed),
            plane: PlaneNote::default(),
            last: [-1; 4],
            was_grounded: true,
            last_vy: 0.0,
            dice: Dice::new(seed.wrapping_mul(31).wrapping_add(7)),
        }
    }
}

/// **Everything the shell plays**: the sounds read off the disc for this ride, a [`SeatSound`] per
/// local rider, and the mixer they go into.
pub struct Sound {
    pub mixer: Arc<Mutex<Mixer>>,
    pub seats: Vec<SeatSound>,
    pub rider: RiderSounds,
    pub landing: Landing,
    pub world: WorldSounds,
    pub plane: LoopEngine,
    started: bool,
    world_voices: Vec<VoiceId>,
    dice: Dice,
    last: [i32; 3],
}

/// The frame the shell steps at.
const FRAME: f32 = 1.0 / 60.0;

impl Sound {
    pub fn new(mixer: Arc<Mutex<Mixer>>) -> Sound {
        Sound {
            mixer,
            seats: Vec::new(),
            rider: RiderSounds::default(),
            landing: Landing::default(),
            world: WorldSounds::default(),
            plane: LoopEngine::default(),
            started: false,
            world_voices: Vec::new(),
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

    /// One frame of every seat.
    pub fn frame(&mut self, frames: &[SeatFrame]) {
        let Ok(mut m) = self.mixer.lock() else { return };
        let m = &mut *m;
        if !self.started {
            self.started = true;
            for (clip, at, looping, near, far) in &self.world.fixed {
                let play = Play { looping: *looping, at: Some(*at), near: *near, far: *far, ..Play::default() };
                self.world_voices.push(m.play(clip.clone(), play));
            }
            if let Some((clip, _)) = self.world.crowd_bed.pick(&mut self.dice, &mut self.last[0]) {
                self.world_voices.push(m.play(clip, Play { gain: 0.5, looping: true, ..Play::default() }));
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
            if let Some((note, at)) = f.plane {
                s.engine.silence(m);
                s.plane.update(m, &self.plane, note, at);
                continue;
            }
            s.plane.silence(m);
            s.engine.update(h, FRAME, m, &s.bike);
            let near = Play { at: Some(h.at), near: self.rider.near, far: self.rider.far, ..Play::default() };
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
                        m.play(clip, near);
                    }
                }
                let bike = Play { near: s.bike.wreck_near, far: s.bike.wreck_far, ..near };
                if let Some((clip, _)) = s.bike.wreck.pick(&mut s.dice, &mut s.last[2]) {
                    m.play(clip, bike);
                }
                if let Some((clip, _)) = self.world.oh.pick(&mut self.dice, &mut self.last[1]) {
                    m.play(clip, Play { gain: 0.6, ..Play::default() });
                }
            }
            if h.grounded && !s.was_grounded && f.wreck.is_none() {
                if let Some((clip, gain)) = self.landing.pick(-s.last_vy) {
                    m.play(clip, Play { gain, ..near });
                }
                if f.landed_trick {
                    if let Some((clip, _)) = self.world.cheer.pick(&mut self.dice, &mut self.last[2]) {
                        m.play(clip, Play { gain: 0.6, ..Play::default() });
                    }
                }
            }
            s.was_grounded = h.grounded;
            s.last_vy = f.vy;
        }
    }

    /// The ride is over: stop every effect, keep the music.
    pub fn quiet(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.stop_all();
        }
        self.started = false;
        self.world_voices.clear();
        for s in &mut self.seats {
            s.engine = Engine::new(s.dice.below(u32::MAX) as u64);
            s.plane = PlaneNote::default();
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

    pub fn rider(&mut self, res: &str) -> RiderSounds {
        let text = self.text(res, "aud\\rider.aud");
        let lib = &mut *self.lib;
        RiderSounds::read(&text, &mut |n| lib.clip(n))
    }

    pub fn machines(&mut self) -> (Landing, LoopEngine) {
        let machine = self.text("machine.res", "aud\\machine.aud");
        let cars = self.text("machine.res", "aud\\carengine.aud");
        let lib = &mut *self.lib;
        let landing = Landing::read(&machine, &mut |n| lib.clip(n));
        (landing, LoopEngine::read(&cars, "biplane", &mut |n| lib.clip(n)))
    }

    /// `track` is the track's name (`free10`): its `.aud` is in `tracks\free10.res`, the race's events
    /// in `tracks\free10_2.res`.
    pub fn world(&mut self, track_res: &str, events_res: &str, track: &str) -> WorldSounds {
        let t = self.text(track_res, &format!("tracks\\{track}\\{track}.aud"));
        let e = self.text(events_res, "aud\\event.aud");
        let lib = &mut *self.lib;
        WorldSounds::read(&t, &e, &mut |n| lib.clip(n))
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
    pub fn start(mixer: Arc<Mutex<Mixer>>) -> Result<Output, String> {
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

/// A slider's step (`0..=10`) as a gain.
pub fn volume(step: u8) -> f32 {
    let v = step.min(10) as f32 / 10.0;
    v * v
}
