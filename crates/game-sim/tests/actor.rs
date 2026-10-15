//! Actor lifetime, proved against the original's own manager.
//!
//! None of the functions under test is a pure function of its arguments: they build linked lists on the
//! heap, they intern names in two globals, and the searches read an actor's position through a
//! scene-graph call. Three things make them provable anyway.
//!
//! 1. **A Rust heap.** `operator new`, `operator new[]`, `operator delete` and `operator delete[]` are
//!    hooked with a bump allocator that hands out addresses in an area this test owns and *records every
//!    allocation and every free in order*. The game's own list code runs untouched on top of it, so the
//!    nodes, their links and their counts are the game's; what the hook adds is a log, which is how the
//!    teardown test can see the order a level comes down in.
//! 2. **One hooked scene call.** The node getter that turns a scene node into the world-space centre of
//!    its bounding box is matrix work on a node this test does not build, so it is replaced by a closure
//!    that writes a centre the test chose, looked up by the node pointer. Everything the searches
//!    *compute* — the subtract, the three squares, the square root, both comparisons — is the game's.
//! 3. **No stubs anywhere else.** The container, Add, Remove, both searches, the two dispatch passes,
//!    the record's constructor, the name interner and the manager's destructor all run as shipped.
//!
//! What is proved here: the container's order and every one of its operations; that the two per-frame
//! dispatch passes visit actors in registration order; Add and Remove over generated sequences (master
//! order, group order, group membership, the manager back-pointer, and the duplicate-add no-op); what
//! the original does with a group that has been emptied, which is the one case the port deliberately
//! does not reproduce; the name registries' bits, including the 33rd name and the 0x80-byte truncation;
//! the record constructor's field values; both searches over generated positions, with ties, touching
//! distances, exclusions, null nodes, cleared gates and kind masks; and the exact order the manager's
//! destructor releases a level in.
//!
//! What is **not** proved here is in the journal entry's "Not verified / not 1:1" list.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use game_sim::actor::{
    self, Actor, ActorId, Actors, Kind, Kinds, NAME_BYTES, NameBits, Registry, Released,
};
use ps2_float::Vf;
use ps2_oracle::{Ee, addr, game_or_skip};

/// Generous: the longest call here is a destructor walking a few dozen nodes.
const BUDGET: u64 = 4_000_000;

// --- where this test puts the objects the manager walks --------------------------------------------
// All well past the program image (which ends below 0x4f0000) and well below the interpreter's stack.

/// The manager object itself.
const MANAGER: u32 = 0x0080_0000;
/// A bare list object, for the container test.
const LIST: u32 = 0x0080_0100;
/// Actor records, one every [`RECORD_STRIDE`] bytes.
const RECORD0: u32 = 0x0081_0000;
const RECORD_STRIDE: u32 = 0x100;
/// Where a name string goes before it is interned.
const NAME0: u32 = 0x0082_0000;
const NAME_STRIDE: u32 = 0x200;
/// The quadword a search measures from, the mask it reads through a pointer, and its out-parameter.
const POINT: u32 = 0x0083_0000;
const MASK: u32 = 0x0083_0010;
const OUT_DISTANCE: u32 = 0x0083_0020;
/// Scene nodes: never dereferenced, because the one function that would is hooked.
const NODE0: u32 = 0x0084_0000;
const NODE_STRIDE: u32 = 0x10;
/// The vtable every record in this test points at.
const VTABLE: u32 = 0x0085_0000;
/// Addresses the hooks live at: not code, just somewhere for a table entry to point.
const HOOK_EARLY: u32 = 0x0086_0000;
const HOOK_LATE: u32 = 0x0086_0010;
/// The bump allocator's area.
const HEAP: u32 = 0x0090_0000;

// --- the fields this test reads and writes ----------------------------------------------------------

/// A list object: a count, a head and a tail.
const L_COUNT: u32 = 0x00;
const L_HEAD: u32 = 0x04;
const L_TAIL: u32 = 0x08;
/// A list node: the item, the previous node, the next node.
const N_ITEM: u32 = 0x00;
const N_PREV: u32 = 0x04;
const N_NEXT: u32 = 0x08;
/// The manager's two lists.
const M_MASTER: u32 = 0x30;
const M_GROUPS: u32 = 0x3c;
/// An actor record.
const R_KIND: u32 = 0x00;
const R_CLASSES: u32 = 0x04;
const R_ARG3: u32 = 0x0c;
const R_SEARCHABLE: u32 = 0x10;
const R_SECOND_FLAG: u32 = 0x14;
const R_NODE: u32 = 0x18;
const R_MANAGER: u32 = 0x1c;
const R_PLAYER: u32 = 0x20;
const R_VTABLE: u32 = 0x48;

/// One named address out of `research/oracle.txt`, or a panic saying which name is missing.
fn at(name: &str) -> u32 {
    addr(name).unwrap_or_else(|e| panic!("{e}"))
}

/// A tiny xorshift, so the generated sequences are the same on every machine and a failure can be
/// reproduced from the seed in the source alone.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// A coordinate from a small grid, so that equal distances actually happen.
    fn coord(&mut self) -> f32 {
        (self.below(9) as i64 - 4) as f32
    }
}

/// What the heap hooks saw, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    New { at: u32, size: u32 },
    Delete(u32),
}

/// The bump allocator behind `operator new`.
struct Heap {
    next: u32,
    log: Vec<Event>,
}

impl Heap {
    fn alloc(&mut self, size: u32) -> u32 {
        let at = self.next;
        self.next = (at + size.max(1) + 15) & !15;
        self.log.push(Event::New { at, size });
        at
    }
    /// Every address freed since the log was last cleared, in order.
    fn freed(&self) -> Vec<u32> {
        self.log
            .iter()
            .filter_map(|e| match e {
                Event::Delete(a) => Some(*a),
                Event::New { .. } => None,
            })
            .collect()
    }
}

/// The original's manager, running in the interpreter.
struct Original {
    ee: Ee,
    heap: Rc<RefCell<Heap>>,
    /// Node pointer -> the centre the hooked getter hands back for it.
    centres: Rc<RefCell<BTreeMap<u32, [f32; 4]>>>,
    /// The actors the dispatch passes visited, in order.
    visited: Rc<RefCell<Vec<u32>>>,
}

