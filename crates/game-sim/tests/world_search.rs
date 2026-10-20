//! S44 — the nearby search and the pair filter against the original.
//!
//! Both on the save state beside C7's recording, on the plane's own collidables: its ray probe and its
//! sweep probe, and the first ten candidates its nearby search gathered there. Each case rewrites what
//! the search and the filter read on all twelve — class, flags, the nearby flag, reach, category and
//! mask, the three lists, listeners and callbacks — and the gathered list itself (repeats, the probe
//! itself, distances on the reach exactly), then:
//!
//! - `the_pair_filter_matches_the_original`: the original's filter on generated pairs, both ways of
//!   asking, against [`accepts`];
//! - `the_nearby_search_matches_the_original`: the original's search with one of the two probes. The
//!   probe's class vtable is swapped for one whose clear, prepare and narrow phase are hooks; the
//!   group's two part functions, the listeners' two slots and the callbacks are hooks too. Each hook logs
//!   its call; the narrow phase answers off a generated sequence, the part count and the parts too, the
//!   clear lowers the hit bit as the real ones do. The filter runs as the original. Compared: the
//!   return, every call in order with its arguments, and every collidable's flags and group.

use game_sim::world_search::{
    Candidate, Categories, Class, Collidable, Narrow, Nearby, Objects, accepts, flags, search_nearby,
};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const CASES: usize = 10_000;
const BUDGET: u64 = 2_000_000;

/// The collision object's fields: its nearby search and its two probes.
const NEARBY: u32 = 0x10c;
const RAY_PROBE: u32 = 0x1f0;
const SWEEP_PROBE: u32 = 0x1f4;

/// A collidable's fields.
mod at {
    pub const CLASS: u32 = 0x02;
    pub const NEARBY: u32 = 0x31;
    pub const CLASS_VTABLE: u32 = 0x38;
    pub const RADIUS: u32 = 0x60;
    pub const GROUP: u32 = 0x78;
    pub const CATEGORY: u32 = 0x84;
    pub const COLLIDES_WITH: u32 = 0xcc;
    pub const OBJECTS: u32 = 0xd0;
    pub const MORE_OBJECTS: u32 = 0xdc;
    pub const CATEGORIES: u32 = 0xe8;
    pub const LISTENERS: u32 = 0xf4;
    pub const ON_HIT: u32 = 0x10c;
    pub const ON_STRUCK: u32 = 0x110;
    pub const FLAGS: u32 = 0x116;
}

/// The search's fields.
mod search_at {
    pub const FOUND: u32 = 0x00;
    pub const COUNT: u32 = 0x04;
    pub const PREPARE: u32 = 0x38;
}

/// The probe class's vtable slots the search calls.
mod slot {
    pub const CLEAR: u32 = 0x90;
    pub const PAIR: u32 = 0x94;
    pub const PREPARE: u32 = 0xb0;
}

/// Where the hooks live: addresses no code reaches, each one a function the test stands in.
mod hook {
    pub const CLEAR: u32 = 0x2000;
    pub const PREPARE: u32 = 0x2010;
    pub const PAIR: u32 = 0x2020;
    pub const HEARD_HIT: u32 = 0x2030;
    pub const HEARD_STRUCK: u32 = 0x2040;
    /// Two callbacks of each kind, so the log says which one ran.
    pub const ON_HIT: [u32; 2] = [0x2050, 0x2060];
    pub const ON_STRUCK: [u32; 2] = [0x2070, 0x2080];
}

/// How many of the search's candidates join the two probes.
const OTHERS: usize = 10;
/// Listener objects, shared out among the collidables.
const LISTENERS: usize = 6;

fn plane() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: C7's save state, `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// A deterministic generator for the cases.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The scene: the twelve collidables (the two probes first), the listener objects, and the scratch the
/// lists, the vtables and the search live in.
struct Scene {
    objects: Vec<u32>,
    listeners: Vec<u32>,
    listener_vtable: u32,
    probe_vtable: u32,
    search: u32,
    found: u32,
    /// Per collidable, the scratch its four lists live in.
    lists: Vec<[u32; 4]>,
}

