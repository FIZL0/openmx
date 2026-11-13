//! A world's ground paint held near the riders (P283): every map's far pages are on the GPU from the
//! load, and a map's own pages are read again from its folder on a thread of their own as a rider
//! comes within [`Reach::load`] of it, written into the world's page arrays a few layers a frame, and
//! let go past [`Reach::drop`] — so world_ultra's 77 maps hold a few maps' pages, not every one's.

use crate::clock::Instant;
use crate::worlds::{self, WorldMod, WorldPiece};
use game_render::gpu::Renderer;
use game_sim::tiles::Reach;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};

/// One map's layers as the GPU takes them ([`game_render::gpu::page_layer_mips`]): its pages, then
/// its sharp splat's modulate pages.
type Layers = Vec<Vec<(u32, u32, Vec<u8>)>>;

/// The most bytes of pages a frame writes, past the first layer (P283: a map of world_ultra's is
/// 4-21 MB of layers with their levels).
pub const PAINT_BYTES_A_FRAME: usize = 4 << 20;

#[derive(Clone, Copy, PartialEq)]
enum Held {
    /// On its far pages.
    Far,
    Asked,
    /// Its layers being written.
    Writing,
    Shown,
    /// Not painted on the GPU, or its pages did not read: left as it is.
    Never,
}

pub struct PaintStream {
    owned: Vec<[f32; 4]>,
    names: Vec<String>,
    reach: Reach,
    held: Vec<Held>,
    ask: Sender<(usize, (u32, u32))>,
    read: Receiver<(usize, Result<Layers, String>)>,
    /// Read maps not all written yet: the map, its layers and the next to write.
    writing: VecDeque<(usize, Layers, u32)>,
    log: Vec<String>,
    /// The most bytes a frame wrote, the bytes in all, and the maps shown and let go in the ride.
    pub most_bytes: usize,
    pub bytes: usize,
    pub shown: usize,
    pub let_go: usize,
    /// The longest a frame's update took, in seconds.
    pub slowest: f32,
    /// The most CPU time the shell's thread spent in a frame's update, in seconds (K23): what the
    /// gate's budget reads, the wall clock above taking in the other processes' load. The wall
    /// clock's where the thread's is not known.
    pub slowest_cpu: f32,
    /// The CPU time of each update that wrote pages, in seconds (K53): the gate judges their median,
    /// which a contended machine's odd slow update (a page fault, a cache another process emptied)
    /// does not move the way it moves the slowest.
    pub writing_cpu: Vec<f32>,
}

impl PaintStream {
    /// The stream over `w`'s maps as the renderer holds them now: a map shown on its own pages at the
    /// load is held, the rest are on their far pages.
    pub fn start(w: &WorldMod, reach: Reach, r: &Renderer) -> PaintStream {
        let regions = w.regions();
        let maps: Vec<WorldPiece> = w.pieces[..w.tile_count].to_vec();
        let names = maps.iter().map(|p| p.tile.name.clone()).collect();
        let held = (0..maps.len())
            .map(|i| match (r.world_paint_layer_size(i), r.world_paint_shown(i)) {
                (None, _) => Held::Never,
                (Some(_), true) => Held::Shown,
                (Some(_), false) => Held::Far,
            })
            .collect();
        let (ask, asked) = channel::<(usize, (u32, u32))>();
        let (send, read) = channel();
        std::thread::spawn(move || {
            while let Ok((i, size)) = asked.recv() {
                let layers = worlds::map_paint(&maps[i]).and_then(|(pages, sharp, ..)| {
                    let pages = pages.ok_or("no pages")?;
                    let modulate = sharp.iter().flat_map(|s| s.modulate.iter());
                    Ok(pages.pages.iter().chain(modulate).map(|t| game_render::gpu::page_layer_mips(t, size)).collect())
                });
                if send.send((i, layers)).is_err() {
                    return;
                }
            }
        });
        PaintStream {
            owned: regions.owned,
            names,
            reach,
            held,
            ask,
            read,
            writing: VecDeque::new(),
            log: Vec::new(),
            most_bytes: 0,
            bytes: 0,
            shown: 0,
            let_go: 0,
            slowest: 0.0,
            slowest_cpu: 0.0,
            writing_cpu: Vec::new(),
        }
    }

