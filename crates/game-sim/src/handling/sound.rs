//! **SLUS-21104's wreck sounds** (F31b5u): what the rider's wreck cue ([`super::rider::wreck_cue`])
//! asks of its cue and its sounds, in Rust. Where the original keeps each: research/oracle.txt
//! `# F31b5u`.
//!
//! A cue holds the sounds it may play and a list of the ones playing (a list: count, array, step,
//! cap, max). A sound plays through the sound driver: a voice index off the driver's index table
//! ([`index_pick`]), a voice off its two banks of 24 ([`voice_pick`]), and two commands queued for
//! the audio side (the stream's header, then the start). A voice's handle is its generation, slot
//! and bank; it answers "busy" off the voice's words, which the audio side keeps up to date.
//!
//! Ported: the cue's busy and its "made playing" (with the list's push and grow), the sound's busy
//! and the voice's, and the sound's play on a stream of one voice by its index. The paths the wreck
//! cue never takes are handed out whole ([`Sounds::play_handed`]): a sample, a stream of a voice pair,
//! a stream by its name. So are the sound's stop, the cue's own pick, the heap's resize and the
//! driver's queue flush.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

pub use super::node::Ram;
use ps2_float::F;

/// A sound's busy level when its voice is in no state the driver knows (0) and when it plays (2).
pub const QUIET: u32 = 0;
pub const PLAYING: u32 = 2;

/// A cue's "stopped" word (`+0x30`): set, the cue answers busy whatever its sounds say.
pub const CUE_STOPPED: u32 = 0x30;
/// A cue's list of the sounds playing (`+0x38`).
pub const CUE_PLAYING: u32 = 0x38;

/// A list's words: count, array, step, cap, max.
pub const COUNT: u32 = 0;
pub const ARRAY: u32 = 8;
pub const STEP: u32 = 0xc;
pub const CAP: u32 = 0x10;
pub const MAX: u32 = 0x14;

/// A sound's words.
pub const LOOSE: u32 = 0x1c;
pub const HANDLE: u32 = 0x2c;
pub const OWNER: u32 = 0x30;
/// The levels a play reads without and with the "held" bit: volume, pan, pitch each.
pub const LEVELS: u32 = 0x34;
pub const HELD_LEVELS: u32 = 0x40;
pub const STARTED: u32 = 0x7c;
pub const BYTE: u32 = 0xa0;
pub const STREAM: u32 = 0xa4;
pub const PAIR: u32 = 0xa8;
pub const RESTART: u32 = 0xac;
pub const INDEX: u32 = 0xb4;
/// The owner's word a stream carries (`+0x2c`).
pub const OWNER_WORD: u32 = 0x2c;

/// A play's "how" bits: stop first if busy, and play at the held levels (else the plain ones,
/// copied over the held).
pub const HOW_RESTART: u32 = 1;
pub const HOW_HELD: u32 = 4;

/// The driver's words, from its base ([`Sounds::driver`]).
const VOICES: u32 = 0x400;
const STATUS: u32 = 0x340;
const INDEX_USED: u32 = 0x1408;
const INDEX_LEVEL: u32 = 0x1410;
const INDEX_ENTRY: u32 = 0x1414;
const GENERATION: u32 = 0x1418;
const FLAGS: u32 = 0x1478;
const ASKED: u32 = 0x147c;
const ASKED_TOO: u32 = 0x1480;
const VOICE_ZERO: u32 = 0x1484;
const VOICE_LEVEL: u32 = 0x1488;
const VOICE_WORD: u32 = 0x1840;
const PANNED: u32 = 0x1b44;
const QUEUED: u32 = 0x1b4c;
const QUEUE: u32 = 0xa5c8;
const STREAMS: u32 = 0x1_3050;
const TWO_BANKS: u32 = 0x1_35ac;

/// The queue's room, and one command's size.
const QUEUE_ROOM: u32 = 64;
const COMMAND: u32 = 0x114;
const HEADER: u32 = 7;
const START: u32 = 8;

/// The answers of a stream start that failed: no voice index, no voice, a volume or a pitch out of
/// range.
pub const NO_INDEX: i32 = -107;
pub const NO_VOICE: i32 = -100;
pub const BAD_VOLUME: i32 = -111;
pub const BAD_PITCH: i32 = -113;

