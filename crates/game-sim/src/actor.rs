//! Actor lifetime: how a level's live objects are made, registered, walked and let go.
//!
//! Every object a level simulates — the riders, their machines, the one-off props — is registered with
//! one manager as it is built, and everything that happens to all of them afterwards happens in the
//! order that manager holds them in. That order is the subject of this module, because it is not
//! cosmetic: the per-frame dispatch, the two "nearest actor" searches and (through them) every
//! tie-break in the game resolve in registration order, and a port that iterates in a different order
//! draws the same random numbers in a different sequence and resolves the same collision the other way
//! round.
//!
//! # There is no actor array
//!
//! Nothing in this engine has a base, a stride and a count. The manager owns two intrusive
//! doubly-linked lists — a master list of every actor, and a list of per-kind *groups*, each of which is
//! another list of the actors of one kind — and the only fixed-size thing in the picture is the 12-byte
//! list node (`item`, `prev`, `next`), one heap allocation per entry. [`List`] is that container; the
//! port keeps its order and its operations and drops the pointers, which nothing outside the container
//! could observe.
//!
//! # A kind is an interned name, not an enum
//!
//! The word at the top of an actor record looks like a type id — a level holds actors of kind 1, 2 and 4
//! — and it is not. The record's constructor interns the actor's *type name* ("MotocrossRider" for the
//! rideable machine) in a global name list and keeps the bit of the slot it landed in: the first distinct
//! name registered is `1`, the second `2`, the third `4`. So the numbers are allocation order, they are
//! only stable for as long as the names are first seen in the same order, and nothing may hard-code
//! them — which is why [`Kind`] is a newtype over the bit rather than an enum, and why [`NameBits`] is
//! part of this module instead of a table somewhere. The second registry works the same way and holds
//! *class* names: each level of an actor's class hierarchy ORs its own bit into the record, giving the
//! "is-a" mask that [`Actor::classes`] holds.
//!
//! One bit per name means a registry saturates: the 32nd name gets `0x8000_0000` and every name after
//! that gets **zero**, because the original shifts a running `1` left once per entry examined rather
//! than shifting by an index. A zero kind is a kind that matches no group and no search mask, so this
//! is a real cliff and not a curiosity — see [`NameBits::bit`].
//!
//! # Created, registered, freed — three steps, not one
//!
//! * **Created.** The world build allocates the owning object and its constructor chain builds the
//!   actor record as a sub-object inside it. The record comes out with its kind interned, its class
//!   mask ORed together, its three per-actor sub-lists empty, its node, manager and player pointers
//!   null, and the word both searches test set to one ([`Actor::searchable`]).
//! * **Registered.** The world build then hands the record to the manager ([`Registry::add`]). Nothing
//!   in the constructor chain does this, so an actor exists unregistered for as long as the builder
//!   takes, and the order the builder registers in *is* the iteration order for the whole level.
//! * **Freed.** The record's destructor frees the actor's own sub-lists and **does not unregister it**.
//!   Whoever tears the level down either calls [`Registry::remove`] first or drops the whole manager
//!   ([`Registry::teardown`]), which releases the lists and leaves the actors to their owners — the
//!   manager never owned them. [`Actors::destroy`] keeps that split, so a port that forgets the order
//!   fails loudly instead of reading a dangling entry.
//!
//! # What the iteration order decides
//!
//! The manager offers two per-frame dispatches (both walk the master list from the head and call one
//! virtual function on every actor — those functions are the actors' own business and belong to the
//! tasks that port them), the two searches here, and a pairwise exchange between two actors that walks
//! one actor's sub-list and then the other's. The searches are where the order shows up arithmetically:
//! both keep a candidate only when it is *strictly* closer than the best so far, so equal distances go
//! to whichever actor was registered first.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{Acc, Dest, F, Vf};

use crate::hash::{State, Walk};

// ---------------------------------------------------------------------------------------------------
// The container
// ---------------------------------------------------------------------------------------------------