impl Original {
    /// `None` (with a printed reason) when there is no disc to read.
    fn new() -> Option<Original> {
        let mut ee = game_or_skip()?;
        let heap = Rc::new(RefCell::new(Heap {
            next: HEAP,
            log: Vec::new(),
        }));
        let centres: Rc<RefCell<BTreeMap<u32, [f32; 4]>>> = Rc::new(RefCell::new(BTreeMap::new()));
        let visited = Rc::new(RefCell::new(Vec::new()));

        // The heap. Both the scalar and the array forms, because the name interner uses the array one.
        for name in ["operator_new", "operator_new_array"] {
            let h = heap.clone();
            ee.hooks.insert(
                at(name),
                Box::new(move |ee: &mut Ee| {
                    let size = ee.r(4) as u32;
                    let block = h.borrow_mut().alloc(size);
                    for k in (0..(size + 3) & !3).step_by(4) {
                        ee.set_u32(block + k, 0);
                    }
                    ee.set(2, block as u64);
                }),
            );
        }
        for name in ["operator_delete", "operator_delete_array"] {
            let h = heap.clone();
            ee.hooks.insert(
                at(name),
                Box::new(move |ee: &mut Ee| {
                    let p = ee.r(4) as u32;
                    h.borrow_mut().log.push(Event::Delete(p));
                }),
            );
        }

        // The one scene call the searches make: hand back the centre this test chose for the node.
        let c = centres.clone();
        ee.hooks.insert(
            at("node_box_centre"),
            Box::new(move |ee: &mut Ee| {
                let node = ee.r(4) as u32;
                let out = ee.r(5) as u32;
                let extent = ee.r(6) as u32;
                let centre = *c
                    .borrow()
                    .get(&node)
                    .unwrap_or_else(|| panic!("the search asked for node {node:#x}, which this test did not place"));
                for (k, v) in centre.iter().enumerate() {
                    ee.set_f32(out + 4 * k as u32, *v);
                }
                // The original leaves w = 1.0 in both out quadwords; nothing reads the extent here.
                for k in 0..4 {
                    ee.set_f32(extent + 4 * k, if k == 3 { 1.0 } else { 0.0 });
                }
            }),
        );

        // The two per-frame dispatches call one virtual on every actor; these stand in for it.
        for hook in [HOOK_EARLY, HOOK_LATE] {
            let v = visited.clone();
            ee.hooks
                .insert(hook, Box::new(move |ee: &mut Ee| v.borrow_mut().push(ee.r(4) as u32)));
        }
        ee.set_u32(VTABLE + 0x28, HOOK_EARLY);
        ee.set_u32(VTABLE + 0x58, HOOK_LATE);

        Some(Original {
            ee,
            heap,
            centres,
            visited,
        })
    }

    fn call(&mut self, f: u32, ints: &[u64]) -> u64 {
        self.ee.call(f, ints, &[], BUDGET);
        self.ee.v0()
    }

    /// A manager with two empty lists, and a clear heap log.
    fn fresh_manager(&mut self) {
        self.call(at("actor_manager_ctor"), &[MANAGER as u64]);
        self.heap.borrow_mut().log.clear();
    }

    /// The items of a list object, head to tail, with the node each one is in.
    fn walk(&self, list: u32) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        let mut node = self.ee.u32_at(list + L_HEAD);
        while node != 0 {
            out.push((self.ee.u32_at(node + N_ITEM), node));
            node = self.ee.u32_at(node + N_NEXT);
        }
        out
    }

    fn items(&self, list: u32) -> Vec<u32> {
        self.walk(list).into_iter().map(|(item, _)| item).collect()
    }

    fn count(&self, list: u32) -> u32 {
        self.ee.u32_at(list + L_COUNT)
    }

    /// The master list's actors, in order.
    fn master(&self) -> Vec<u32> {
        self.items(MANAGER + M_MASTER)
    }

    /// The group objects, in order, each with its members in order.
    fn groups(&self) -> Vec<(u32, Vec<u32>)> {
        self.items(MANAGER + M_GROUPS)
            .into_iter()
            .map(|group| (group, self.items(group)))
            .collect()
    }

    /// Every link in a list is sound in both directions, and the count matches the walk. The container
    /// is intrusive and doubly linked, so a port that only ever walked forwards could miss a broken
    /// `prev` — this checks the backwards walk too.
    fn links_are_sound(&self, list: u32) {
        let forwards = self.walk(list);
        assert_eq!(
            self.count(list) as usize,
            forwards.len(),
            "the count word and the chain disagree"
        );
        match forwards.first() {
            None => assert_eq!(self.ee.u32_at(list + L_TAIL), 0, "an empty list with a tail"),
            Some(&(_, head)) => {
                assert_eq!(self.ee.u32_at(list + L_HEAD), head);
                assert_eq!(self.ee.u32_at(head + N_PREV), 0, "the head has a previous node");
            }
        }
        if let Some(&(_, tail)) = forwards.last() {
            assert_eq!(self.ee.u32_at(list + L_TAIL), tail);
            assert_eq!(self.ee.u32_at(tail + N_NEXT), 0, "the tail has a next node");
        }
        let mut backwards = Vec::new();
        let mut node = self.ee.u32_at(list + L_TAIL);
        while node != 0 {
            backwards.push((self.ee.u32_at(node + N_ITEM), node));
            node = self.ee.u32_at(node + N_PREV);
        }
        backwards.reverse();
        assert_eq!(forwards, backwards, "the two directions of the chain disagree");
    }

    /// Put a record where the constructor would leave one, without running it: the constructor is
    /// proved on its own, and the registry tests want kinds of their choosing.
    fn place_record(&mut self, index: u32, kind: u32, node: Option<u32>) -> u32 {
        let record = RECORD0 + index * RECORD_STRIDE;
        for k in (0..RECORD_STRIDE).step_by(4) {
            self.ee.set_u32(record + k, 0);
        }
        self.ee.set_u32(record + R_KIND, kind);
        self.ee.set_u32(record + R_SEARCHABLE, 1);
        self.ee.set_u32(record + R_NODE, node.unwrap_or(0));
        self.ee.set_u32(record + R_VTABLE, VTABLE);
        record
    }

    /// A string where the interner and the constructor can read it.
    fn place_name(&mut self, index: u32, name: &str) -> u32 {
        let at = NAME0 + index * NAME_STRIDE;
        for (k, b) in name.bytes().enumerate() {
            self.ee.write(at + k as u32, 1, b as u128);
        }
        self.ee.write(at + name.len() as u32, 1, 0);
        at
    }

    /// Both name registries emptied, as they are before a level builds.
    fn clear_name_registries(&mut self) {
        for registry in [at("actor_kind_names"), at("actor_class_names")] {
            for k in [L_COUNT, L_HEAD, L_TAIL] {
                self.ee.set_u32(registry + k, 0);
            }
        }
    }
}

/// The two sides of a registry test, kept in step: the original's manager and the port's.
struct Both {
    o: Original,
    actors: Actors,
    registry: Registry,
    /// Record address by id, and the reverse.
    address: Vec<u32>,
}

impl Both {
    fn new(o: Original) -> Both {
        Both {
            o,
            actors: Actors::new(),
            registry: Registry::new(),
            address: Vec::new(),
        }
    }

    /// One actor on both sides: a record in the interpreter and a record in the port.
    fn actor(&mut self, kind: u32, node: Option<Vf>) -> ActorId {
        let index = self.address.len() as u32;
        let node_pointer = node.map(|_| NODE0 + index * NODE_STRIDE);
        let record = self.o.place_record(index, kind, node_pointer);
        if let (Some(p), Some(centre)) = (node_pointer, node) {
            self.o.centres.borrow_mut().insert(p, centre.0);
        }
        self.address.push(record);
        let mut actor = Actor::new(Kind(kind));
        actor.node = node;
        let id = self.actors.insert(actor);
        assert_eq!(id, ActorId(index), "the port's ids and this test's indices have drifted apart");
        id
    }

    fn address_of(&self, id: ActorId) -> u32 {
        self.address[id.0 as usize]
    }

    fn id_of(&self, record: u32) -> ActorId {
        ActorId(
            self.address
                .iter()
                .position(|&a| a == record)
                .unwrap_or_else(|| panic!("{record:#x} is not one of this test's records")) as u32,
        )
    }

    /// Clear the word both searches gate on, in both places.
    fn set_searchable(&mut self, id: ActorId, on: bool) {
        let record = self.address_of(id);
        self.o.ee.set_u32(record + R_SEARCHABLE, on as u32);
        self.actors.get_mut(id).searchable = on;
    }