/// What the sounds hand out.
pub trait Sounds: Ram {
    /// The sound driver's base (its global's word, `+0x100`).
    fn driver(&mut self) -> u32;
    /// A sound's busy level (its vt `+0x20`); the plain sound's is [`busy`].
    fn busy_of(&mut self, sound: u32) -> u32;
    /// A sound's stop (its vt `+0x28`), handed 0.
    fn stop(&mut self, sound: u32);
    /// A sound's play, the original's whole: for a sample, a pair and a stream by name.
    fn play_handed(&mut self, sound: u32, how: u32) -> u32;
    /// A cue's own word on the sound it makes playing (its vt `+0xa4`), when handed one.
    fn cue_chose(&mut self, cue: u32, sound: u32, word: u32);
    /// The heap's resize of a list's array.
    fn resize(&mut self, block: u32, bytes: u32) -> u32;
    /// The driver's queue sent to the audio side, before a command that would fill it.
    fn flush(&mut self, driver: u32);
    /// The word the original's stream start copies into a lone voice's start command from under
    /// its own frame, where nothing on this path wrote (the audio side reads it only for a pair).
    fn stack_word(&mut self) -> u32;
}

fn byte(m: &mut impl Ram, at: u32) -> u32 {
    (m.word(at & !3) >> (8 * (at & 3))) & 0xff
}

fn half(m: &mut impl Ram, at: u32) -> u32 {
    (m.word(at & !3) >> (8 * (at & 2))) & 0xffff
}

fn set_byte(m: &mut impl Ram, at: u32, b: u32) {
    let s = 8 * (at & 3);
    let w = m.word(at & !3);
    m.set(at & !3, w & !(0xff << s) | (b & 0xff) << s);
}

fn set_half(m: &mut impl Ram, at: u32, h: u32) {
    let s = 8 * (at & 2);
    let w = m.word(at & !3);
    m.set(at & !3, w & !(0xffff << s) | (h & 0xffff) << s);
}

fn f(m: &mut impl Ram, at: u32) -> F {
    F::from_bits(m.word(at))
}

/// **A cue's busy level**: 0 with nothing playing, 2 when stopped, else the highest of its playing
/// sounds' (signed), the walk ending at the first that plays.
pub fn cue_busy(m: &mut impl Sounds, cue: u32) -> u32 {
    let list = cue + CUE_PLAYING;
    if m.word(list + COUNT) == 0 {
        return QUIET;
    }
    if m.word(cue + CUE_STOPPED) != 0 {
        return PLAYING;
    }
    let mut best = 0i32;
    let mut i = 0i32;
    while i < m.word(list + COUNT) as i32 {
        let array = m.word(list + ARRAY);
        let sound = m.word(array.wrapping_add((i as u32) << 2));
        let v = m.busy_of(sound) as i32;
        if best < v {
            best = v;
        }
        if v == PLAYING as i32 {
            break;
        }
        i += 1;
    }
    best as u32
}

/// **A cue's sound made playing**: the cue's own word on it first if handed one, then its playing
/// list emptied (the array zeroed) and the sound pushed. Answers 1.
pub fn cue_playing(m: &mut impl Sounds, cue: u32, sound: u32, word: u32) -> u32 {
    if word != 0 {
        m.cue_chose(cue, sound, word);
    }
    let list = cue + CUE_PLAYING;
    let n = m.word(list + COUNT);
    if n != 0 {
        let array = m.word(list + ARRAY);
        for k in 0..(n << 2) / 4 {
            m.set(array.wrapping_add(4 * k), 0);
        }
        m.set(list + COUNT, 0);
    }
    push(m, list, sound);
    1
}

/// **A list's push**: grown by its step when full (and allowed); answers 0 if it could not grow.
pub fn push(m: &mut impl Sounds, list: u32, v: u32) -> u32 {
    let n = m.word(list + COUNT);
    let step = m.word(list + STEP);
    if (n as i32) >= m.word(list + CAP) as i32 && grow(m, list, n.wrapping_add(step)) == 0 {
        return 0;
    }
    let n = m.word(list + COUNT);
    let array = m.word(list + ARRAY);
    m.set(array.wrapping_add(n << 2), v);
    let n = m.word(list + COUNT);
    m.set(list + COUNT, n.wrapping_add(1));
    1
}