fn scene(ee: &Ee) -> Scene {
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    let near = ee.u32_at(rec + NEARBY);
    let found = ee.u32_at(near + search_at::FOUND);
    assert!(ee.u32_at(near + search_at::COUNT) as usize >= OTHERS, "the plane's search has fewer candidates than the test uses");
    let mut objects = vec![ee.u32_at(rec + RAY_PROBE), ee.u32_at(rec + SWEEP_PROBE)];
    objects.extend((0..OTHERS as u32).map(|i| ee.u32_at(found + 8 * i)));
    let mut k = 0;
    let mut next = || {
        k += 1;
        ee.scratch(k)
    };
    let listeners = (0..LISTENERS).map(|_| next()).collect();
    let listener_vtable = next();
    let probe_vtable = next();
    next();
    next();
    let search = next();
    let found = next();
    next();
    let lists = (0..objects.len()).map(|_| [next(), next(), next(), next()]).collect();
    Scene { objects, listeners, listener_vtable, probe_vtable, search, found, lists }
}

/// The port's view of an address: a collidable's index, or something no collidable is.
fn index_of(s: &Scene, a: u32) -> usize {
    s.objects.iter().position(|&o| o == a).unwrap_or(1_000_000 + a as usize)
}

fn write_list(ee: &mut Ee, at: u32, scratch: u32, mode: u32, items: &[u32]) {
    ee.set_u32(at, mode);
    ee.set_u32(at + 4, scratch);
    ee.set_u32(at + 8, items.len() as u32);
    for (i, &x) in items.iter().enumerate() {
        ee.set_u32(scratch + 4 * i as u32, x);
    }
}

/// One generated scene, written into the original's RAM.
fn generate(ee: &mut Ee, s: &Scene, p: &mut Pick) {
    // Few categories, so masks meet and miss in about equal measure.
    let mut category = |p: &mut Pick| match p.below(4) {
        0 => 0,
        _ => 1 << p.below(4),
    };
    for (i, &o) in s.objects.iter().enumerate() {
        let class = match p.below(8) {
            0 | 1 => 3,
            2 => [0, 1, 4][p.below(3) as usize],
            _ => 2,
        };
        ee.write(o + at::CLASS, 1, class);
        let f = (p.next() as u8 & !0x04) | if p.one_in(6) { 0 } else { 0x04 };
        ee.write(o + at::FLAGS, 1, u128::from(f));
        let nearby = (p.next() as u8 & !1) | u8::from(!p.one_in(6));
        ee.write(o + at::NEARBY, 1, u128::from(nearby));
        let r = if p.one_in(8) { 0.0 } else { p.range(0.5, 20.0) };
        ee.set_f32(o + at::RADIUS, r);
        ee.set_u32(o + at::GROUP, if p.one_in(2) { 0 } else { s.objects[p.below(s.objects.len() as u64) as usize] });
        let c = category(p) | if p.one_in(3) { category(p) } else { 0 };
        ee.set_u32(o + at::CATEGORY, c);
        // Two in three collidables are permissive, so that pairs get through the filter as often as
        // they are refused; the rest exercise each refusal.
        let open = p.below(3) != 0;
        let m = match p.below(6) {
            _ if open => !0,
            0 => 0,
            1 => category(p),
            _ => !category(p),
        };
        ee.set_u32(o + at::COLLIDES_WITH, m);
        for (k, off) in [at::OBJECTS, at::MORE_OBJECTS].into_iter().enumerate() {
            let n = if p.one_in(2) { 0 } else { 1 + p.below(3) as usize };
            let items: Vec<u32> = (0..n)
                .map(|_| if p.one_in(6) { 0x0123_4560 } else { s.objects[p.below(s.objects.len() as u64) as usize] })
                .collect();
            let mode = if open { [1, 2][p.below(2) as usize] } else { p.below(3) as u32 };
            write_list(ee, o + off, s.lists[i][k], mode, if open && mode == 1 { &[] } else { &items });
        }
        let n = if p.one_in(3) { 0 } else { 1 + p.below(3) as usize };
        let masks: Vec<u32> = (0..n).map(|_| category(p) | if p.one_in(4) { category(p) } else { 0 }).collect();
        let mode = if open { 1 } else { [0, 1, 1, 2][p.below(4) as usize] };
        let masks = if open { masks.into_iter().filter(|_| p.one_in(3)).map(|_| 0).collect() } else { masks };
        write_list(ee, o + at::CATEGORIES, s.lists[i][2], mode, &masks);
        let n = match p.below(4) {
            0 | 1 => 0,
            2 => 1,
            _ => 2 + p.below(2) as usize,
        };
        let ls: Vec<u32> = (0..n).map(|_| s.listeners[p.below(LISTENERS as u64) as usize]).collect();
        // The listener list: count, then (+8) the array.
        ee.set_u32(o + at::LISTENERS, ls.len() as u32);
        ee.set_u32(o + at::LISTENERS + 8, s.lists[i][3]);
        for (k, &l) in ls.iter().enumerate() {
            ee.set_u32(s.lists[i][3] + 4 * k as u32, l);
        }
        let cb = |p: &mut Pick, which: [u32; 2]| if p.one_in(2) { 0 } else { which[p.below(2) as usize] };
        let h = cb(p, hook::ON_HIT);
        ee.set_u32(o + at::ON_HIT, h);
        let h = cb(p, hook::ON_STRUCK);
        ee.set_u32(o + at::ON_STRUCK, h);
    }
}

