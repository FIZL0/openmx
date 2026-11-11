//! F46b's gather in RAM (out of `mxvatv_bike_ride.rs`, shared since S289 with
//! `mxvatv_rider_modes.rs`, whose rider wreck arm gathers its own nearby list): state 4's quadtree,
//! moving list and collidables read out of RAM, [`atv_world::gather`] run, and everything it changed
//! written back.
#![allow(dead_code)]

use game_sim::atv_world;
use game_sim::broad_phase as bp;
use game_sim::world_search as ws;
use ps2_float::F;
use ps2_oracle::Ee;

/// The quadtree's and the list sub-database's vtables (`mxvatv_bike_ride.rs`'s F43b and MD75c1 ones).
const TREE_VT: u32 = 0x003b_6700;
const DB_LIST_VT: u32 = 0x003b_5510;

/// F46b: the gather's globals — the stamp queries hand entries out under (a halfword), the
/// quadtree last queried, world units per tree cell, the table every global query ends with, the
/// ecosystem (placed groups' cell and models). The quadtree's vtable is [`TREE_VT`].
pub const STAMP_AT: u32 = 0x003d_3b88;
pub const CURRENT_AT: u32 = 0x0040_c198;
pub const CELL_SIZE_AT: u32 = 0x0040_c1a0;
pub const GLOBAL_AT: u32 = 0x0040_c440;
pub const ECO_AT: u32 = 0x0052_cd88;

/// A float's word as it is, bits and all (the gather copies the centre by integer loads).
pub fn word_f(ee: &Ee, a: u32) -> F {
    F(f32::from_bits(ee.u32_at(a)))
}

pub fn set_u8(ee: &mut Ee, a: u32, v: u8) {
    ee.write(a, 1, u128::from(v));
}

pub fn set_u16(ee: &mut Ee, a: u32, v: u16) {
    ee.write(a, 2, u128::from(v));
}

/// The quadtree query's node pool (tree `+0x94`), as its take and its reset leave it in RAM: chunks
/// of `+8` nodes of `+0x1c` bytes after a `+0x38`-byte header, the chunks in use from `+0xc` to
/// `+0x10`, the spare ones from `+0x14`, the next node of the last chunk at `+0x28` with `+0x18` of
/// it taken, the free chain at `+0x24`.
pub mod query_pool {
    use ps2_oracle::Ee;

    /// A node: the next of the last chunk, else off the free chain, else the first spare chunk (a
    /// new chunk off the heap the gather never needs here: it panics).
    pub fn take(ee: &mut Ee, p: u32) -> u32 {
        let (cap, used, stride) = (ee.i32_at(p + 8), ee.i32_at(p + 0x18), ee.u32_at(p + 0x1c));
        if used < cap {
            let n = ee.u32_at(p + 0x28);
            ee.set_u32(p + 0x28, n + stride);
            ee.set_u32(p + 0x18, (used + 1) as u32);
            return n;
        }
        let free = ee.u32_at(p + 0x24);
        if free != 0 {
            ee.set_u32(p + 0x24, ee.u32_at(free));
            return free;
        }
        let spare = ee.u32_at(p + 0x14);
        assert_ne!(spare, 0, "the query's pool {p:#x} wants a chunk off the heap: not modelled");
        if ee.u32_at(p + 0xc) == 0 {
            ee.set_u32(p + 0x10, spare);
            ee.set_u32(p + 0xc, spare);
        } else {
            let last = ee.u32_at(p + 0x10);
            ee.set_u32(last, spare);
            ee.set_u32(p + 0x10, spare);
        }
        ee.set_u32(p + 0x14, ee.u32_at(spare));
        ee.set_u32(spare, 0);
        let n = spare + ee.u32_at(p + 0x38);
        ee.set_u32(p + 0x28, n + stride);
        ee.set_u32(p + 0x18, 1);
        n
    }