/// **A list grown to `to`**: only past its cap and up to its max; the array resized.
pub fn grow(m: &mut impl Sounds, list: u32, to: u32) -> u32 {
    if (m.word(list + CAP) as i32) >= to as i32 || (m.word(list + MAX) as i32) < to as i32 {
        return 0;
    }
    let array = m.word(list + ARRAY);
    let array = m.resize(array, to << 2);
    m.set(list + ARRAY, array);
    m.set(list + CAP, to);
    1
}

/// **The plain sound's busy level**: none without a voice, else its voice's ([`voice_busy`]).
pub fn busy(m: &mut impl Sounds, sound: u32) -> u32 {
    let h = m.word(sound + HANDLE);
    if h == u32::MAX {
        return QUIET;
    }
    let d = m.driver();
    voice_busy(m, d, h)
}

/// A voice's bank, slot and generation off its handle's high half.
fn voice_of(h: u32) -> (u32, u32, u32) {
    let v = (h as i32 >> 16) as u32;
    (v & 1, (v >> 1) & 31, (v >> 6) & 0xffff)
}

/// **A voice's busy level** by its handle (the high half): 0 if its status is clear or its
/// generation moved on; a streamed voice 2 on its word 1, 0 on 99, else 1; any other 3 on status 8,
/// else 2.
pub fn voice_busy(m: &mut impl Ram, d: u32, h: u32) -> u32 {
    if h == u32::MAX {
        return QUIET;
    }
    let (bank, slot, gn) = voice_of(h);
    let status = half(m, d + bank * 0x60 + slot * 4 + STATUS);
    if status == 0 || half(m, d + bank * 0x30 + slot * 2 + GENERATION) != gn {
        return QUIET;
    }
    if byte(m, d + bank * 0x1e0 + slot * 0x14 + FLAGS) & 1 != 0 {
        match m.word(d + bank * 0x180 + slot * 0x10 + VOICE_WORD) {
            1 => PLAYING,
            99 => QUIET,
            _ => 1,
        }
    } else if status == 8 {
        3
    } else {
        PLAYING
    }
}

/// **The plain sound's play**, `how` its bits ([`HOW_RESTART`], [`HOW_HELD`]): stopped first if it
/// restarts or (asked) is busy; a stream of one voice by its index started ([`stream`]) at the held
/// levels or at the plain ones (copied over the held); the answer kept as its handle. Answers 1 if
/// it started (the sound then not loose and started), else 0.
pub fn play(m: &mut impl Sounds, sound: u32, how: u32) -> u32 {
    if m.word(sound + STREAM) == 0 || m.word(sound + PAIR) != 0 || (m.word(sound + INDEX) as i32) < 0 {
        return m.play_handed(sound, how);
    }
    if m.word(sound + RESTART) != 0 || (how & HOW_RESTART != 0 && m.busy_of(sound) != 0) {
        m.stop(sound);
    }
    let held = how & HOW_HELD != 0;
    let at = if held { HELD_LEVELS } else { LEVELS };
    let d = m.driver();
    let owner = m.word(sound + OWNER);
    let owner = m.word(owner + OWNER_WORD);
    let s = Stream {
        owner,
        restart: m.word(sound + RESTART),
        volume: m.word(sound + at) as i32,
        pan: m.word(sound + at + 4) as i32,
        pitch: m.word(sound + at + 8) as i32,
        index: m.word(sound + INDEX),
        byte: m.word(sound + BYTE) & 0xff,
    };
    let h = stream(m, d, &s);
    m.set(sound + HANDLE, h as u32);
    if !held {
        for k in [8, 0, 4] {
            let w = m.word(sound + LEVELS + k);
            m.set(sound + HELD_LEVELS + k, w);
        }
    }
    if h < 0 {
        return 0;
    }
    m.set(sound + LOOSE, 0);
    m.set(sound + STARTED, 1);
    1
}

/// A stream start of one voice: the owner's word, the restart word (its low bit kept on the
/// voice), the levels, the stream's index and the start's byte.
pub struct Stream {
    pub owner: u32,
    pub restart: u32,
    pub volume: i32,
    pub pan: i32,
    pub pitch: i32,
    pub index: u32,
    pub byte: u32,
}