fn objects_of(ee: &Ee, s: &Scene) -> Vec<Collidable> {
    let list = |base: u32| -> (u32, Vec<u32>) {
        let n = ee.i32_at(base + 8).max(0) as u32;
        let a = ee.u32_at(base + 4);
        (ee.u32_at(base), (0..n).map(|i| ee.u32_at(a + 4 * i)).collect())
    };
    s.objects
        .iter()
        .map(|&o| {
            let (mode, l) = list(o + at::OBJECTS);
            let objects = Objects { mode, list: l.iter().map(|&a| index_of(s, a)).collect() };
            let (mode, l) = list(o + at::MORE_OBJECTS);
            let more_objects = Objects { mode, list: l.iter().map(|&a| index_of(s, a)).collect() };
            let (mode, list) = list(o + at::CATEGORIES);
            let n = ee.u32_at(o + at::LISTENERS);
            let la = ee.u32_at(o + at::LISTENERS + 8);
            let some = |a: u32| (a != 0).then_some(a as usize);
            let group = ee.u32_at(o + at::GROUP);
            Collidable {
                class: Class::from_byte(ee.u8_at(o + at::CLASS)),
                flags: ee.u8_at(o + at::FLAGS),
                nearby: ee.u8_at(o + at::NEARBY) & 1 != 0,
                radius: F(ee.f32_at(o + at::RADIUS)),
                group: (group != 0).then(|| index_of(s, group)),
                category: ee.u32_at(o + at::CATEGORY),
                collides_with: ee.u32_at(o + at::COLLIDES_WITH),
                objects,
                more_objects,
                categories: Categories { mode, list },
                listeners: (0..n).map(|k| ee.u32_at(la + 4 * k) as usize).collect(),
                on_hit: some(ee.u32_at(o + at::ON_HIT)),
                on_struck: some(ee.u32_at(o + at::ON_STRUCK)),
            }
        })
        .collect()
}