/// The engine's one list, as the registry uses it: entries in order, appended at the tail.
///
/// The original is an intrusive doubly-linked list — a count, a head and a tail in the owner, and a
/// heap-allocated `{item, prev, next}` node per entry — and every operation below is the original's,
/// with the links replaced by position in a vector:
///
/// * **append** makes a node, hangs it off the tail and bumps the count, so list order is the order
///   things were added and stays that way;
/// * **find** is a linear walk from the head that stops at the *first* node holding the item;
/// * **remove** finds that node, unlinks it, frees it and decrements the count — the item itself is
///   never touched;
/// * **at** walks from the head counting steps;
/// * **clear** frees every node from the head down, leaving the items alone.
///
/// The original keeps the count in a word of its own. The port derives it from the entries, which is
/// the same number on every path the original can reach: the one place the two could drift is a failed
/// allocation, and that path leaves the links and the count alike untouched (the original's append
/// returns false having changed nothing — here it cannot fail, see [`List::append`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List<T> {
    /// Head first, tail last.
    entries: Vec<T>,
}

/// Spelled out rather than derived: a derived `Default` would ask the entries to have one, and an empty
/// list of a type that has no default is still an empty list.
impl<T> Default for List<T> {
    fn default() -> List<T> {
        List::new()
    }
}

impl<T> List<T> {
    /// An empty list: the constructor zeroes the count, the head and the tail.
    pub const fn new() -> List<T> {
        List { entries: Vec::new() }
    }

    /// The count the original keeps in its own word.
    pub fn count(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Append on the tail. The original returns whether the node could be allocated, and changes
    /// nothing at all when it could not; the port's allocation cannot fail, so this is always true —
    /// kept as a `bool` because a caller that checked it is a caller that handled a full heap.
    pub fn append(&mut self, item: T) -> bool {
        self.entries.push(item);
        true
    }

    /// The first entry, or `None` for an empty list (the original's null head).
    pub fn head(&self) -> Option<&T> {
        self.entries.first()
    }

    /// The last entry — the one an append goes after.
    pub fn tail(&self) -> Option<&T> {
        self.entries.last()
    }

    /// The entry `index` steps from the head, with the original's two quirks: a negative index gives
    /// the head (the walk's loop never runs), and an index past the end gives nothing rather than
    /// saturating at the tail.
    pub fn at(&self, index: i32) -> Option<&T> {
        self.entries.get(index.max(0) as usize)
    }

    /// Head to tail: the order everything in the engine iterates in.
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.entries.iter_mut()
    }

    /// Head to tail as a slice, for a caller that wants to index or search it itself.
    pub fn as_slice(&self) -> &[T] {
        &self.entries
    }

    /// The entry `index` steps from the head, to change. Panics on an index the list does not hold,
    /// which would be a port bug rather than a state the original reaches.
    fn at_mut(&mut self, index: u32) -> &mut T {
        &mut self.entries[index as usize]
    }

    /// Free every node, head first, and leave the list empty. The items are not freed: the original
    /// writes zero into each node's item word on the way past and then releases the node alone.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<T: PartialEq> List<T> {
    /// How many steps from the head the first node holding `item` is, or `None`.
    pub fn find(&self, item: &T) -> Option<u32> {
        self.entries.iter().position(|e| e == item).map(|i| i as u32)
    }

    pub fn contains(&self, item: &T) -> bool {
        self.find(item).is_some()
    }

    /// Unlink the first node holding `item` and hand the item back, or `None` when the list does not
    /// hold it — in which case nothing changes, not even the count.
    pub fn remove(&mut self, item: &T) -> Option<T> {
        let at = self.find(item)? as usize;
        Some(self.entries.remove(at))
    }
}

// ---------------------------------------------------------------------------------------------------
// Interned names: the kind word and the class mask
// ---------------------------------------------------------------------------------------------------

/// How many bytes of a name the registry keeps and compares. Longer names are truncated to this and
/// two names agreeing in their first `NAME_BYTES` bytes are the same name.
pub const NAME_BYTES: usize = 0x80;

/// How many distinct names a registry can hand a bit out for before it starts handing out zero.
pub const NAME_SLOTS: usize = 32;