    /// The unmasked search, on both sides: the actor each found, and the distance word each wrote.
    fn nearest(&mut self, point: Vf, exclude: Option<ActorId>) -> ((Option<ActorId>, u32), (Option<ActorId>, u32)) {
        self.place_point(point);
        let who = self.o.call(
            at("actor_manager_nearest"),
            &[
                MANAGER as u64,
                POINT as u64,
                OUT_DISTANCE as u64,
                exclude.map_or(0, |id| self.address_of(id)) as u64,
            ],
        ) as u32;
        let found = self.registry.nearest(&self.actors, point, exclude);
        (
            (self.maybe_id(who), self.o.ee.u32_at(OUT_DISTANCE)),
            (found.actor, found.distance.0.to_bits()),
        )
    }

    /// The masked search, which reads its mask through a pointer.
    fn nearest_of(
        &mut self,
        mask: u32,
        point: Vf,
        exclude: Option<ActorId>,
    ) -> ((Option<ActorId>, u32), (Option<ActorId>, u32)) {
        self.place_point(point);
        self.o.ee.set_u32(MASK, mask);
        let who = self.o.call(
            at("actor_manager_nearest_kinds"),
            &[
                MANAGER as u64,
                MASK as u64,
                POINT as u64,
                OUT_DISTANCE as u64,
                exclude.map_or(0, |id| self.address_of(id)) as u64,
            ],
        ) as u32;
        let found = self.registry.nearest_of(&self.actors, Kinds(mask), point, exclude);
        (
            (self.maybe_id(who), self.o.ee.u32_at(OUT_DISTANCE)),
            (found.actor, found.distance.0.to_bits()),
        )
    }

    fn place_point(&mut self, point: Vf) {
        for (k, v) in point.0.iter().enumerate() {
            self.o.ee.set_f32(POINT + 4 * k as u32, *v);
        }
        self.o.ee.set_u32(OUT_DISTANCE, 0xcccc_cccc);
    }

    fn maybe_id(&self, record: u32) -> Option<ActorId> {
        (record != 0).then(|| self.id_of(record))
    }

    fn add(&mut self, id: ActorId) {
        let record = self.address_of(id);
        self.o.call(at("actor_manager_add"), &[MANAGER as u64, record as u64]);
        self.registry.add(&mut self.actors, id);
    }

    fn remove(&mut self, id: ActorId) {
        let record = self.address_of(id);
        self.o.call(at("actor_manager_remove"), &[MANAGER as u64, record as u64]);
        self.registry.remove(&mut self.actors, id);
    }

