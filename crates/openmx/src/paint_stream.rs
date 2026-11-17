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
    /// Shown, and read again to be written over where it is (MD97a: the iPhone app was away, and
    /// what went up as it went may not have): what is drawn stays till its layers come.
    Again,
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
            if gaps[i] > self.reach.drop.0 && matches!(self.held[i], Held::Shown | Held::Writing | Held::Again) {
                r.world_paint_let_go(i);
                self.writing.retain(|w| w.0 != i);
                // One read again is still coming, and is let go as it comes.
                self.held[i] = if self.held[i] == Held::Again { Held::Asked } else { Held::Far };
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
                // Read once already, and drawn as it was.
                Err(e) if self.held[i] == Held::Again => {
                    self.log.push(format!("world paint: {} not written again: {e}", self.names[i]));
                    self.held[i] = Held::Shown;
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

    /// Every map shown or being written read again and written over its own pages from the next
    /// updates (MD97a: after the iPhone app was away, when the pages written as it went may have been
    /// refused, a dirt-coloured map left drawn for good). A map shown stays drawn till its layers come;
    /// one being written starts again. A map being read is written when it comes, as it would be. The
    /// number of maps asked.
    pub fn write_again(&mut self, r: &Renderer) -> usize {
        self.writing.clear();
        let mut n = 0;
        for i in 0..self.held.len() {
            if matches!(self.held[i], Held::Shown | Held::Writing)
                && let Some(size) = r.world_paint_layer_size(i)
                && self.ask.send((i, size)).is_ok()
            {
                self.held[i] = Held::Again;
                n += 1;
            }
        }
        n
    }

    /// Every map back on its far pages, as [`Renderer::give_back_ground`] left them (P307: the iPhone
    /// app in the background): those near the riders are read and written again from the next
    /// update, once [`Renderer::take_back_ground_paint`] has made the arrays again. A map being read
    /// stays asked, and is written when it comes. How many maps were let go.
    pub fn given_back(&mut self) -> usize {
        self.writing.clear();
        let mut n = 0;
        for (held, name) in self.held.iter_mut().zip(&self.names) {
            if matches!(held, Held::Shown | Held::Writing | Held::Again) {
                // A map read again still has its read coming.
                *held = if *held == Held::Again { Held::Asked } else { Held::Far };
                n += 1;
                self.log.push(format!("world paint: {name} let go in the background"));
            }
        }
        self.let_go += n;
        n
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_render::gpu::{GroundChunk, GroundPaint, GroundPiece, GroundVertex, Scene3, Target, View};
    use game_render::mat::v3;
    use game_render::pst::Texture;
    use game_render::terrain::{PaintTile, Pages, Regions, WorldPaint};
    use game_render::world::{Assets, Camera, Lighting};
    use ps2_float::F;

    const SPACING: f32 = 9.0;
    /// A map's side: four leaves of sixteen samples, one page.
    const SIDE: f32 = 64.0 * SPACING;
    /// Where the second map sits, past the first.
    const AT: [f32; 2] = [2048.0, 0.0];

    /// A map of one 128-pixel page of `rgb`.
    fn pages(rgb: [u8; 3]) -> Pages {
        let page = Texture { width: 128, height: 128, rgba: [rgb[0], rgb[1], rgb[2], 255].repeat(128 * 128) };
        Pages { pages: vec![page], across: 4, along: 4, page_of: vec![Some(0); 16], spacing: SPACING, detail: None, detail_repeat: SPACING * 8.0 }
    }

    fn world() -> WorldPaint {
        WorldPaint {
            regions: Regions { owned: vec![[0.0, 0.0, SIDE, SIDE], [AT[0], AT[1], AT[0] + SIDE, AT[1] + SIDE]], period: [0.0; 2] },
            tiles: vec![PaintTile { at: [0.0, 0.0], pages: Some(pages([90, 140, 70])), sharp: None }, PaintTile { at: AT, pages: Some(pages([200, 60, 40])), sharp: None }],
            dirt: [150.0, 118.0, 86.0],
            far: Default::default(),
        }
    }

    /// One flat chunk over the second map.
    fn chunk() -> GroundChunk {
        let cells = 64;
        let vertices = (0..=cells)
            .flat_map(|j| (0..=cells).map(move |i| (i, j)))
            .map(|(i, j)| GroundVertex { pos: [AT[0] + i as f32 * SPACING, 40.0, AT[1] + j as f32 * SPACING], normal: [0.0, 1.0, 0.0] })
            .collect();
        GroundChunk { vertices, cells, at: (0, 0), missing: Vec::new(), centre: v3(AT[0] + SIDE / 2.0, 40.0, AT[1] + SIDE / 2.0), radius: SIDE }
    }

    /// The second map from a rider's height.
    fn frame(gpu: &mut Renderer) -> Vec<u8> {
        let scene = game_render::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
        let light = Lighting::of(&scene);
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        let camera = Camera { eye: v3(AT[0] + 288.0, 46.0, AT[1] + 120.0), at: v3(AT[0] + 300.0, 40.0, AT[1] + 160.0), fov: 1.0, near: 0.5, far: 2000.0 };
        let (w, h) = (160u32, 90u32);
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
        gpu.render(
            &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
            &Assets::default(),
            &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
        );
        gpu.read(&texture)
    }

    /// A stream over `w`'s maps as `r` holds them, its reads answered by the test: the asks it
    /// makes, and where the test sends what was read.
    fn stream(w: &WorldPaint, r: &Renderer) -> (PaintStream, Receiver<(usize, (u32, u32))>, Sender<(usize, Result<Layers, String>)>) {
        let (ask, asked) = channel();
        let (send, read) = channel();
        let held = (0..w.tiles.len()).map(|i| if r.world_paint_shown(i) { Held::Shown } else { Held::Far }).collect();
        let s = PaintStream {
            owned: w.regions.owned.clone(),
            names: (0..w.tiles.len()).map(|i| format!("map{i}")).collect(),
            reach: Reach { load: F(5000.0), drop: F(6000.0) },
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
        };
        (s, asked, send)
    }

    fn layers(w: &WorldPaint, i: usize, size: (u32, u32)) -> Layers {
        w.tiles[i].pages.as_ref().unwrap().pages.iter().map(|t| game_render::gpu::page_layer_mips(t, size)).collect()
    }

    /// MD97a: after the iPhone app was away, every map shown is read again and written over its own
    /// pages — one whose pages a refused write left wrong draws as it did before — and is drawn all
    /// the while; then the stream rests. Skips without an adapter.
    #[test]
    fn the_maps_shown_are_written_again_after_the_app_was_away() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        let w = world();
        gpu.set_ground_streamed(64, GroundPaint::World(&w), w.dirt, 40000.0).expect("the world");
        gpu.add_ground_piece_of(1, GroundPiece::of(vec![chunk()])).expect("the piece");
        gpu.show_ground_piece(1, 0.0);
        assert!(gpu.world_paint_shown(0) && gpu.world_paint_shown(1), "both maps on their own pages from the load");
        let before = frame(&mut gpu);
        // What a write refused in the background leaves: not the map's pages.
        let size = gpu.world_paint_layer_size(1).expect("a paged map");
        gpu.world_paint_write(1, 0, &layers(&WorldPaint { tiles: vec![PaintTile { at: AT, pages: Some(pages([0, 0, 0])), sharp: None }], ..world() }, 0, size)[0]);
        let spoilt = frame(&mut gpu);
        let off = before.chunks_exact(4).zip(spoilt.chunks_exact(4)).filter(|(a, b)| a != b).count();
        assert!(off > 1000, "only {off} pixels changed: the spoilt page is not drawn");

        let (mut s, asked, send) = stream(&w, &gpu);
        let near = [[AT[0] + 300.0, AT[1] + 160.0]];
        s.update(&near, &mut gpu);
        assert!(asked.try_recv().is_err(), "nothing asked before the app was away");
        assert_eq!(s.write_again(&gpu), 2);
        let asks: Vec<_> = asked.try_iter().collect();
        assert_eq!(asks.iter().map(|a| a.0).collect::<Vec<_>>(), [0, 1]);
        assert!(gpu.world_paint_shown(0) && gpu.world_paint_shown(1), "drawn on their pages till they come");
        s.update(&near, &mut gpu);
        assert!(asked.try_recv().is_err(), "asked once");
        for (i, size) in asks {
            send.send((i, Ok(layers(&w, i, size)))).unwrap();
        }
        s.update(&near, &mut gpu);
        assert_eq!(s.shown, 2, "both written again in a frame: {:?}", s.log);
        assert!(s.held.iter().all(|h| *h == Held::Shown));
        let back = frame(&mut gpu);
        let off = before.chunks_exact(4).zip(back.chunks_exact(4)).filter(|(a, b)| a != b).count();
        assert_eq!(off, 0, "{off} pixels differ from before the page was spoilt");
        for _ in 0..3 {
            s.update(&near, &mut gpu);
        }
        assert!(asked.try_recv().is_err() && s.shown == 2, "written again once, not every frame");

        // Gone out of reach while read again: let go at once, and the read let go as it comes, not
        // asked twice.
        assert_eq!(s.write_again(&gpu), 2);
        let asks: Vec<_> = asked.try_iter().collect();
        let far = [[AT[0] + 20000.0, AT[1] + 20000.0]];
        s.update(&far, &mut gpu);
        assert!(!gpu.world_paint_shown(0) && !gpu.world_paint_shown(1), "let go out of reach");
        assert!(s.held.iter().all(|h| *h == Held::Asked));
        for (i, size) in asks {
            send.send((i, Ok(layers(&w, i, size)))).unwrap();
        }
        s.update(&far, &mut gpu);
        assert!(s.held.iter().all(|h| *h == Held::Far));
        assert_eq!(gpu.world_paint_pool().expect("a world").0.iter().map(|u| u.0).sum::<u32>(), 0, "no layers left taken");
        s.update(&near, &mut gpu);
        assert_eq!(asked.try_iter().count(), 2, "asked once each coming back");
    }
}