/// One of the engine's two global name registries: a list of names, and a bit per name.
///
/// There are two of them in a running game, both built as the level's manager is built and destroyed
/// with it. One interns actor *type* names and gives [`Actor::kind`]; the other interns *class* names
/// and gives the bits that [`Actor::classes`] is the OR of. Nothing else distinguishes them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameBits {
    /// In registration order: slot 0's bit is 1, slot 1's is 2, slot 2's is 4.
    names: List<[u8; NAME_BYTES]>,
}

impl NameBits {
    pub const fn new() -> NameBits {
        NameBits { names: List::new() }
    }

    /// The bit for `name`, registering it if this is the first time it has been asked for.
    ///
    /// The bit is `1 << slot`, and the original computes it by shifting a running `1` left once per
    /// name it walks past rather than by shifting an index — so slot 31 gets `0x8000_0000` and **every
    /// slot from 32 on gets zero**, the shift having walked the one off the top. A zero bit is not a
    /// kind: it matches no group's head and no search mask. The port reproduces it rather than
    /// clamping, because a port that quietly kept handing out bits would diverge from a game that had
    /// stopped.
    pub fn bit(&mut self, name: &str) -> u32 {
        let key = Self::key(name);
        let slot = match self.names.find(&key) {
            Some(slot) => slot as usize,
            None => {
                let slot = self.names.count() as usize;
                self.names.append(key);
                slot
            }
        };
        1u32.checked_shl(slot as u32).unwrap_or(0)
    }

    /// The bit a name already has, without registering it.
    pub fn peek(&self, name: &str) -> Option<u32> {
        let slot = self.names.find(&Self::key(name))?;
        Some(1u32.checked_shl(slot).unwrap_or(0))
    }

    /// How many names have been registered.
    pub fn count(&self) -> u32 {
        self.names.count()
    }

    /// Free every name and the list that held them: the manager's destructor does this to both
    /// registries, so the bits a level handed out mean nothing after it.
    pub fn clear(&mut self) {
        self.names.clear();
    }

    /// A name as the registry stores and compares it: the first [`NAME_BYTES`] bytes, zero-padded.
    /// The original copies with a bounded string copy and compares with a bounded string compare, so a
    /// name longer than that keeps no terminator and only its head is ever looked at.
    fn key(name: &str) -> [u8; NAME_BYTES] {
        let mut key = [0u8; NAME_BYTES];
        let bytes = name.as_bytes();
        let n = bytes.len().min(NAME_BYTES);
        key[..n].copy_from_slice(&bytes[..n]);
        key
    }
}

/// An actor's kind: the bit its type name was interned as.
///
/// A level's actors come out as `Kind(1)`, `Kind(2)` and `Kind(4)` because three type names are
/// registered; which name got which bit is the order the world build asked, and that is all the number
/// means. The groups key on it and [`Kinds`] masks over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Kind(pub u32);

/// A set of kinds, as the search that takes one reads it: the bits of the kinds it will consider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Kinds(pub u32);

impl Kinds {
    /// Every kind.
    pub const ALL: Kinds = Kinds(u32::MAX);

    /// Just this one.
    pub fn of(kind: Kind) -> Kinds {
        Kinds(kind.0)
    }

    /// Does the mask take this kind? A kind of zero (a name past the registry's 32nd) is in no mask at
    /// all, including [`Kinds::ALL`].
    pub fn has(self, kind: Kind) -> bool {
        self.0 & kind.0 != 0
    }
}

// ---------------------------------------------------------------------------------------------------
// The actor record
// ---------------------------------------------------------------------------------------------------

/// What the registry stores in place of the original's pointer to an actor record.
///
/// The original's entries are addresses, and an address freed is an address another actor can be built
/// at. Ids here are never reused, which is the one deliberate difference: everything the simulation can
/// observe about an entry is its order in a list and whether two entries are the same entry, and both
/// of those a fresh id keeps — while a reused one would turn the original's dangling pointer into a
/// silently valid actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(pub u32);

/// Which player an actor belongs to, as the record's own word holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u32);