    /// Every node given back: the chunks in use put before the spare ones.
    pub fn reset(ee: &mut Ee, p: u32) {
        let first = ee.u32_at(p + 0xc);
        if first != 0 {
            let (spare, last) = (ee.u32_at(p + 0x14), ee.u32_at(p + 0x10));
            if spare != 0 && last != 0 {
                ee.set_u32(last, spare);
            }
            ee.set_u32(p + 0x14, first);
        }
        ee.set_u32(p + 0xc, 0);
        ee.set_u32(p + 0x10, 0);
        ee.set_u32(p + 0x18, ee.u32_at(p + 8));
        ee.set_u32(p + 0x24, 0);
    }
}

/// Where the world [`GatherInRam::read`] read is in RAM: the entries by item (a collidable's is its
/// `+0x18`), the space links, tables and structures in the model's order, and per structure its
/// tree nodes, their chunks and the open query's list nodes.
#[derive(Default)]
pub struct GatherInRam {
    pub items: Vec<u32>,
    pub links: Vec<u32>,
    pub tables: Vec<u32>,
    pub structures: Vec<u32>,
    pub nodes: Vec<Vec<u32>>,
    pub chunks: Vec<Vec<Vec<u32>>>,
    pub listed: Vec<Vec<u32>>,
    pub current: u32,
    /// Where each entry is in `items` (S289: state 4's 15,000 entries made a search quadratic),
    /// built again when it no longer matches `items`, which a caller may have replaced.
    pub index: std::collections::HashMap<u32, usize>,
}

impl GatherInRam {
    pub fn item(&mut self, e: u32) -> usize {
        assert_ne!(e, 0, "an entry at 0");
        if self.index.len() != self.items.len() || self.index.get(&e).is_some_and(|&i| self.items.get(i) != Some(&e)) {
            self.index = self.items.iter().enumerate().map(|(i, &x)| (x, i)).collect();
        }
        *self.index.entry(e).or_insert_with(|| {
            self.items.push(e);
            self.items.len() - 1
        })
    }

    pub fn table(&mut self, t: u32) -> usize {
        self.tables.iter().position(|&x| x == t).unwrap_or_else(|| {
            self.tables.push(t);
            self.tables.len() - 1
        })
    }

    pub fn structure(&mut self, s: u32) -> Option<usize> {
        (s != 0).then(|| {
            self.structures.iter().position(|&x| x == s).unwrap_or_else(|| {
                self.structures.push(s);
                self.structures.len() - 1
            })
        })
    }

    pub fn pair(&mut self, ee: &Ee, t: u32) -> bp::Pair {
        bp::Pair { fixed: self.structure(ee.u32_at(t + 0x28)), moving: self.structure(ee.u32_at(t + 0x2c)) }
    }

    pub fn objects(&mut self, ee: &Ee, l: u32) -> ws::Objects {
        // The pair filter reads the count signed: none at or below 0 (S289's poked ignore lists).
        let (mode, at, n) = (ee.u32_at(l), ee.u32_at(l + 4), ee.i32_at(l + 8).max(0) as u32);
        ws::Objects { mode, list: (0..n).map(|i| self.item(ee.u32_at(at + 4 * i) + 0x18)).collect() }
    }

    /// Tree node `n` of structure `s` and every node under it, numbered as met.
    pub fn tree_node(&mut self, ee: &Ee, s: usize, n: u32, t: &mut bp::Quadtree) -> usize {
        let k = t.nodes.len();
        t.nodes.push(bp::Node::default());
        self.nodes[s].push(n);
        self.chunks[s].push(vec![]);
        let mut c = ee.u32_at(n + 4);
        while c != 0 {
            let slots = std::array::from_fn(|j| match ee.u32_at(c + 4 + 4 * j as u32) {
                0 => None,
                e => Some(self.item(e)),
            });
            t.nodes[k].chunks.push(slots);
            self.chunks[s][k].push(c);
            assert!(self.chunks[s][k].len() < 4096, "node {n:#x}'s chunks do not end");
            c = ee.u32_at(c);
        }
        let children = ee.u32_at(n);
        if children != 0 {
            for i in 0..4 {
                let ch = ee.u32_at(children + 4 * i as u32);
                if ch != 0 {
                    t.nodes[k].children[i] = Some(self.tree_node(ee, s, ch, t));
                }
            }
        }
        k
    }