    /// Everything about the registry that is observable, compared.
    fn agree(&self, after: &str) {
        let master: Vec<ActorId> = self.o.master().into_iter().map(|a| self.id_of(a)).collect();
        assert_eq!(
            master,
            self.registry.iter().collect::<Vec<_>>(),
            "the master list's order differs after {after}"
        );
        assert_eq!(
            self.o.count(MANAGER + M_MASTER),
            self.registry.count(),
            "the master count differs after {after}"
        );
        self.o.links_are_sound(MANAGER + M_MASTER);
        self.o.links_are_sound(MANAGER + M_GROUPS);

        let groups: Vec<Vec<ActorId>> = self
            .o
            .groups()
            .into_iter()
            .map(|(group, members)| {
                self.o.links_are_sound(group);
                members.into_iter().map(|a| self.id_of(a)).collect()
            })
            .collect();
        let ported: Vec<Vec<ActorId>> = self
            .registry
            .groups()
            .iter()
            .map(|g| g.members().iter().copied().collect())
            .collect();
        assert_eq!(groups, ported, "the groups differ after {after}");

        for (index, &record) in self.address.iter().enumerate() {
            let id = ActorId(index as u32);
            let registered = self.o.ee.u32_at(record + R_MANAGER);
            assert_eq!(
                registered != 0,
                self.actors.get(id).registered,
                "{id:?}'s manager back-pointer ({registered:#x}) differs after {after}"
            );
            if registered != 0 {
                assert_eq!(registered, MANAGER, "{id:?} points at something that is not the manager");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The container
// ---------------------------------------------------------------------------------------------------

/// Four hundred generated container operations — append, append a duplicate, remove something present,
/// remove something absent, find, index, count and clear — against the master list's own instantiation
/// of the engine's list. After every single one: the items head to tail, the count word, and that the
/// chain is sound in both directions.
///
/// The return values are compared too, because three of them carry information the port reproduces: the
/// node a find lands on (its position from the head), the item a remove hands back, and the zero an
/// index past the end gives rather than the tail.
#[test]
fn the_container_matches_the_originals_list() {
    let Some(mut o) = Original::new() else { return };
    o.call(at("actor_list_ctor"), &[LIST as u64]);
    let mut port: actor::List<u32> = actor::List::new();

    // Items are opaque to the container: it compares them and stores them, and never follows one.
    let pool: Vec<u32> = (1..=12u32).map(|k| 0x1100 + k * 0x40).collect();
    let mut pick = Pick(0x5eed_0002);
    let mut appends = 0u32;
    let mut frees = 0u32;

    for step in 0..400 {
        let what = pick.below(if step % 97 == 96 { 8 } else { 7 });
        match what {
            0 | 1 => {
                // 0: whatever comes next out of the pool. 1: an item the list may already hold, which
                // the original is happy to store twice.
                let item = pool[pick.below(pool.len() as u64) as usize];
                o.heap.borrow_mut().log.clear();
                let got = o.call(at("list_append"), &[LIST as u64, item as u64]);
                assert_eq!(got, 1, "step {step}: the original's append failed");
                assert_eq!(port.append(item), got == 1, "step {step}: append disagreed");
                appends += 1;
                // One twelve-byte node per entry, and it is the new tail.
                let log = o.heap.borrow().log.clone();
                assert_eq!(log.len(), 1, "step {step}: an append allocated {log:?}");
                let Event::New { at: node, size } = log[0] else {
                    panic!("step {step}: an append freed something")
                };
                assert_eq!(size, 0xc, "step {step}: a node is not twelve bytes");
                assert_eq!(o.ee.u32_at(LIST + L_TAIL), node, "step {step}: the node is not the tail");
            }
            2 | 3 => {
                // Present about half the time; the absent case must change nothing at all.
                let item = if what == 2 && !port.is_empty() {
                    *port.at(pick.below(port.count() as u64) as i32).expect("in range")
                } else {
                    0x7000 + pick.below(4) as u32
                };
                o.heap.borrow_mut().log.clear();
                let got = o.call(at("actor_list_remove"), &[LIST as u64, item as u64]) as u32;
                assert_eq!(
                    port.remove(&item).unwrap_or(0),
                    got,
                    "step {step}: remove({item:#x}) handed back something else"
                );
                let freed = o.heap.borrow().freed();
                if got == 0 {
                    assert!(freed.is_empty(), "step {step}: a failed remove freed {freed:?}");
                } else {
                    assert_eq!(freed.len(), 1, "step {step}: a remove freed {freed:?}");
                    frees += 1;
                }
            }
            4 => {
                let item = pool[pick.below(pool.len() as u64) as usize];
                let node = o.call(at("list_find"), &[LIST as u64, item as u64]) as u32;
                match port.find(&item) {
                    None => assert_eq!(node, 0, "step {step}: the port missed {item:#x}"),
                    Some(index) => assert_eq!(
                        node,
                        o.walk(LIST)[index as usize].1,
                        "step {step}: find landed on a different node than index {index}"
                    ),
                }
            }
            5 => {
                let index = pick.below(port.count() as u64 + 4) as i64 - 2;
                let got = o.call(at("actor_list_at"), &[LIST as u64, index as u64]) as u32;
                assert_eq!(
                    port.at(index as i32).copied().unwrap_or(0),
                    got,
                    "step {step}: item {index} steps from the head"
                );
            }
            6 => {
                let got = o.call(at("actor_list_count"), &[LIST as u64]) as u32;
                assert_eq!(port.count(), got, "step {step}: the count");
            }
            _ => {
                let held = port.count();
                o.heap.borrow_mut().log.clear();
                o.call(at("actor_list_clear"), &[LIST as u64]);
                port.clear();
                assert_eq!(
                    o.heap.borrow().freed().len() as u32,
                    held,
                    "step {step}: a clear freed a different number of nodes than the list held"
                );
                frees += held;
            }
        }
        assert_eq!(o.items(LIST), port.as_slice(), "step {step}: the items differ");
        assert_eq!(o.count(LIST), port.count(), "step {step}: the count word differs");
        o.links_are_sound(LIST);
    }
    assert!(appends > 100 && frees > 30, "the generated sequence hardly exercised anything");
}

/// The original's list can hold the same item twice, and then `find` and `remove` both mean the first
/// one. Worth its own test because it is the one place the port's `Vec` could plausibly differ, and
/// because the registry relies on the opposite — [`Registry::add`]'s duplicate guard is what keeps a
/// double registration out of the master list.
#[test]
fn a_duplicated_item_is_found_and_removed_at_its_first_place() {
    let Some(mut o) = Original::new() else { return };
    o.call(at("actor_list_ctor"), &[LIST as u64]);
    let mut port: actor::List<u32> = actor::List::new();
    for item in [0x1000u32, 0x2000, 0x1000, 0x3000, 0x1000] {
        o.call(at("list_append"), &[LIST as u64, item as u64]);
        port.append(item);
    }
    let first = o.walk(LIST)[0].1;
    assert_eq!(o.call(at("list_find"), &[LIST as u64, 0x1000]) as u32, first);
    assert_eq!(port.find(&0x1000), Some(0));

    o.call(at("actor_list_remove"), &[LIST as u64, 0x1000]);
    port.remove(&0x1000);
    assert_eq!(o.items(LIST), port.as_slice());
    assert_eq!(o.items(LIST), [0x2000, 0x1000, 0x3000, 0x1000]);
    assert_eq!(o.count(LIST), 4, "the count followed the removal");
}

// ---------------------------------------------------------------------------------------------------
// Registration, and the order it fixes
// ---------------------------------------------------------------------------------------------------

/// Add and Remove over three hundred generated operations, with every observable thing compared after
/// each one: the master list's order and count, the group list's order, each group's membership, the
/// soundness of all those chains, and every actor's manager back-pointer.
///
/// The generated sequence keeps one invariant: it never removes the last registered actor of a kind.
/// That is not to make the port look good — it is the domain the original is defined on, and the case
/// outside it has a test of its own below.
#[test]
fn add_and_remove_match_the_original() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();

    // Kinds as a level has them — a few names interned, so a few bits — plus a zero kind, which is what
    // an actor whose type name came after the registry's 32nd gets.
    let kinds = [1u32, 2, 4, 8, 1, 2, 4, 1, 0, 0, 2, 16];
    let ids: Vec<ActorId> = kinds.iter().map(|&k| both.actor(k, None)).collect();
    both.agree("the world was built");

    let mut pick = Pick(0x5eed_0003);
    let mut registered: Vec<ActorId> = Vec::new();
    let mut adds = 0;
    let mut removes = 0;
    let mut no_ops = 0;

    for step in 0..300 {
        let id = ids[pick.below(ids.len() as u64) as usize];
        let kind = kinds[id.0 as usize];
        let already = registered.contains(&id);
        let last_of_kind = registered
            .iter()
            .filter(|&&other| kinds[other.0 as usize] == kind)
            .count()
            == 1;
        if pick.below(3) == 0 {
            // Remove: an unregistered actor (a no-op the original handles) or a registered one, so long
            // as it is not the last of its kind.
            if already && last_of_kind {
                continue;
            }
            both.remove(id);
            if already {
                registered.retain(|&other| other != id);
                removes += 1;
            } else {
                no_ops += 1;
            }
            both.agree(&format!("step {step}: remove {id:?}"));
        } else {
            let before = both.registry.count();
            both.add(id);
            if already {
                no_ops += 1;
                assert_eq!(
                    both.registry.count(),
                    before,
                    "step {step}: a second registration grew the master list"
                );
            } else {
                registered.push(id);
                adds += 1;
            }
            both.agree(&format!("step {step}: add {id:?}"));
        }
    }
    assert!(
        adds > 40 && removes > 20 && no_ops > 20,
        "the generated sequence hardly exercised anything: {adds} adds, {removes} removes, {no_ops} no-ops"
    );
}

/// A second registration is ignored outright: not appended to the master list, and not filed in the
/// group a second time either. The guard is a `find` on the master list, so the test also checks that
/// an actor the manager has *forgotten* can be registered again and lands at the tail.
#[test]
fn registering_twice_changes_nothing() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let a = both.actor(1, None);
    let b = both.actor(1, None);
    both.add(a);
    both.add(b);
    both.add(a);
    both.add(a);
    both.agree("a was registered three times");
    assert_eq!(both.o.master(), [both.address_of(a), both.address_of(b)]);
    assert_eq!(both.o.groups().len(), 1, "one kind, one group");
    assert_eq!(both.o.groups()[0].1.len(), 2, "and two members, not four");

    both.remove(a);
    both.add(a);
    both.agree("a was removed and registered again");
    assert_eq!(
        both.o.master(),
        [both.address_of(b), both.address_of(a)],
        "a re-registered actor goes to the tail — its place in every walk changes"
    );
    assert_eq!(
        both.registry.iter().collect::<Vec<_>>(),
        [b, a],
        "and the port agrees, which is the whole point of the module"
    );
}

/// Groups are keyed on *whichever actor is at the group's head*, read fresh every time, and are
/// ordered by the first time each kind was seen. So the first actor of a kind decides both which group
/// its kind's actors go in and where that group sits in the list — and removing a group's head hands
/// the key to the next member, which is only the same kind because every member shares it.
#[test]
fn groups_are_keyed_on_their_head_and_ordered_by_first_sighting() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let four = both.actor(4, None);
    let one_a = both.actor(1, None);
    let one_b = both.actor(1, None);
    let two = both.actor(2, None);
    for id in [four, one_a, two, one_b] {
        both.add(id);
    }
    both.agree("four kinds registered out of order");
    let groups = both.o.groups();
    assert_eq!(
        groups.iter().map(|(_, m)| m.len()).collect::<Vec<_>>(),
        [1, 2, 1],
        "three groups: kind 4, kind 1 (two members), kind 2 — in first-sighting order"
    );
    assert_eq!(groups[1].1, [both.address_of(one_a), both.address_of(one_b)]);
    for (index, kind) in [4u32, 1, 2].iter().enumerate() {
        let group = both
            .registry
            .group_of(&both.actors, Kind(*kind))
            .expect("the port has a group for every kind registered");
        assert_eq!(
            group.kind(&both.actors),
            Some(Kind(*kind)),
            "the port's group {index} answers with its head member's kind"
        );
    }

    // Take the head of the kind-1 group away: the group survives, and its key now comes from the actor
    // that was second.
    both.remove(one_a);
    both.agree("the head of a group was removed");
    let groups = both.o.groups();
    assert_eq!(groups[1].1, [both.address_of(one_b)], "the group kept its place in the list");
    assert_eq!(
        both.registry.group_of(&both.actors, Kind(1)).map(|g| g.members().count()),
        Some(1)
    );
}

/// Both per-frame dispatch passes walk the master list from the head and call one virtual function on
/// every actor, so the order they visit in *is* registration order — including after a removal and a
/// re-registration, which moves an actor to the end of the frame.
///
/// This is the one test that watches the order the rest of the engine actually sees, rather than the
/// list the order comes out of.
#[test]
fn the_dispatch_passes_visit_in_registration_order() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let ids: Vec<ActorId> = [1u32, 2, 1, 4, 2, 1].iter().map(|&k| both.actor(k, None)).collect();