/// An actor record, as far as the registry and its searches read it.
///
/// This is the head of the record and not all of it: the three per-actor sub-lists (only one of which
/// owns what it holds — the record's destructor frees that one's items and borrows the others'), the
/// rigid body and the transform all live further down it and belong to the subsystems that use them.
/// What the constructor leaves in the words this port does not model is written down in the journal
/// entry rather than guessed at here.
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    /// The interned type-name bit. The groups key on it, and the masked search tests it.
    pub kind: Kind,
    /// The OR of the class-name bits of every level of the actor's hierarchy — its "is-a" set. The
    /// constructor leaves it zero and each level ORs its own bit in, so an actor three classes deep
    /// comes out with three bits set.
    pub classes: u32,
    /// The word both searches insist on being non-zero before they will even measure the distance to an
    /// actor. The constructor sets it to one and nothing read for this task ever clears it, so what it
    /// is *for* is not known — it is modelled as the flag the searches test and no more.
    pub searchable: bool,
    /// The actor's scene node, reduced to the one thing the searches read through it: the world-space
    /// centre of the node's bounding box. `None` is the original's null node pointer, which both
    /// searches skip.
    ///
    /// The getter that computes this centre from the node's transform is named in the journal entry and
    /// belongs to the scene graph, which is not ported yet; until it is, whoever advances an actor keeps
    /// this in step with it.
    pub node: Option<Vf>,
    /// The manager back-pointer the registry writes when it takes the actor and clears when it lets it
    /// go. There is one manager in a level, so "which" is nothing to record — only "whether".
    pub registered: bool,
    /// The player this actor belongs to, null until something sets it.
    pub player: Option<PlayerId>,
}

impl Actor {
    /// A record as the constructor leaves it, for an actor of `kind`.
    ///
    /// Everything is empty or null except the searches' flag, which starts set. Note what is *not*
    /// here: the manager, which is the world build's next step ([`Registry::add`]).
    pub fn new(kind: Kind) -> Actor {
        Actor {
            kind,
            classes: 0,
            searchable: true,
            node: None,
            registered: false,
            player: None,
        }
    }
}

/// The level's actor records: the "created" and "freed" half of an actor's life.
///
/// The original has no such table — each record is a sub-object inside a bigger heap allocation the
/// world build owns — so this is the port's stand-in for the heap, and it exists for one reason: an
/// [`ActorId`] has to lead back to a record without a pointer. It does not own anything else about an
/// actor, and it is deliberately not the registry: creating an actor does not register it.
#[derive(Clone, Debug, Default)]
pub struct Actors {
    /// Indexed by id. A destroyed actor leaves its hole behind, so no id is ever handed out twice.
    records: Vec<Option<Actor>>,
}

impl Actors {
    pub const fn new() -> Actors {
        Actors { records: Vec::new() }
    }

    /// Build a record the way the constructor chain builds one: intern the type name for the kind,
    /// intern each level of the class hierarchy and OR its bit into the class mask, and leave the rest
    /// of the record empty.
    ///
    /// `hierarchy` is in the order the constructors run, outermost class last, because that is the
    /// order the bits are ORed in — which does not change the mask, but does decide which name reaches
    /// a saturated registry first.
    pub fn create(
        &mut self,
        kinds: &mut NameBits,
        classes: &mut NameBits,
        type_name: &str,
        hierarchy: &[&str],
    ) -> ActorId {
        let mut actor = Actor::new(Kind(kinds.bit(type_name)));
        for class in hierarchy {
            actor.classes |= classes.bit(class);
        }
        self.insert(actor)
    }

    /// Put a record already built into the table. The honest shape of "the world build allocated an
    /// object and its constructor filled in the record"; [`Actors::create`] is this plus the name
    /// registries.
    pub fn insert(&mut self, actor: Actor) -> ActorId {
        self.records.push(Some(actor));
        ActorId(self.records.len() as u32 - 1)
    }

    /// The record, or a panic naming the id — which is this port's version of following a freed
    /// pointer, and is always a bug in the caller rather than in the data.
    pub fn get(&self, id: ActorId) -> &Actor {
        self.try_get(id).unwrap_or_else(|| panic!("{id:?} has been destroyed, or never existed"))
    }

    pub fn get_mut(&mut self, id: ActorId) -> &mut Actor {
        self.try_get_mut(id).unwrap_or_else(|| panic!("{id:?} has been destroyed, or never existed"))
    }