    pub fn tree(&mut self, ee: &Ee, s: usize) -> bp::Quadtree {
        let a = self.structures[s];
        let mut t = bp::Quadtree {
            flags: ee.u32_at(a + 0xc),
            origin: [word_f(ee, a + 0x28), word_f(ee, a + 0x30)],
            extent: [word_f(ee, a + 0x40), word_f(ee, a + 0x44)],
            cell: ee.i32_at(a + 0x50),
            scale: word_f(ee, a + 0x54),
            busy: ee.u32_at(a + 0x890),
            cells: [0x8a0, 0x8a4, 0x8a8, 0x8ac].map(|o| ee.i32_at(a + o)),
            slot: ee.u32_at(a + 0xa4) as usize,
            ..Default::default()
        };
        let root = ee.u32_at(a + 0x8c);
        if root != 0 {
            t.root = Some(self.tree_node(ee, s, root, &mut t));
        }
        let mut l = ee.u32_at(a + 0x9c);
        while l != 0 {
            let n = ee.u32_at(l + 4);
            t.list.push(self.nodes[s].iter().position(|&x| x == n).unwrap_or_else(|| panic!("listed node {n:#x} is none of the tree's")));
            self.listed[s].push(l);
            assert!(t.list.len() < 4096, "the open query's list does not end");
            l = ee.u32_at(l);
        }
        t.head = (!t.list.is_empty()).then_some(0);
        t.at = match ee.u32_at(a + 0xa0) {
            0 => None,
            at => Some(self.listed[s].iter().position(|&x| x == at).unwrap_or_else(|| panic!("the query is at {at:#x}, off its list"))),
        };
        t.chunk = match ee.u32_at(a + 0xa8) {
            0 => None,
            c => Some(
                self.chunks[s]
                    .iter()
                    .enumerate()
                    .find_map(|(n, cs)| cs.iter().position(|&x| x == c).map(|k| (n, k)))
                    .unwrap_or_else(|| panic!("the query's chunk {c:#x} is none of the tree's")),
            ),
        };
        t
    }

    pub fn list(&mut self, ee: &Ee, s: usize) -> bp::Blocks {
        let a = self.structures[s];
        let (size, last, arr) = (ee.i32_at(a + 0x28), ee.i32_at(a + 0x30), ee.u32_at(a + 0x34));
        let blocks = (arr != 0).then(|| {
            let n = ee.u32_at(a + 0x2c);
            (0..n)
                .map(|b| {
                    let base = ee.u32_at(arr + 4 * b);
                    let used = if b + 1 == n { last } else { size };
                    (0..size)
                        .map(|i| match ee.u32_at(base + 4 * i as u32) {
                            _ if i >= used => None,
                            w if w & 2 != 0 => None,
                            w => Some(self.item(w)),
                        })
                        .collect()
                })
                .collect()
        });
        bp::Blocks { flags: ee.u32_at(a + 0xc), size, last, blocks, block: ee.i32_at(a + 0x3c), entry: ee.i32_at(a + 0x40) }
    }