    let mut pick = Pick(0x5eed_0004);
    let mut order: Vec<ActorId> = Vec::new();
    for round in 0..24 {
        // Register, re-register or (where it does not empty a group) unregister one actor, then watch a
        // frame's two passes.
        let id = ids[pick.below(ids.len() as u64) as usize];
        if order.contains(&id) && pick.below(2) == 0 {
            let kind = both.actors.get(id).kind;
            let others = order
                .iter()
                .filter(|&&o| both.actors.get(o).kind == kind)
                .count();
            if others > 1 {
                both.remove(id);
                order.retain(|&other| other != id);
            }
        } else if !order.contains(&id) {
            both.add(id);
            order.push(id);
        }
        both.agree(&format!("round {round}"));

        for pass in ["actor_manager_each_early", "actor_manager_each_late"] {
            both.o.visited.borrow_mut().clear();
            both.o.call(at(pass), &[MANAGER as u64]);
            let visited: Vec<ActorId> = both
                .o
                .visited
                .borrow()
                .iter()
                .map(|&record| both.id_of(record))
                .collect();
            assert_eq!(visited, order, "round {round}: {pass} visited in a different order");
            assert_eq!(
                visited,
                both.registry.iter().collect::<Vec<_>>(),
                "round {round}: the port's iteration order differs from {pass}"
            );
        }
    }
    assert!(order.len() >= 3, "the generated rounds registered hardly anything");
}

/// What the original does with a group that has been emptied — the one case this port deliberately
/// does not reproduce, written down as a test so that the deviation is a measured difference rather
/// than a hope.
///
/// Remove the only actor of a kind and the group stays in the group list with a null head. The next
/// lookup still asks every group for its kind, and it asks by reading the head node's item and then
/// that item's first word: with a null head, that is a read of the word at address zero, used as a
/// pointer. This test plants a pointer there, so the question "what does the original do?" has a
/// visible answer: the empty group claims the kind the planted word names, and the next actor of that
/// kind is filed into it.
///
/// On the console the word at address zero belongs to the kernel, and the game would dereference it
/// and fault; no level can reach this, because reaching it means a kind went from one actor to none
/// and then something else was registered or removed. The port skips an empty group instead.
#[test]
fn an_emptied_group_is_where_the_port_stops_following() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let one = both.actor(1, None);
    let two = both.actor(2, None);
    let seven = both.actor(7, None);
    both.add(one);
    both.add(two);
    both.remove(one);
    both.agree("the only actor of kind 1 was removed");
    assert_eq!(
        both.o.groups().iter().map(|(_, m)| m.len()).collect::<Vec<_>>(),
        [0, 1],
        "the emptied group is still in the list, with no members"
    );

    // A word that says "kind 7", and address zero pointing at it.
    const PLANTED: u32 = 0x0083_0100;
    both.o.ee.set_u32(PLANTED, 7);
    both.o.ee.set_u32(0, PLANTED);

    both.o.call(at("actor_manager_add"), &[MANAGER as u64, both.address_of(seven) as u64]);
    both.registry.add(&mut both.actors, seven);

    assert_eq!(
        both.o.groups().iter().map(|(_, m)| m.clone()).collect::<Vec<_>>(),
        [vec![both.address_of(seven)], vec![both.address_of(two)]],
        "the original read the planted word through the empty group and filed the actor there"
    );
    let ported: Vec<Vec<ActorId>> = both
        .registry
        .groups()
        .iter()
        .map(|g| g.members().iter().copied().collect())
        .collect();
    assert_eq!(
        ported,
        vec![vec![], vec![two], vec![seven]],
        "the port skipped the empty group and made a new one at the tail"
    );
    assert_eq!(
        both.o.master().into_iter().map(|a| both.id_of(a)).collect::<Vec<_>>(),
        both.registry.iter().collect::<Vec<_>>(),
        "the master list — the order everything iterates in — agrees even here"
    );
}

// ---------------------------------------------------------------------------------------------------
// Interned names: the kind word and the class mask
// ---------------------------------------------------------------------------------------------------

/// The name registry's bits, against the original's interner: a fresh name gets the next bit, a name
/// already registered gets its own bit back and allocates nothing, the 32nd name gets `0x8000_0000`
/// and every name after that gets zero, and two names that agree in their first 0x80 bytes are one
/// name.
#[test]
fn interned_names_get_the_bits_the_original_gives() {
    let Some(mut o) = Original::new() else { return };
    o.clear_name_registries();
    let registry = at("actor_kind_names");
    let mut port = NameBits::new();

    // Thirty-six distinct names (four past the cliff), a long one, a longer one that agrees with it for
    // exactly 0x80 bytes, and repeats of the first few interleaved.
    let long = "L".repeat(NAME_BYTES - 1) + "A";
    let longer = "L".repeat(NAME_BYTES - 1) + "A" + "but this is never looked at";
    let mut names: Vec<String> = (0..36).map(|k| format!("Actor{k:02}Type")).collect();
    names.insert(5, long.clone());
    names.insert(9, longer.clone());
    for k in 0..6 {
        names.push(format!("Actor{k:02}Type"));
    }
    names.push(long.clone());

    for (step, name) in names.iter().enumerate() {
        let string = o.place_name(step as u32, name);
        let before = o.count(registry);
        o.heap.borrow_mut().log.clear();
        let bit = o.call(at("name_bit"), &[registry as u64, string as u64]) as u32;
        assert_eq!(port.bit(name), bit, "step {step}: the bit for {name:?}");
        assert_eq!(port.count(), o.count(registry), "step {step}: the number of names registered");

        let fresh = o.count(registry) != before;
        let log = o.heap.borrow().log.clone();
        if fresh {
            // A fresh name is a 0x80-byte block plus a node, and the block holds the name truncated to
            // 0x80 bytes with no terminator of its own when it is that long.
            assert_eq!(log.len(), 2, "step {step}: registering a name allocated {log:?}");
            let Event::New { at: block, size } = log[0] else {
                panic!("step {step}: a name registration freed something")
            };
            assert_eq!(size, NAME_BYTES as u32, "step {step}: a name block is not 0x80 bytes");
            let mut expected = [0u8; NAME_BYTES];
            let bytes = name.as_bytes();
            let n = bytes.len().min(NAME_BYTES);
            expected[..n].copy_from_slice(&bytes[..n]);
            let stored: Vec<u8> = (0..NAME_BYTES as u32).map(|k| o.ee.u8_at(block + k)).collect();
            assert_eq!(stored, expected, "step {step}: the stored name");
        } else {
            assert!(log.is_empty(), "step {step}: a name already registered allocated {log:?}");
        }
        o.links_are_sound(registry);
    }

    // Thirty-six generated names plus the long one — the longer one is the same name, truncated.
    assert_eq!(port.count(), 37, "the two long names are one name");
    assert_eq!(port.peek(&long), port.peek(&longer), "0x80 bytes is the whole of a name");
    // The long name took slot 5, so every generated name from `Actor05Type` on is one slot later.
    assert_eq!(port.peek("Actor30Type"), Some(0x8000_0000), "the 32nd name gets the top bit");
    assert_eq!(port.peek("Actor31Type"), Some(0), "the 33rd name and everything after it gets zero");
    assert_eq!(port.peek("Actor35Type"), Some(0), "including the last one registered");
    assert_eq!(port.peek("never registered"), None);
    assert!(
        !Kinds::ALL.has(Kind(0)),
        "a zero kind is in no mask at all, which is what makes the cliff matter"
    );
}