    pub fn try_get(&self, id: ActorId) -> Option<&Actor> {
        self.records.get(id.0 as usize)?.as_ref()
    }

    pub fn try_get_mut(&mut self, id: ActorId) -> Option<&mut Actor> {
        self.records.get_mut(id.0 as usize)?.as_mut()
    }

    /// Is this id a live actor?
    pub fn is_live(&self, id: ActorId) -> bool {
        self.try_get(id).is_some()
    }

    /// Every live actor, in creation order. **Not** the order anything in the engine iterates in — that
    /// is [`Registry::iter`], and the two are only the same while the world build registers in the
    /// order it creates.
    pub fn ids(&self) -> impl Iterator<Item = ActorId> + '_ {
        self.records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_some())
            .map(|(i, _)| ActorId(i as u32))
    }

    /// How many live actors there are.
    pub fn count(&self) -> u32 {
        self.records.iter().filter(|r| r.is_some()).count() as u32
    }

    /// Free a record, exactly as the destructor does: the actor's own lists go, and **the registry is
    /// not told**. Unregister first ([`Registry::remove`]) or tear the registry down with it
    /// ([`Registry::teardown`]); a registry left holding a destroyed id will panic the next time
    /// anything reads that actor, where the original read freed memory.
    pub fn destroy(&mut self, id: ActorId) {
        if let Some(slot) = self.records.get_mut(id.0 as usize) {
            *slot = None;
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------------------------------

/// The actors of one kind, in registration order.
///
/// A group is a list of its own on the heap, and the group list holds pointers to them. It carries no
/// key: the kind a group is for is the kind of **whichever actor is at its head**, read fresh on every
/// lookup, which is why a group may never be left empty — see [`Registry::remove`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    members: List<ActorId>,
}

impl Group {
    /// The kind this group is for: its head member's.
    pub fn kind(&self, actors: &Actors) -> Option<Kind> {
        Some(actors.try_get(*self.members.head()?)?.kind)
    }

    pub fn members(&self) -> &List<ActorId> {
        &self.members
    }
}

/// What a search found: the actor, and the distance the original writes into its out-parameter.
///
/// `actor` is `None` when nothing matched, and `distance` is then the largest float the FPU holds —
/// the value the search starts with and stores whether or not it found anything, which a caller that
/// compares distances before checking the actor will read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    pub actor: Option<ActorId>,
    pub distance: F,
}

/// One thing a teardown released, in the order it released it.
///
/// A port with no heap has no other way to show the destructor's order, and the order is worth keeping:
/// it says what is still readable while a level is coming down. Note what is *not* in the list — the
/// actors themselves, which the manager never owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Released {
    /// A member entry of the group at this index.
    GroupMember { group: u32, actor: ActorId },
    /// The group list object at this index, once its entries are gone.
    Group { group: u32 },
    /// A node of the list of groups.
    GroupEntry { group: u32 },
    /// An entry of the master list. The actor it held is left alone.
    Entry { actor: ActorId },
}

/// The level's actor registry: the master list every iteration walks, and the per-kind groups.
///
/// One of these exists while a level does, hanging off the game-level singleton beside the same
/// container holding the players (whose registry is the same machine with a different key — the
/// differences are in the journal entry, not here).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    all: List<ActorId>,
    groups: List<Group>,
}

impl Registry {
    pub const fn new() -> Registry {
        Registry { all: List::new(), groups: List::new() }
    }

    /// Register an actor: the world build's step after the constructor.
    ///
    /// In the original's order — an actor already in the master list is ignored outright, so this is
    /// idempotent and the second registration does not file a second group entry either:
    ///
    /// 1. append to the master list, which is what fixes the actor's place in every later walk;
    /// 2. write the manager into the record;
    /// 3. look for the group whose head member is of this actor's kind and append to it;
    /// 4. failing that, make a group, put the actor in it, and append the group to the tail of the
    ///    group list — so groups are ordered by the first time each kind was seen.
    ///
    /// Returns whether the actor was taken (false when it was already registered).
    pub fn add(&mut self, actors: &mut Actors, id: ActorId) -> bool {
        if self.all.contains(&id) {
            return false;
        }
        self.all.append(id);
        actors.get_mut(id).registered = true;

        let kind = actors.get(id).kind;
        if let Some(group) = self.group_for(actors, kind) {
            self.groups.at_mut(group).members.append(id);
        } else {
            let mut group = Group::default();
            group.members.append(id);
            self.groups.append(group);
        }
        true
    }