    /// The world the gather round `probe` (a collidable) walks, as [`bp::Broad`] and
    /// [`ws::Collidable`]s model it: the probe's chain of spaces, their tables and the global one,
    /// each table's structures (the quadtree node by node, the list of moving entries slot by
    /// slot, and nothing else), and every entry they hold, with the collidables their object lists
    /// name. The probe is item 0.
    pub fn read(ee: &Ee, probe: u32) -> (GatherInRam, Vec<ws::Collidable>, bp::Broad) {
        let mut m = GatherInRam::default();
        m.item(probe + 0x18);
        let mut broad = bp::Broad { stamp: ee.u16_at(STAMP_AT), cell_size: word_f(ee, CELL_SIZE_AT), ..Default::default() };
        let mut l = ee.u32_at(probe + 0x20);
        while l != 0 {
            m.links.push(l);
            assert!(m.links.len() < 64, "the probe's spaces do not end");
            l = ee.u32_at(l);
        }
        for k in 0..m.links.len() {
            let tables = m.table(ee.u32_at(m.links[k] + 4));
            broad.spaces.push(bp::Space { next: (k + 1 < m.links.len()).then_some(k + 1), tables });
        }
        let global = ee.u32_at(GLOBAL_AT);
        broad.global = (global != 0).then(|| m.table(global));
        let mut k = 0;
        while k < m.tables.len() {
            let t = m.tables[k];
            let own = m.pair(ee, t);
            let (subs, n) = (ee.u32_at(t + 0x38), ee.u32_at(t + 0x3c));
            let subs = (0..n).map(|i| m.pair(ee, ee.u32_at(subs + 4 * i))).collect();
            broad.tables.push(bp::Tables { own, subs });
            k += 1;
        }
        m.nodes = vec![vec![]; m.structures.len()];
        m.chunks = vec![vec![]; m.structures.len()];
        m.listed = vec![vec![]; m.structures.len()];
        for s in 0..m.structures.len() {
            let a = m.structures[s];
            broad.structures.push(match ee.u32_at(a + 4) {
                TREE_VT => bp::Structure::Tree(m.tree(ee, s)),
                DB_LIST_VT => bp::Structure::List(m.list(ee, s)),
                vt => panic!("structure {a:#x} (vtable {vt:#x}) is neither the quadtree nor the list"),
            });
        }
        let trees = broad.structures.iter().filter(|s| matches!(s, bp::Structure::Tree(_))).count();
        assert!(trees <= 1, "the probe's tables hold {trees} quadtrees: the write-back follows one");
        m.current = ee.u32_at(CURRENT_AT);
        broad.current = m.structures.iter().position(|&x| x == m.current);
        let mut objects = vec![];
        let mut k = 0;
        while k < m.items.len() {
            let e = m.items[k];
            let class = ws::Class::from_byte(ee.u8_at(e + 2));
            let mut place = bp::Placement { stamp: ee.u16_at(e), ..Default::default() };
            let mut o = ws::Collidable { class, ..Default::default() };
            match class {
                ws::Class::Object => {
                    let c = e - 0x18;
                    place.centre = [0, 4, 8, 12].map(|i| word_f(ee, c + 0x50 + i));
                    o.flags = ee.u8_at(c + 0x126);
                    o.nearby = ee.u8_at(c + 0x45) & 1 != 0;
                    o.radius = word_f(ee, c + 0x70);
                    o.category = ee.u32_at(c + 0x94);
                    o.collides_with = ee.u32_at(c + 0xdc);
                    o.objects = m.objects(ee, c + 0xe0);
                    o.more_objects = m.objects(ee, c + 0xec);
                    let cs = c + 0xf8;
                    o.categories = ws::Categories { mode: ee.u32_at(cs), list: (0..ee.i32_at(cs + 8).max(0) as u32).map(|i| ee.u32_at(ee.u32_at(cs + 4) + 4 * i)).collect() };
                }
                ws::Class::Group => {
                    place.cell = [ee.u16_at(e + 0xc), ee.u16_at(e + 0x10)];
                    place.model = ee.u8_at(e + 0x12);
                    place.blend = ee.u8_at(e + 0x15) as i8;
                    let eco = ee.u32_at(ECO_AT);
                    broad.group_cell = word_f(ee, eco + 0xc4);
                    let mi = usize::from(place.model);
                    if broad.models.len() <= mi {
                        broad.models.resize(mi + 1, bp::Model::default());
                    }
                    let model = ee.u32_at(ee.u32_at(eco + 0x84) + 4 * mi as u32);
                    broad.models[mi] = bp::Model { near: word_f(ee, model + 0x10c), far: word_f(ee, model + 0x114) };
                }
                ws::Class::Other(_) => {}
            }
            objects.push(o);
            broad.places.push(place);
            k += 1;
        }
        broad.places[0].space = (!broad.spaces.is_empty()).then_some(0);
        (m, objects, broad)
    }