/// The record constructor, field by field, and the class descriptors that OR an actor's "is-a" bits
/// into it — against [`Actors::create`], which is the port's whole creation path.
///
/// Two of the words checked here are ones the port does not model (`+0x0c`, which is simply the
/// constructor's third argument, and the second flag word beside the searches'); they are asserted so
/// that the journal entry's description of them is a measurement.
#[test]
fn the_record_constructor_matches_the_original() {
    let Some(mut o) = Original::new() else { return };
    o.clear_name_registries();
    let mut kinds = NameBits::new();
    let mut classes = NameBits::new();

    // Three actors, with overlapping hierarchies: the shared base classes must come out as shared bits.
    let hierarchies: [(&str, &[&str]); 4] = [
        ("MonsterTruck", &["Object", "Vehicle", "MonsterTruck"]),
        ("MotocrossRider", &["Object", "Vehicle", "MotocrossRider"]),
        ("Prop", &["Object", "Prop"]),
        ("MonsterTruck", &["Object", "Vehicle", "MonsterTruck"]),
    ];
    for (index, (type_name, hierarchy)) in hierarchies.iter().enumerate() {
        let record = RECORD0 + index as u32 * RECORD_STRIDE;
        for k in (0..RECORD_STRIDE).step_by(4) {
            o.ee.set_u32(record + k, 0xdead_beef); // so a word the constructor leaves alone shows up
        }
        let name = o.place_name(index as u32, type_name);
        let arg3 = 0x0101_0000 + index as u32;
        o.call(at("actor_record_ctor"), &[record as u64, name as u64, arg3 as u64]);

        // Then one class descriptor per level of the hierarchy, each ORing its own bit into the record.
        let mut levels = 0u32;
        for (level, class) in hierarchy.iter().enumerate() {
            let descriptor = 0x0087_0000 + (level as u32) * 0x10;
            let string = o.place_name(16 + level as u32, class);
            o.call(
                at("actor_class_descriptor_ctor"),
                &[descriptor as u64, string as u64, (record + R_CLASSES) as u64, 0x2000 + level as u64],
            );
            assert_eq!(
                o.ee.u32_at(descriptor),
                0x2000 + level as u32,
                "level {level} of {type_name}: the descriptor keeps its first argument"
            );
            let bit = o.ee.u32_at(descriptor + 0x04);
            assert_eq!(bit.count_ones(), 1, "level {level} of {type_name}: a class bit is one bit");
            levels |= bit;
        }
        assert_eq!(
            o.ee.u32_at(record + R_CLASSES),
            levels,
            "{type_name}: the record's mask is the OR of its levels' bits and nothing else"
        );

        let mut port = Actors::new();
        let id = port.create(&mut kinds, &mut classes, type_name, hierarchy);
        let actor = port.get(id);
        assert_eq!(o.ee.u32_at(record + R_KIND), actor.kind.0, "{type_name}: the kind bit");
        assert_eq!(o.ee.u32_at(record + R_CLASSES), actor.classes, "{type_name}: the class mask");
        assert_eq!(o.ee.u32_at(record + R_SEARCHABLE) != 0, actor.searchable, "{type_name}: the search gate");
        assert_eq!(o.ee.u32_at(record + R_NODE), 0, "{type_name}: no scene node yet");
        assert!(actor.node.is_none());
        assert_eq!(o.ee.u32_at(record + R_MANAGER), 0, "{type_name}: and no manager — creating is not registering");
        assert!(!actor.registered);
        assert_eq!(o.ee.u32_at(record + R_PLAYER), 0, "{type_name}: and no player");
        assert!(actor.player.is_none());
        // The words the port does not model, measured rather than assumed.
        assert_eq!(o.ee.u32_at(record + R_ARG3), arg3, "{type_name}: +0x0c is the third argument");
        assert_eq!(o.ee.u32_at(record + R_SECOND_FLAG), 1, "{type_name}: the second flag word starts set");
        assert_eq!(o.ee.u32_at(record + 0x08), 0, "{type_name}: +0x08 starts cleared");
        for list in [0x24u32, 0x30, 0x3c] {
            for word in [L_COUNT, L_HEAD, L_TAIL] {
                assert_eq!(
                    o.ee.u32_at(record + list + word),
                    0,
                    "{type_name}: the sub-list at +{list:#x} starts empty"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The two searches
// ---------------------------------------------------------------------------------------------------

/// Six hundred generated searches — three hundred points through each of the two searches — against
/// the original's, comparing both the actor found and the float written into the out-parameter.
///
/// The actors are placed on a small integer grid on purpose: equal distances then happen several times
/// a run, and an equal distance is the case where iteration order decides the answer. The set includes
/// an actor with no scene node, an actor whose search gate has been cleared, and an actor of kind zero
/// — which the unmasked search finds and the masked one never can, because it has no mask rather than
/// a full one.
#[test]
fn both_searches_match_the_original() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let mut pick = Pick(0x5eed_0005);

    let mut ids = Vec::new();
    for k in 0..9u32 {
        let kind = if k == 6 { 0 } else { 1 << (k % 4) };
        let node = if k == 3 || k == 7 {
            None
        } else {
            Some(Vf([pick.coord(), pick.coord(), pick.coord(), 1.0]))
        };
        ids.push(both.actor(kind, node));
    }
    for &id in &ids {
        both.add(id);
    }
    both.set_searchable(ids[5], false);
    both.agree("the world was built");

    let mut hits = 0;
    let mut touching = 0;
    for round in 0..300 {
        // One point in five sits exactly on an actor's centre, so the "too close to count" rejection is
        // part of the generated set and not only of the test that pins it down.
        let point = if pick.below(5) == 0 {
            let on = ids[pick.below(ids.len() as u64) as usize];
            match both.actors.get(on).node {
                Some(centre) => {
                    touching += 1;
                    centre
                }
                None => Vf([pick.coord(), pick.coord(), pick.coord(), 1.0]),
            }
        } else {
            Vf([pick.coord(), pick.coord(), pick.coord(), 1.0])
        };
        let exclude = if pick.below(4) == 0 {
            Some(ids[pick.below(ids.len() as u64) as usize])
        } else {
            None
        };
        let (original, ported) = both.nearest(point, exclude);
        assert_eq!(
            original, ported,
            "round {round}: nearest to {point:?} excluding {exclude:?}"
        );
        if original.0.is_some() {
            hits += 1;
        }

        // A mask of every shape, including zero (takes nothing) and bits no actor has.
        let mask = pick.below(32) as u32;
        let (original, ported) = both.nearest_of(mask, point, exclude);
        assert_eq!(
            original, ported,
            "round {round}: nearest of mask {mask:#x} to {point:?} excluding {exclude:?}"
        );
        if mask == 0 {
            assert_eq!(original.0, None, "round {round}: an empty mask takes nothing");
        }
    }
    assert!(
        hits > 200 && touching > 20,
        "the generated searches hardly exercised anything: {hits} found something, {touching} were on \
         top of an actor"
    );

    // The kind-zero actor: reachable by the unmasked search, unreachable by the masked one, whatever
    // mask it is handed.
    let zero = ids[6];
    let centre = both.actors.get(zero).node.expect("the kind-zero actor has a node");
    for mask in [0u32, 1, 0xffff_ffff] {
        let (original, ported) = both.nearest_of(mask, centre, Some(zero));
        assert_eq!(original, ported);
        assert!(
            original.0 != Some(zero),
            "a kind of zero is in no mask, not even {mask:#x}"
        );
    }
}

/// Equal distances go to whichever actor was registered first, and the answer follows the registration
/// order rather than the actors: unregister the winner and register it again, and the *other* actor
/// wins the same tie. This is the test that makes the module's claim about RNG draw order and
/// collision tie-breaks concrete.
#[test]
fn a_tie_goes_to_the_actor_registered_first() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    // Two actors the same distance from the origin, in opposite directions.
    let left = both.actor(1, Some(Vf([-3.0, 0.0, 0.0, 1.0])));
    let right = both.actor(1, Some(Vf([3.0, 0.0, 0.0, 1.0])));
    both.add(left);
    both.add(right);
    let origin = Vf([0.0, 0.0, 0.0, 1.0]);

    let (original, ported) = both.nearest(origin, None);
    assert_eq!(original, ported);
    assert_eq!(original.0, Some(left), "the first registered actor keeps the tie");
    assert_eq!(f32::from_bits(original.1), 3.0, "and the distance is the one it wrote");

    // Re-register the first: it goes to the tail, and the tie changes hands.
    both.remove(left);
    both.add(left);
    both.agree("left was re-registered");
    let (original, ported) = both.nearest(origin, None);
    assert_eq!(original, ported);
    assert_eq!(original.0, Some(right), "the tie now goes to the other actor");

    // Excluding the winner hands it to the loser, and excluding both leaves the out-parameter at the
    // value the search starts with.
    let (original, ported) = both.nearest(origin, Some(right));
    assert_eq!(original, ported);
    assert_eq!(original.0, Some(left));
    both.remove(left);
    both.remove(right);
    let (original, ported) = both.nearest(origin, None);
    assert_eq!(original, ported);
    assert_eq!(original.0, None);
    assert_eq!(
        original.1,
        f32::MAX.to_bits(),
        "an empty registry leaves the largest float the FPU holds in the out-parameter"
    );
}

/// The searches refuse a candidate that is *at* the point they were asked about: the comparison is
/// against 1e-6, and it rejects anything that is not strictly beyond it. The exact boundary is
/// whatever the FPU's square root makes of the squares, so the test asks the original where the
/// boundary is rather than asserting one — and checks the port lands in the same place every time.
#[test]
fn a_candidate_at_the_point_is_not_a_candidate() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let here = both.actor(1, Some(Vf([0.0, 0.0, 0.0, 1.0])));
    let far = both.actor(2, Some(Vf([10.0, 0.0, 0.0, 1.0])));
    both.add(here);
    both.add(far);

    for offset in [0.0f32, 1e-7, 1e-6, 1.0000001e-6, 2e-6, 1e-3, 1.0] {
        let point = Vf([offset, 0.0, 0.0, 1.0]);
        let (original, ported) = both.nearest(point, None);
        assert_eq!(original, ported, "at an offset of {offset:e}");
        if offset <= 1e-6 {
            assert_eq!(
                original.0,
                Some(far),
                "{offset:e} from the actor is too close to count, so the far one wins"
            );
        }
        if offset >= 1e-3 {
            assert_eq!(original.0, Some(here), "{offset:e} away is a normal candidate");
        }
    }

    // The out-parameter is optional: passing zero for it is what most callers do.
    both.place_point(Vf([1.0, 0.0, 0.0, 1.0]));
    let who = both.o.call(
        at("actor_manager_nearest"),
        &[MANAGER as u64, POINT as u64, 0, 0],
    ) as u32;
    assert_eq!(both.maybe_id(who), Some(here), "the search works with no out-parameter");
    assert_eq!(
        both.o.ee.u32_at(OUT_DISTANCE),
        0xcccc_cccc,
        "and writes nothing when it is not given one"
    );
}

/// The gate both searches test before they will measure a distance: a cleared word, and a null scene
/// node, each make an actor invisible to them while leaving it in every list and in both dispatch
/// passes.
#[test]
fn the_searches_skip_what_the_original_skips() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let gated = both.actor(1, Some(Vf([1.0, 0.0, 0.0, 1.0])));
    let nodeless = both.actor(1, None);
    let far = both.actor(1, Some(Vf([9.0, 0.0, 0.0, 1.0])));
    for id in [gated, nodeless, far] {
        both.add(id);
    }
    let origin = Vf([0.0, 0.0, 0.0, 1.0]);

    let (original, ported) = both.nearest(origin, None);
    assert_eq!(original, ported);
    assert_eq!(original.0, Some(gated), "with its gate set it is the nearest");

    both.set_searchable(gated, false);
    let (original, ported) = both.nearest(origin, None);
    assert_eq!(original, ported);
    assert_eq!(original.0, Some(far), "with the gate cleared it is not a candidate at all");
    let (original, ported) = both.nearest_of(1, origin, None);
    assert_eq!(original, ported);
    assert_eq!(original.0, Some(far), "and the masked search skips it on the same word");

    // Still in the lists, and still dispatched to: the gate is the searches' business alone.
    both.agree("a gate was cleared");
    both.o.visited.borrow_mut().clear();
    both.o.call(at("actor_manager_each_early"), &[MANAGER as u64]);
    assert_eq!(
        both.o.visited.borrow().len(),
        3,
        "all three actors are still visited every frame"
    );
}

// ---------------------------------------------------------------------------------------------------
// Teardown
// ---------------------------------------------------------------------------------------------------

/// The order a level's registry comes down in, address by address, against [`Registry::teardown`].
///
/// The manager's destructor releases, in this order: each group's member entries from the head down and
/// then the group object itself, group by group; then the entries of the group list; then the entries of
/// the master list; then both name registries, each as its name blocks in index order followed by its
/// nodes. The port has no heap, so its [`Released`] list is the only thing it can be compared against —
/// and it is compared against the real addresses the bump allocator handed out, in the real order the
/// destructor freed them.
///
/// The actors themselves appear nowhere in that list, which is the other half of the point: the manager
/// never owned them.
#[test]
fn the_destructor_releases_a_level_in_one_order() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.clear_name_registries();
    both.o.fresh_manager();

    // Names in both registries, as a built level has them.
    let kind_names = ["MonsterTruck", "MotocrossRider", "Prop"];
    let class_names = ["Object", "Vehicle"];
    let mut ported_kinds = NameBits::new();
    let mut ported_classes = NameBits::new();
    for (index, name) in kind_names.iter().enumerate() {
        let string = both.o.place_name(index as u32, name);
        both.o.call(at("name_bit"), &[at("actor_kind_names") as u64, string as u64]);
        ported_kinds.bit(name);
    }
    for (index, name) in class_names.iter().enumerate() {
        let string = both.o.place_name(8 + index as u32, name);
        both.o.call(at("name_bit"), &[at("actor_class_names") as u64, string as u64]);
        ported_classes.bit(name);
    }

    // Six actors over three kinds, so there are three groups of different sizes.
    let ids: Vec<ActorId> = [1u32, 2, 1, 4, 2, 1].iter().map(|&k| both.actor(k, None)).collect();
    for &id in &ids {
        both.add(id);
    }
    both.agree("the level was built");

    // What the destructor is about to free, read out of the lists before it runs.
    let master: BTreeMap<ActorId, u32> = both
        .o
        .walk(MANAGER + M_MASTER)
        .into_iter()
        .map(|(record, node)| (both.id_of(record), node))
        .collect();
    let group_entries: Vec<(u32, u32)> = both.o.walk(MANAGER + M_GROUPS);
    let mut members: BTreeMap<(u32, ActorId), u32> = BTreeMap::new();
    for (index, &(group, _)) in group_entries.iter().enumerate() {
        for (record, node) in both.o.walk(group) {
            members.insert((index as u32, both.id_of(record)), node);
        }
    }
    let mut names = Vec::new();
    for registry in [at("actor_kind_names"), at("actor_class_names")] {
        let held = both.o.walk(registry);
        names.extend(held.iter().map(|&(block, _)| block));
        names.extend(held.iter().map(|&(_, node)| node));
    }

    // What the port says the order is, translated into those addresses.
    let released = both.registry.teardown();
    let mut expected: Vec<u32> = released
        .iter()
        .map(|r| match *r {
            Released::GroupMember { group, actor } => members[&(group, actor)],
            Released::Group { group } => group_entries[group as usize].0,
            Released::GroupEntry { group } => group_entries[group as usize].1,
            Released::Entry { actor } => master[&actor],
        })
        .collect();
    // The names are the manager's too, and the port keeps them in a type of their own.
    expected.extend(names);

    both.o.heap.borrow_mut().log.clear();
    both.o.call(at("actor_manager_dtor"), &[MANAGER as u64, 0]);
    let freed = both.o.heap.borrow().freed();

    assert_eq!(
        freed.len(),
        expected.len(),
        "the destructor freed {} blocks and the port accounts for {}",
        freed.len(),
        expected.len()
    );
    assert_eq!(freed, expected, "the order a level comes down in differs");

    // Nothing of the actors, and nothing of the manager object itself (its second argument said so).
    for (index, &record) in both.address.iter().enumerate() {
        assert!(
            !freed.contains(&record),
            "the destructor freed actor {index}'s record — the manager does not own the actors"
        );
        assert!(both.actors.is_live(ActorId(index as u32)), "and neither does the port's");
    }
    assert!(!freed.contains(&MANAGER));

    // Both sides are empty afterwards, and the lists are sound rather than merely empty.
    assert_eq!(both.o.master(), Vec::<u32>::new());
    assert!(both.o.groups().is_empty());
    for list in [MANAGER + M_MASTER, MANAGER + M_GROUPS, at("actor_kind_names"), at("actor_class_names")] {
        both.o.links_are_sound(list);
        assert_eq!(both.o.count(list), 0);
    }
    assert_eq!(both.registry.count(), 0);
    assert!(both.registry.groups().is_empty());
    ported_kinds.clear();
    ported_classes.clear();
    assert_eq!(ported_kinds.count(), 0, "the port's name registries go the same way");
    assert_eq!(ported_classes.count(), 0);
}