    /// Unregister an actor: unlink it from the master list, clear the manager out of its record, and
    /// unlink it from its group. The actor is not freed, and nothing else about it is touched.
    ///
    /// An actor that is not registered is left entirely alone.
    ///
    /// **A group this empties stays in the group list, and the original cannot survive that.** The
    /// group lookup reads the kind through the group's head node, so an empty group means a read
    /// through a null pointer — on the console, through whatever the kernel left at address zero, which
    /// is not a pointer. The port skips an empty group instead of imitating the fault, which is the one
    /// place in this module where it declines to reproduce the original; the journal entry says so. A
    /// level that removes the last actor of a kind and then registers or removes another is a level the
    /// original crashes in, so the difference is unreachable from anything the game does.
    pub fn remove(&mut self, actors: &mut Actors, id: ActorId) -> bool {
        if self.all.remove(&id).is_none() {
            return false;
        }
        if let Some(actor) = actors.try_get_mut(id) {
            actor.registered = false;
        }
        if let Some(kind) = actors.try_get(id).map(|a| a.kind) {
            if let Some(group) = self.group_for(actors, kind) {
                self.groups.at_mut(group).members.remove(&id);
            }
        }
        true
    }

    /// How many actors are registered.
    pub fn count(&self) -> u32 {
        self.all.count()
    }

    /// Every registered actor, head to tail — **the** order: the per-frame dispatches, the searches
    /// below and everything downstream of them see the actors in this sequence, which is the order the
    /// world build registered them in.
    pub fn iter(&self) -> impl Iterator<Item = ActorId> + '_ {
        self.all.iter().copied()
    }

    /// The master list itself, for a caller that wants to ask it something else.
    pub fn all(&self) -> &List<ActorId> {
        &self.all
    }

    /// The per-kind groups, in the order their kinds were first registered.
    pub fn groups(&self) -> &List<Group> {
        &self.groups
    }

    /// The group holding actors of `kind`: the first group whose head member is of that kind, as the
    /// original asks it, every time, rather than caching a key.
    pub fn group_of(&self, actors: &Actors, kind: Kind) -> Option<&Group> {
        self.group_for(actors, kind).map(|i| &self.groups.as_slice()[i as usize])
    }

    /// The nearest actor to a point, excluding one.
    ///
    /// Walks the master list in order and keeps the candidate whose distance is **strictly** less than
    /// the best so far, so when two actors are the same distance away the one registered first wins. An
    /// actor is considered only when it has a scene node and its [`Actor::searchable`] word is set, and
    /// a candidate nearer than [`Registry::TOUCHING`] is rejected — the original's way of not finding
    /// the thing that is already at the point it was asked about.
    pub fn nearest(&self, actors: &Actors, at: Vf, exclude: Option<ActorId>) -> Found {
        self.search(actors, Kinds::ALL, false, at, exclude)
    }

    /// The nearest actor of one of `kinds`, excluding one. The same walk and the same tie-break, with
    /// the kind mask tested first.
    pub fn nearest_of(&self, actors: &Actors, kinds: Kinds, at: Vf, exclude: Option<ActorId>) -> Found {
        self.search(actors, kinds, true, at, exclude)
    }

    /// Nearer than this and a candidate is the thing doing the asking, not a thing to find.
    pub const TOUCHING: f32 = 1.0e-6;

    /// Both searches: one walk, one distance, one comparison. `masked` says whether the kind mask is
    /// consulted at all, because the unmasked search does not have one rather than having a full one —
    /// a distinction that matters for an actor whose kind is zero.
    fn search(&self, actors: &Actors, kinds: Kinds, masked: bool, at: Vf, exclude: Option<ActorId>) -> Found {
        let mut best = F::MAX;
        let mut found = None;
        for id in self.iter() {
            if Some(id) == exclude {
                continue;
            }
            let actor = actors.get(id);
            if masked && !kinds.has(actor.kind) {
                continue;
            }
            if !actor.searchable {
                continue;
            }
            let Some(centre) = actor.node else { continue };
            let d = distance(centre, at);
            // Strictly closer, and not already touching. Both comparisons are the original's and in its
            // order; the equal case going to the earlier actor is the whole reason iteration order is
            // worth proving.
            if d < best && d > F(Registry::TOUCHING) {
                best = d;
                found = Some(id);
            }
        }
        Found { actor: found, distance: best }
    }

    /// Release the registry the way the destructor does, and say what went in what order:
    ///
    /// 1. each group in turn — its member entries from the head down, then the group object itself;
    /// 2. then the entries of the group list;
    /// 3. then the entries of the master list.
    ///
    /// The actors are not freed here, and neither are the two name registries, which the manager
    /// destroys separately ([`NameBits::clear`]). Afterwards the registry is empty and reusable, which
    /// the original's is not — it is memory about to be released.
    pub fn teardown(&mut self) -> Vec<Released> {
        let mut released = Vec::new();
        for group in 0..self.groups.count() {
            for &actor in self.groups.as_slice()[group as usize].members.as_slice() {
                released.push(Released::GroupMember { group, actor });
            }
            released.push(Released::Group { group });
        }
        for i in 0..self.groups.count() {
            released.push(Released::GroupEntry { group: i });
        }
        for actor in self.iter() {
            released.push(Released::Entry { actor });
        }
        self.groups.clear();
        self.all.clear();
        released
    }

    /// The index of the group whose head member is of `kind`: the walk the original does on every add
    /// and every remove. An empty group matches nothing (see [`Registry::remove`]).
    fn group_for(&self, actors: &Actors, kind: Kind) -> Option<u32> {
        self.groups
            .iter()
            .position(|g| g.kind(actors) == Some(kind))
            .map(|i| i as u32)
    }
}