    /// The gather's outcome (`now`, read as `was`; `begun` the tree just after the query began, to
    /// tell a fresh list from a restarted one) written back where it changed: each entry's stamp and
    /// each collidable's flag byte, the global stamp and the tree last queried, every structure's
    /// walk state, the tree query's nodes taken from its pool and given back as the original takes
    /// and gives them, and the nearby list. What the gather never changes — the spaces, the tables,
    /// the trees' nodes, the lists' slots, every other collidable field — must not have changed.
    /// Answers how the tree's query went: `fresh`, `restarted`, or `off` the tree.
    pub fn write(&self, ee: &mut Ee, list: u32, was: (&[ws::Collidable], &bp::Broad), now: (&[ws::Collidable], &bp::Broad, &atv_world::Nearby), begun: &bp::Broad) -> &'static str {
        let (no, nb, nl) = now;
        let how = self.write_query(ee, was, (no, nb), begun);
        let arr = ee.u32_at(list);
        for (i, c) in nl.found.iter().enumerate() {
            ee.set_u32(arr + 8 * i as u32, self.items[c.object]);
            ee.set_u32(arr + 8 * i as u32 + 4, c.distance.0.to_bits());
        }
        ee.set_u32(list + 4, nl.found.len() as u32);
        for (i, w) in nl.centre.iter().enumerate() {
            ee.set_u32(list + 0x10 + 4 * i as u32, w.0.to_bits());
        }
        ee.set_u32(list + 0x20, nl.bound.0.to_bits());
        ee.set_u32(list + 0x30, u32::from(nl.built));
        how
    }

    /// [`GatherInRam::write`] without the nearby list: a query's outcome written back (F51's global
    /// search runs two queries and fills no list).
    pub fn write_query(&self, ee: &mut Ee, was: (&[ws::Collidable], &bp::Broad), now: (&[ws::Collidable], &bp::Broad), begun: &bp::Broad) -> &'static str {
        let mut how = "off";
        let (wo, wb) = was;
        let (no, nb) = now;
        assert_eq!((&wb.spaces, &wb.tables, wb.global), (&nb.spaces, &nb.tables, nb.global), "the spaces changed");
        for (k, (a, b)) in wo.iter().zip(no).enumerate() {
            let e = self.items[k];
            assert_eq!(ws::Collidable { flags: b.flags, ..a.clone() }, *b, "collidable {:#x} changed beyond its flags", e - 0x18);
            if a.flags != b.flags {
                set_u8(ee, e - 0x18 + 0x126, b.flags);
            }
            let (pa, pb) = (&wb.places[k], &nb.places[k]);
            assert_eq!(bp::Placement { stamp: pb.stamp, ..pa.clone() }, *pb, "entry {e:#x}'s place changed beyond its stamp");
            if pa.stamp != pb.stamp {
                set_u16(ee, e, pb.stamp);
            }
        }
        if wb.stamp != nb.stamp {
            set_u16(ee, STAMP_AT, nb.stamp);
        }
        if wb.current != nb.current {
            ee.set_u32(CURRENT_AT, nb.current.map_or(self.current, |s| self.structures[s]));
        }
        for (s, (sa, sb)) in wb.structures.iter().zip(&nb.structures).enumerate() {
            let a = self.structures[s];
            match (sa, sb, &begun.structures[s]) {
                (bp::Structure::Tree(x), bp::Structure::Tree(y), bp::Structure::Tree(z)) => {
                    assert_eq!((&x.nodes, x.root), (&y.nodes, y.root), "tree {a:#x}'s nodes changed");
                    let was_open = x.flags & 1 != 0;
                    let stamped = begun.stamp != wb.stamp;
                    let restart = was_open && z.flags & 1 != 0 && z.cells == x.cells && stamped;
                    let fresh = z.flags & 1 != 0 && stamped && !restart;
                    let pool = ee.u32_at(a + 0x94);
                    how = if restart { "restarted" } else if fresh { "fresh" } else { "off" };
                    if was_open && fresh {
                        query_pool::reset(ee, pool);
                    }
                    let mut taken = vec![];
                    if fresh {
                        let mut prev = 0;
                        for &n in z.list.iter().rev() {
                            let at = query_pool::take(ee, pool);
                            ee.set_u32(at, prev);
                            ee.set_u32(at + 4, self.nodes[s][n]);
                            prev = at;
                            taken.insert(0, at);
                        }
                    }
                    // A list the walk left open (F51's search ends so) keeps the nodes just taken.
                    let left_open = (fresh || restart || !stamped) && !y.list.is_empty() && y.list == z.list;
                    if (was_open || fresh) && !left_open {
                        query_pool::reset(ee, pool);
                    }
                    assert!(y.list.is_empty() || y.list == x.list || left_open, "tree {a:#x}'s query left a list open");
                    let open = if left_open && fresh { &taken } else { &self.listed[s] };
                    let listed = |i: Option<usize>| i.map_or(0, |i| open[i]);
                    ee.set_u32(a + 0xc, y.flags);
                    ee.set_u32(a + 0x890, y.busy);
                    for (i, c) in y.cells.iter().enumerate() {
                        ee.set_u32(a + 0x8a0 + 4 * i as u32, *c as u32);
                    }
                    ee.set_u32(a + 0x9c, listed(y.head));
                    ee.set_u32(a + 0xa0, listed(y.at));
                    ee.set_u32(a + 0xa4, y.slot as u32);
                    ee.set_u32(a + 0xa8, y.chunk.map_or(0, |(n, c)| self.chunks[s][n][c]));
                    if was_open || stamped {
                        ee.set_u32(a + 0xac, 0);
                    }
                }
                (bp::Structure::List(x), bp::Structure::List(y), _) => {
                    assert_eq!((x.size, x.last, &x.blocks), (y.size, y.last, &y.blocks), "list {a:#x}'s slots changed");
                    ee.set_u32(a + 0xc, y.flags);
                    ee.set_u32(a + 0x3c, y.block as u32);
                    ee.set_u32(a + 0x40, y.entry as u32);
                }
                _ => panic!("structure {a:#x} changed its kind"),
            }
        }
        how
    }
}