/// Freeing an actor record does not unregister it: the destructor touches the actor's own sub-lists and
/// says nothing to the manager, which is left holding an entry pointing at freed memory. The port keeps
/// that split — [`Actors::destroy`] leaves the registry alone — so this test is what justifies the panic
/// the port's accessors raise instead.
#[test]
fn freeing_a_record_leaves_the_manager_holding_it() {
    let Some(o) = Original::new() else { return };
    let mut both = Both::new(o);
    both.o.fresh_manager();
    let one = both.actor(1, None);
    let two = both.actor(1, None);
    both.add(one);
    both.add(two);

    let record = both.address_of(one);
    both.o.heap.borrow_mut().log.clear();
    both.o.call(at("actor_record_dtor"), &[record as u64, 0]);
    both.actors.destroy(one);

    assert_eq!(
        both.o.master(),
        [record, both.address_of(two)],
        "the master list still holds the record it just lost"
    );
    assert_eq!(
        both.o.ee.u32_at(record + R_MANAGER),
        MANAGER,
        "and the record's own back-pointer still names the manager"
    );
    assert_eq!(
        both.registry.iter().collect::<Vec<_>>(),
        [one, two],
        "the port's registry is in the same state, pointing at a destroyed id"
    );
    assert!(!both.actors.is_live(one));
    assert!(both.actors.try_get(one).is_none(), "and reading it is refused rather than guessed at");
}