/// The distance between a node's centre and a point, as the searches measure it: one VU0 subtract over
/// the three position lanes, then the FPU's accumulator for the three squares and its square root.
///
/// The lane the subtract leaves alone is the node centre's `w`, which nothing reads; it is spelled out
/// here because the port's vector type carries it and the original's register did too.
pub fn distance(centre: Vf, at: Vf) -> F {
    let d = centre.merge(Dest::XYZ, centre.sub(at));
    Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z()).sqrt()
}

// ---------------------------------------------------------------------------------------------------
// State, for the hash
// ---------------------------------------------------------------------------------------------------

impl State for Actor {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u32("kind", self.kind.0);
        w.u32("classes", self.classes);
        w.bool("searchable", self.searchable);
        // A node and a node at the origin are different states, so the presence is hashed too.
        w.bool("node", self.node.is_some());
        w.vf("node.centre", self.node.unwrap_or_default());
        w.bool("registered", self.registered);
        w.u32("player", self.player.map_or(u32::MAX, |p| p.0));
    }
}

impl State for Actors {
    /// Every slot, holes included: a destroyed actor is a change of state, and a hash that skipped the
    /// hole would be the same before and after one.
    fn walk(&self, w: &mut Walk<'_>) {
        w.u64("slots", self.records.len() as u64);
        for (i, record) in self.records.iter().enumerate() {
            w.group("record", |w| {
                w.u32("id", i as u32);
                match record {
                    Some(actor) => w.field("actor", actor),
                    None => w.bool("destroyed", true),
                }
            });
        }
    }
}

impl State for Registry {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u64("count", self.all.count() as u64);
        for id in self.iter() {
            w.u32("actor", id.0);
        }
        w.u64("groups", self.groups.count() as u64);
        for group in self.groups.iter() {
            w.group("group", |w| {
                w.u64("count", group.members.count() as u64);
                for &id in group.members.as_slice() {
                    w.u32("member", id.0);
                }
            });
        }
    }
}

impl State for NameBits {
    /// The names themselves, because which name holds which slot is the whole content of a registry —
    /// two registries with the same number of names and different names in them hand out different
    /// kinds.
    fn walk(&self, w: &mut Walk<'_>) {
        w.u64("count", self.names.count() as u64);
        for name in self.names.iter() {
            w.blob("name", name);
        }
    }
}