/// **A stream started on one voice**: a voice index off the index table, the stream's header
/// written to its entry, a voice picked from the top, the pan split into two levels by 180 (off
/// when the driver does not pan), and two commands queued: the header, then the start. Answers the
/// voice's handle in the high half, or [`NO_INDEX`], [`NO_VOICE`], [`BAD_VOLUME`], [`BAD_PITCH`]
/// (the voice and index let go).
pub fn stream(m: &mut impl Sounds, d: u32, s: &Stream) -> i32 {
    let v = index_pick(m, d, 0) as i16 as i32;
    if v == -1 {
        return NO_INDEX;
    }
    let streams = m.word(d + STREAMS);
    let list = m.word(streams + 0x34);
    let header = m.word(list.wrapping_add(s.index.wrapping_mul(12)) + 4);
    let header = m.word(header);
    let head = [
        (v as u32) << 16,
        1 | (s.owner & 0xff) << 8 | (s.restart & 0xff) << 24,
        0,
        m.word(header + 8),
        0,
        m.word(header + 12),
        0,
        m.word(header + 20),
        0,
        0,
    ];
    let entry = m.word(d + INDEX_ENTRY).wrapping_add((v as u32) << 3);
    let w = m.word(header + 4);
    m.set(entry + 4, w);
    let top = half(m, header);
    let w = m.word(entry) & 0xffff | top << 16;
    m.set(entry, w & 0xffff_0000 | s.owner & 0xffff);
    let asked = half(m, entry);
    let h = voice_pick(m, d, v as u32, asked, true, s.restart);
    if h == -1 {
        free_index(m, d, v);
        return NO_VOICE;
    }
    let (bank, slot) = (h as u32 & 1, (h as u32 >> 1) & 31);
    let voice_word = d + bank * 0x180 + slot * 0x10 + VOICE_WORD;
    let mut pan = s.pan;
    if m.word(d + PANNED) == 0 || matches!(pan, 0 | 360 | 180) {
        pan = -1;
    }
    let volume = s.volume;
    if !(-1..=100).contains(&volume) {
        stop_voice(m, d, h as u32);
        free_index(m, d, v);
        return BAD_VOLUME;
    }
    let (left, right) = if pan == -1 {
        (volume, volume)
    } else if pan < 180 {
        (volume.wrapping_mul(pan) / 180, volume)
    } else {
        (volume, volume.wrapping_mul(pan - 180) / 180)
    };
    if !(-1..0x4000).contains(&s.pitch) {
        stop_voice(m, d, h as u32);
        free_index(m, d, v);
        return BAD_PITCH;
    }
    let at = queue(m, d);
    m.set(at + 8, HEADER);
    for (k, w) in head.into_iter().enumerate() {
        m.set(at + 0xc + 4 * k as u32, w);
    }
    let at = queue(m, d);
    let under = m.stack_word();
    let start = [START, (v as u32) << 16, 1, h as u32, 0, left as u32, right as u32, 0, s.pitch as u32, voice_word, under];
    for (k, w) in start.into_iter().enumerate() {
        m.set(at + 8 + 4 * k as u32, w);
    }
    set_byte(m, at + 0x34, s.byte);
    (h << 16) as i32
}

/// The next command's room in the driver's queue (the queue sent first if this one would fill it),
/// counted on the driver and in the queue's own count.
fn queue(m: &mut impl Sounds, d: u32) -> u32 {
    if m.word(d + QUEUED).wrapping_add(1) == QUEUE_ROOM {
        m.flush(d);
    }
    let n = m.word(d + QUEUED).wrapping_add(1);
    m.set(d + QUEUED, n);
    let q = m.word(d + QUEUE);
    let c = m.word(q).wrapping_add(1);
    m.set(q, c);
    m.word(d + QUEUE).wrapping_add(n.wrapping_mul(COMMAND))
}

/// A voice index let go, if it is one.
fn free_index(m: &mut impl Ram, d: u32, v: i32) {
    if v >= 0 && (v as u32) < m.word(d + VOICES) {
        let at = m.word(d + INDEX_USED).wrapping_add((v as u32) << 2);
        m.set(at, 0);
    }
}