#[test]
fn the_pair_filter_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let s = scene(&ee);
    let filter = addr("collision_pair_filter").unwrap();
    let mut p = Pick(0x5ea4_c4fa_0000_0044);
    let (mut yes, mut no, mut asked) = ([0usize; 2], [0usize; 2], 0);
    for case in 0..CASES {
        generate(&mut ee, &s, &mut p);
        let port = objects_of(&ee, &s);
        for _ in 0..4 {
            let a = p.below(s.objects.len() as u64) as usize;
            let b = if p.one_in(12) { a } else { p.below(s.objects.len() as u64) as usize };
            let nearby = p.one_in(2);
            ee.call(filter, &[u64::from(s.objects[a]), u64::from(s.objects[b]), u64::from(nearby)], &[], BUDGET);
            let want = ee.v0() as u32 != 0;
            let got = accepts(&port, a, b, nearby);
            assert_eq!(got, want, "case {case}: {a} against {b} (nearby {nearby}): the game says {want}");
            if want { yes[usize::from(nearby)] += 1 } else { no[usize::from(nearby)] += 1 }
            asked += 1;
        }
    }
    eprintln!("filter: {asked} pairs, accepted {yes:?}, refused {no:?} (without / with nearby)");
    assert!(yes.iter().chain(&no).all(|&n| n > 1000), "both answers, both ways of asking");
}

/// One call the search made outside itself, in addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Clear(u32),
    Prepare(u32),
    Pair(u32, u32),
    Parts(u32),
    Part(u32, u32),
    HeardHit(u32, u32, u32),
    HeardStruck(u32, u32, u32),
    OnHit(u32, u32, u32),
    OnStruck(u32, u32, u32),
}

/// What the hooks answer, in call order: the narrow phase's hits, the part counts and the parts.
#[derive(Default)]
struct Script {
    hits: Vec<bool>,
    counts: Vec<i32>,
    parts: Vec<u32>,
    at: [usize; 3],
}

impl Script {
    fn hit(&mut self) -> bool {
        self.at[0] += 1;
        self.hits[(self.at[0] - 1) % self.hits.len()]
    }
    fn count(&mut self) -> i32 {
        self.at[1] += 1;
        self.counts[(self.at[1] - 1) % self.counts.len()]
    }
    fn part(&mut self) -> u32 {
        self.at[2] += 1;
        self.parts[(self.at[2] - 1) % self.parts.len()]
    }
}

/// The port's side: the same script, the same log, in addresses.
struct Port<'a> {
    s: &'a Scene,
    script: Script,
    log: Vec<Call>,
}

impl Port<'_> {
    fn a(&self, i: usize) -> u32 {
        self.s.objects[i]
    }
}

impl Narrow for Port<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        objects[probe].flags &= !flags::HIT;
        self.log.push(Call::Clear(self.a(probe)));
    }
    fn prepare(&mut self, _: &mut [Collidable], probe: usize) {
        self.log.push(Call::Prepare(self.a(probe)));
    }
    fn pair(&mut self, _: &mut [Collidable], probe: usize, other: usize) -> bool {
        self.log.push(Call::Pair(self.a(probe), self.a(other)));
        self.script.hit()
    }
    fn parts(&mut self, _: &mut [Collidable], group: usize) -> usize {
        self.log.push(Call::Parts(self.a(group)));
        self.script.count().max(0) as usize
    }
    fn part(&mut self, _: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.log.push(Call::Part(self.a(group), i as u32));
        let p = self.script.part();
        (p != 0).then(|| index_of(self.s, p))
    }
    fn heard_hit(&mut self, _: &mut [Collidable], l: usize, probe: usize, other: usize) {
        self.log.push(Call::HeardHit(l as u32, self.a(probe), self.a(other)));
    }
    fn heard_struck(&mut self, _: &mut [Collidable], l: usize, struck: usize, probe: usize) {
        self.log.push(Call::HeardStruck(l as u32, self.a(struck), self.a(probe)));
    }
    fn on_hit(&mut self, _: &mut [Collidable], c: usize, probe: usize, other: usize) {
        self.log.push(Call::OnHit(c as u32, self.a(probe), self.a(other)));
    }
    fn on_struck(&mut self, _: &mut [Collidable], c: usize, struck: usize, probe: usize) {
        self.log.push(Call::OnStruck(c as u32, self.a(struck), self.a(probe)));
    }
}