// ---------------------------------------------------------------------------------------------------
// The state hash
// ---------------------------------------------------------------------------------------------------

/// The hash sees everything this module calls state — and in particular it sees *order*. A registry
/// holding the same actors in a different sequence is a different state, because the whole module
/// exists to say that the sequence matters; so is a table with a hole where an actor used to be, and so
/// is a name registry holding the same number of different names.
///
/// A port-only test: there is nothing to compare against here, because the original has no hash. What
/// it guards is a later task adding a field and forgetting the walk, which a replay would then step
/// straight over.
#[test]
fn the_hash_sees_the_order_and_the_holes() {
    let mut actors = Actors::new();
    let a = actors.insert(Actor::new(Kind(1)));
    let b = actors.insert(Actor::new(Kind(1)));

    let mut first = Registry::new();
    first.add(&mut actors, a);
    first.add(&mut actors, b);
    let mut second = Registry::new();
    second.add(&mut actors, b);
    second.add(&mut actors, a);
    assert_ne!(
        game_sim::hash::of(&first),
        game_sim::hash::of(&second),
        "two registries holding the same two actors in opposite orders hash the same"
    );
    let difference = game_sim::hash::first_difference(&first, &second).expect("they differ");
    assert_eq!(difference.path(), "actor", "and the walk says which entry first differed");

    let before = game_sim::hash::of(&actors);
    actors.destroy(a);
    let after = game_sim::hash::of(&actors);
    assert_ne!(before, after, "a destroyed record is a change of state");
    assert_eq!(actors.count(), 1);
    assert_eq!(
        actors.insert(Actor::new(Kind(2))),
        ActorId(2),
        "and the hole is not handed out again"
    );

    let mut one = NameBits::new();
    let mut other = NameBits::new();
    one.bit("Vehicle");
    other.bit("Prop");
    assert_ne!(
        game_sim::hash::of(&one),
        game_sim::hash::of(&other),
        "two registries of one name each are different states — the names are what they hold"
    );

    // An actor's own fields, including the one that is only sometimes there.
    let mut actor = Actor::new(Kind(4));
    let plain = game_sim::hash::of(&actor);
    actor.node = Some(Vf([0.0, 0.0, 0.0, 0.0]));
    assert_ne!(plain, game_sim::hash::of(&actor), "a node at the origin is not no node");
}