    /// Let go of the maps past reach of `near` (the riders and their copies across a wrap's seams),
    /// ask for the maps within reach nearest first, and write a frame's worth of what was read.
    pub fn update(&mut self, near: &[[f32; 2]], r: &mut Renderer) {
        let at = Instant::now();
        let cpu_at = cpu_time(true);
        let gaps: Vec<f32> = self.owned.iter().map(|o| near.iter().map(|&p| gap(o, p)).fold(f32::INFINITY, f32::min)).collect();
        for i in 0..self.held.len() {
            if gaps[i] > self.reach.drop.0 && matches!(self.held[i], Held::Shown | Held::Writing) {
                r.world_paint_let_go(i);
                self.writing.retain(|w| w.0 != i);
                self.held[i] = Held::Far;
                self.let_go += 1;
                self.log.push(format!("world paint: {} let go", self.names[i]));
            }
        }
        let mut want: Vec<usize> = (0..self.held.len()).filter(|&i| self.held[i] == Held::Far && gaps[i] <= self.reach.load.0).collect();
        want.sort_by(|a, b| gaps[*a].total_cmp(&gaps[*b]));
        for i in want {
            if let Some(size) = r.world_paint_layer_size(i)
                && self.ask.send((i, size)).is_ok()
            {
                self.held[i] = Held::Asked;
            }
        }
        while let Ok((i, layers)) = self.read.try_recv() {
            match layers {
                // Gone out of reach while it was read.
                Ok(_) if gaps[i] > self.reach.drop.0 => self.held[i] = Held::Far,
                Ok(layers) => {
                    self.held[i] = Held::Writing;
                    self.writing.push_back((i, layers, 0));
                }
                Err(e) => {
                    self.log.push(format!("world paint: {} stays far: {e}", self.names[i]));
                    self.held[i] = Held::Never;
                }
            }
        }
        // Nearest first, and a layer at least.
        self.writing.make_contiguous().sort_by(|a, b| gaps[a.0].total_cmp(&gaps[b.0]));
        let mut wrote = 0usize;
        let mut k = 0;
        while k < self.writing.len() && (wrote == 0 || wrote < PAINT_BYTES_A_FRAME) {
            let i = self.writing[k].0;
            if self.writing[k].2 == 0 && !r.world_paint_take(i) {
                // No room in its array till a map is let go.
                k += 1;
                continue;
            }
            let (_, layers, next) = &mut self.writing[k];
            while (*next as usize) < layers.len() && (wrote == 0 || wrote < PAINT_BYTES_A_FRAME) {
                let levels = std::mem::take(&mut layers[*next as usize]);
                wrote += levels.iter().map(|l| l.2.len()).sum::<usize>();
                r.world_paint_write(i, *next, &levels);
                *next += 1;
            }
            if *next as usize == layers.len() {
                r.world_paint_show(i);
                self.held[i] = Held::Shown;
                self.shown += 1;
                self.writing.remove(k);
                self.log.push(format!("world paint: {} shown", self.names[i]));
            } else {
                k += 1;
            }
        }
        self.most_bytes = self.most_bytes.max(wrote);
        self.bytes += wrote;
        let took = at.elapsed();
        self.slowest = self.slowest.max(took.as_secs_f32());
        let cpu = cpu_at.zip(cpu_time(true)).map_or(took, |(a, b)| b.saturating_sub(a));
        self.slowest_cpu = self.slowest_cpu.max(cpu.as_secs_f32());
        if wrote > 0 {
            self.writing_cpu.push(cpu.as_secs_f32());
        }
    }

    /// The median and the 90th percentile of the writing updates' CPU time, in seconds, and how many
    /// there were (K53).
    pub fn writing_cpu_spread(&self) -> (f32, f32, usize) {
        let mut cpu = self.writing_cpu.clone();
        if cpu.is_empty() {
            return (0.0, 0.0, 0);
        }
        cpu.sort_by(f32::total_cmp);
        (cpu[cpu.len() / 2], cpu[cpu.len() * 9 / 10], cpu.len())
    }

    /// What the stream did since the shell last took the lines.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }
}

/// The CPU time this process (`thread: false`, every thread summed) or the calling thread has run so
/// far (K23), as Linux counts it: unlike the wall clock it hardly moves with the other processes'
/// load, so a budget the gate holds a load or a frame's update to reads it. The process's comes from
/// `/proc/self/stat`'s user and system ticks (Linux's are 100 a second), the thread's in nanoseconds
/// from `/proc/thread-self/schedstat`. None where there is no /proc.
pub fn cpu_time(thread: bool) -> Option<std::time::Duration> {
    if thread {
        let stat = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
        return Some(std::time::Duration::from_nanos(stat.split_whitespace().next()?.parse().ok()?));
    }
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The fields after the command's name, which is in brackets and may hold spaces: utime and stime
    // are the 14th and 15th of the line, the 12th and 13th after the name.
    let mut after = stat.get(stat.rfind(')')? + 1..)?.split_whitespace().skip(11);
    let ticks: u64 = after.next()?.parse::<u64>().ok()? + after.next()?.parse::<u64>().ok()?;
    Some(std::time::Duration::from_millis(ticks * 10))
}

/// How far `p` is from the rectangle `[x0, z0, x1, z1]`, 0 inside it.
fn gap(r: &[f32; 4], p: [f32; 2]) -> f32 {
    let dx = (r[0] - p[0]).max(p[0] - r[2]).max(0.0);
    let dz = (r[1] - p[1]).max(p[1] - r[3]).max(0.0);
    dx.hypot(dz)
}