/// The nearby list at `l` (the bike's `+0xc4`) as [`atv_world::Nearby`] holds it, its candidates
/// left out (the gather starts it afresh).
pub fn nearby_in_ram(ee: &Ee, l: u32) -> atv_world::Nearby {
    atv_world::Nearby {
        found: vec![],
        centre: [0, 4, 8, 12].map(|i| word_f(ee, l + 0x10 + i)),
        bound: word_f(ee, l + 0x20),
        least: word_f(ee, l + 0x24),
        extra: word_f(ee, l + 0x28),
        no_groups: ee.u32_at(l + 0x2c) != 0,
        built: ee.u32_at(l + 0x30) != 0,
        capacity: ee.i32_at(l + 0x34),
        filter: ee.u32_at(l + 0x38) != 0,
        flat: ee.u32_at(l + 0x3c) != 0,
    }
}

/// **F46b — the collision sync's gather in Rust**: the world round `probe` (a collidable) and the
/// nearby list at `list` read out of RAM, [`atv_world::gather`] run with `kind`, and its marks,
/// stamps, walk state and list written back.
pub fn gather_in_ram(ee: &mut Ee, list: u32, probe: u32, kind: u32) -> &'static str {
    let (m, mut objects, mut broad) = GatherInRam::read(ee, probe);
    let mut near = nearby_in_ram(ee, list);
    let (was_o, was_b) = (objects.clone(), broad.clone());
    // The query's first structure begun as the gather begins it, on a copy: whether the tree's list
    // is fresh, and which.
    let mut begun = broad.clone();
    let space = begun.places[0].space.expect("a probe in a space");
    let (centre, l) = (begun.places[0].centre, near.least);
    let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
    bp::Query::begin(&mut begun, space, kind, centre, [l, l, l, F::ONE], Some((&mut lo, &mut hi)), true);
    atv_world::gather(&mut objects, &mut broad, &mut near, 0, kind);
    m.write(ee, list, (&was_o, &was_b), (&objects, &broad, &near), &begun)
}