#[test]
fn the_nearby_search_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let s = scene(&ee);
    let search_at = addr("collide_against_search").unwrap();
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let script: Rc<RefCell<Script>> = Rc::default();

    // The hooks: each logs its call; the narrow phase, the part count and the parts answer off the
    // script; the clear lowers the hit bit, as both real clears do.
    let a = |ee: &Ee, i: usize| ee.r(4 + i) as u32;
    {
        let log = log.clone();
        ee.hooks.insert(hook::CLEAR, Box::new(move |ee| {
            let probe = a(ee, 0);
            let f = ee.u8_at(probe + at::FLAGS) & !flags::HIT;
            ee.write(probe + at::FLAGS, 1, u128::from(f));
            log.borrow_mut().push(Call::Clear(probe));
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(hook::PREPARE, Box::new(move |ee| log.borrow_mut().push(Call::Prepare(a(ee, 0)))));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(hook::PAIR, Box::new(move |ee| {
            log.borrow_mut().push(Call::Pair(a(ee, 0), a(ee, 1)));
            let hit = script.borrow_mut().hit();
            ee.set(2, u64::from(hit));
        }));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(addr("group_part_count").unwrap(), Box::new(move |ee| {
            log.borrow_mut().push(Call::Parts(a(ee, 0)));
            let n = script.borrow_mut().count();
            ee.set(2, n as i64 as u64);
        }));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(addr("group_part").unwrap(), Box::new(move |ee| {
            log.borrow_mut().push(Call::Part(a(ee, 0), a(ee, 1)));
            let p = script.borrow_mut().part();
            ee.set(2, u64::from(p));
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(hook::HEARD_HIT, Box::new(move |ee| {
            log.borrow_mut().push(Call::HeardHit(a(ee, 0), a(ee, 1), a(ee, 2)));
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(hook::HEARD_STRUCK, Box::new(move |ee| {
            log.borrow_mut().push(Call::HeardStruck(a(ee, 0), a(ee, 1), a(ee, 2)));
        }));
    }
    for h in hook::ON_HIT {
        let log = log.clone();
        ee.hooks.insert(h, Box::new(move |ee| log.borrow_mut().push(Call::OnHit(h, a(ee, 0), a(ee, 1)))));
    }
    for h in hook::ON_STRUCK {
        let log = log.clone();
        ee.hooks.insert(h, Box::new(move |ee| log.borrow_mut().push(Call::OnStruck(h, a(ee, 0), a(ee, 1)))));
    }

    // The listener objects share one vtable; the probes get theirs.
    for &l in &s.listeners {
        ee.set_u32(l, s.listener_vtable);
    }
    ee.set_u32(s.listener_vtable + 8, hook::HEARD_HIT);
    ee.set_u32(s.listener_vtable + 0xc, hook::HEARD_STRUCK);
    ee.set_u32(s.probe_vtable + slot::CLEAR, hook::CLEAR);
    ee.set_u32(s.probe_vtable + slot::PAIR, hook::PAIR);
    ee.set_u32(s.probe_vtable + slot::PREPARE, hook::PREPARE);

    let mut p = Pick(0x5ea4_c4fa_0000_4404);
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for case in 0..CASES {
        generate(&mut ee, &s, &mut p);
        let probe = p.below(2) as usize;
        ee.set_u32(s.objects[probe] + at::CLASS_VTABLE, s.probe_vtable);
        // A probe that searches groups half the time.
        let f = ee.u8_at(s.objects[probe] + at::FLAGS) & !flags::NO_GROUPS | u8::from(p.one_in(2));
        ee.write(s.objects[probe] + at::FLAGS, 1, u128::from(f));

        // The gathered list: anyone, the probe included, now and then twice; distances inside, outside
        // and exactly on the probe's reach.
        let reach = ee.f32_at(s.objects[probe] + at::RADIUS);
        let n = p.below(9) as usize;
        let mut found = Vec::new();
        for k in 0..n {
            let o = if p.one_in(16) { probe } else { 2 + p.below(OTHERS as u64) as usize };
            let d = match p.below(8) {
                0 => reach,
                1 => p.range(reach, reach * 2.0 + 1.0),
                2 => 0.0,
                _ => p.range(0.0, reach),
            };
            ee.set_u32(s.found + 8 * k as u32, s.objects[o]);
            ee.set_f32(s.found + 8 * k as u32 + 4, d);
            found.push(Candidate { object: o, distance: F(d) });
        }
        ee.set_u32(s.search + search_at::FOUND, s.found);
        ee.set_u32(s.search + search_at::COUNT, n as u32);
        let prepare = p.one_in(3);
        ee.set_u32(s.search + search_at::PREPARE, u32::from(prepare));

        let hits: Vec<bool> = (0..1 + p.below(6)).map(|_| !p.one_in(3)).collect();
        let counts: Vec<i32> = (0..1 + p.below(3)).map(|_| [-1, 0, 1, 2, 3][p.below(5) as usize]).collect();
        let parts: Vec<u32> = (0..1 + p.below(4))
            .map(|_| if p.one_in(4) { 0 } else { s.objects[p.below(s.objects.len() as u64) as usize] })
            .collect();
        *script.borrow_mut() = Script { hits: hits.clone(), counts: counts.clone(), parts: parts.clone(), at: [0; 3] };
        log.borrow_mut().clear();

        let mut objects = objects_of(&ee, &s);
        let filtered = found
            .iter()
            .filter(|c| c.distance <= objects[probe].radius && objects[c.object].class == Class::Object)
            .filter(|c| !accepts(&objects, probe, c.object, true))
            .count();
        let mut port = Port { s: &s, script: Script { hits, counts, parts, at: [0; 3] }, log: Vec::new() };
        let got = search_nearby(&mut objects, probe, &Nearby { found, prepare }, &mut port);

        ee.call(search_at, &[u64::from(s.objects[probe]), u64::from(s.search)], &[], BUDGET);
        let want = ee.v0() != 0;

        assert_eq!(port.log, *log.borrow(), "case {case}: the calls out of the search");
        assert_eq!(got, want, "case {case}: the hit");
        let after = objects_of(&ee, &s);
        for (i, (g, w)) in objects.iter().zip(&after).enumerate() {
            assert_eq!(g.flags, w.flags, "case {case}: collidable {i}'s flags");
            assert_eq!(g.group, w.group, "case {case}: collidable {i}'s group");
        }

        let calls = log.borrow();
        let mut count = |k: &'static str, n: usize| *seen.entry(k).or_default() += n;
        count("hit", usize::from(want));
        count("pairs", calls.iter().filter(|c| matches!(c, Call::Pair(..))).count());
        count("parts", calls.iter().filter(|c| matches!(c, Call::Part(..))).count());
        count("heard hit", calls.iter().filter(|c| matches!(c, Call::HeardHit(..))).count());
        count("heard struck", calls.iter().filter(|c| matches!(c, Call::HeardStruck(..))).count());
        count("callbacks", calls.iter().filter(|c| matches!(c, Call::OnHit(..) | Call::OnStruck(..))).count());
        count("prepared", usize::from(prepare));
        count("objects filtered out", filtered);
        count("on the reach", (0..n).filter(|&k| ee.f32_at(s.found + 8 * k as u32 + 4) == reach).count());
    }
    let mut seen: Vec<_> = seen.into_iter().collect();
    seen.sort();
    eprintln!("nearby search over {CASES} cases: {seen:?}");
    for (k, n) in &seen {
        assert!(*n > 300, "{k} reached only {n} times");
    }
}