/// A voice stopped by its handle, if its generation is still the handle's: its status 1, and its
/// level 3 (a streamed voice's level and word 0).
fn stop_voice(m: &mut impl Ram, d: u32, h: u32) {
    let (bank, slot, gn) = ((h & 1), (h >> 1) & 31, (h as i32 >> 6) as u32 & 0xffff);
    if half(m, d + bank * 0x30 + slot * 2 + GENERATION) != gn {
        return;
    }
    let at = d + bank * 0x60 + slot * 4 + STATUS;
    let w = m.word(at);
    m.set(at, w & 0xffff_0000 | 1);
    let voice = d + bank * 0x1e0 + slot * 0x14;
    if byte(m, voice + FLAGS) & 1 != 0 {
        m.set(voice + VOICE_LEVEL, 0);
        m.set(d + bank * 0x180 + slot * 0x10 + VOICE_WORD, 0);
    } else {
        m.set(voice + VOICE_LEVEL, 0x4040_0000); // 3.0
    }
}

/// **A voice index picked**: of the free ones (all, past the first two, or the first two with the
/// driver's two banks on and `two` set or not), the first while the best level is zero, then any of
/// a higher level. Marked used, its level zeroed; -1 if none.
pub fn index_pick(m: &mut impl Ram, d: u32, two: u32) -> i32 {
    let (from, to) = if m.word(d + TWO_BANKS) == 0 {
        (0, m.word(d + VOICES) as i32)
    } else if two == 0 {
        (2, m.word(d + VOICES) as i32)
    } else {
        (0, 2)
    };
    let mut best = F::ZERO;
    let mut picked = -1i32;
    let mut i = from;
    while i < to {
        let k = (i as u32) << 2;
        let used = m.word(d + INDEX_USED);
        if m.word(used.wrapping_add(k)) == 0 {
            let level = m.word(d + INDEX_LEVEL);
            let level = f(m, level.wrapping_add(k));
            if F::ZERO == best || !(level <= best) {
                best = level;
                picked = i;
            }
        }
        i += 1;
    }
    if picked != -1 {
        let k = (picked as u32) << 2;
        let used = m.word(d + INDEX_USED).wrapping_add(k);
        m.set(used, 1);
        let level = m.word(d + INDEX_LEVEL).wrapping_add(k);
        m.set(level, 0);
    }
    picked
}

/// **A voice picked** for voice index `asked` (and `asked_too`): over both banks' 24 slots, from the
/// first or (`top`) from the last, a slot with a clear status and a level past 2.5, the first such
/// while the best is zero, then any of a higher level, until the best reaches 180. Its status takes
/// the index, its generation steps (0 past 500), its flags the top and `restart` bits; answers its
/// handle (generation, slot, bank), or -1 if none.
pub fn voice_pick(m: &mut impl Ram, d: u32, asked: u32, asked_too: u32, top: bool, restart: u32) -> i32 {
    let (least, enough) = (F::from_bits(0x4020_0000), F::from_bits(0x4334_0000));
    let mut best = F::ZERO;
    let mut picked = None;
    let order = |k: u32, n: u32| if top { n - 1 - k } else { k };
    'banks: for b in 0..2 {
        let bank = order(b, 2);
        for s in 0..24 {
            let slot = order(s, 24);
            if half(m, d + bank * 0x60 + slot * 4 + STATUS) == 0 {
                let level = f(m, d + bank * 0x1e0 + slot * 0x14 + VOICE_LEVEL);
                if !(level <= least) && (F::ZERO == best || !(level <= best)) {
                    best = level;
                    picked = Some((bank, slot));
                }
            }
            if picked.is_some() && !(best < enough) {
                break 'banks;
            }
        }
    }
    let Some((bank, slot)) = picked else { return -1 };
    m.set(d + bank * 0x60 + slot * 4 + STATUS, asked << 16 | 5);
    let at = d + bank * 0x30 + slot * 2 + GENERATION;
    let gn = half(m, at).wrapping_add(1) & 0xffff;
    set_half(m, at, if gn < 501 { gn } else { 0 });
    let gn = half(m, at) as u16 as i16 as i32;
    let voice = d + bank * 0x1e0 + slot * 0x14;
    let flags = byte(m, voice + FLAGS) & !0x1f | u32::from(top) | (restart & 1) << 1;
    set_byte(m, voice + FLAGS, flags);
    m.set(voice + ASKED, asked);
    m.set(voice + ASKED_TOO, asked_too);
    m.set(voice + VOICE_ZERO, 0);
    m.set(voice + VOICE_LEVEL, 0);
    gn << 6 | (slot << 1 | bank) as i32
}
