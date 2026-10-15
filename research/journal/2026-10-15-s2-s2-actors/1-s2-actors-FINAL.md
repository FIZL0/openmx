# 1 — s2 actors (FINAL)

Task S2. 2026-10-15.

## What I did

Ported actor lifetime — creation, registration, iteration, removal and teardown — into
`crates/game-sim/src/actor.rs` (new), and proved it against the original with sixteen tests in
`crates/game-sim/tests/actor.rs` (new). R6 had found the manager and the chain to the Nth actor by
watching RAM; this task read the code instead, which corrected two of R6's guesses (see below) and
answered the question R6 left open about what the group list is keyed on.

The port adds one module and one test file, plus two instructions to the interpreter
(`crates/ps2-oracle/src/ee.rs`: `psubb` and `psubw`, which the SDK's bounded string copy and compare
use sixteen bytes at a time — without them nothing that interns a name can run) with a test of their
own in `crates/ps2-oracle/tests/interpret.rs`. New names and addresses are in `research/oracle.txt`
under an `S2` section; nothing in the port carries an address.

## What the original does

### The container

One intrusive doubly-linked list, instantiated per use. The owner holds `{count, head, tail}`; a node is
`{item, prev, next}`, twelve bytes, one `operator new` per entry. `./kit fn 0x0021afc0` (constructor,
zeroes the three words), `0x0021b120` (append: allocate, link on the tail, `count += 1`; returns 0 and
changes nothing if the allocation fails), `0x0021afe0` (find: linear walk from the head, first match),
`0x0021b020` (remove: find, then `0x0021b080` to mend the four links, free the node and `count -= 1`),
`0x0021af60` (count), `0x0021af70` (index: walk from the head; an index ≤ 0 gives the head and an index
past the end gives 0, not the tail), `0x0021b1d0` (clear: zero each node's item word, free head first),
`0x0021b230` (destructor: clear, and free the list object itself when the second argument is > 0 — how a
group object goes). The same template appears again for the group list (`0x0021af40`, `0x0021b2c0`,
`0x0021b370`, `0x0021b3d0`) and again for the name registries (`0x001e35e0`, `0x001e3570`): R6's note
calling `0x0021b2c0` "the second half" of append is wrong, they are two instantiations.

A list is happy to hold the same item twice; `find` and `remove` then both mean the first.

### A kind is an interned name

`./kit fn 0x00281cb0` is the whole of the type system: walk a global list of names comparing the first
0x80 bytes (`strncmp`), shifting a running `1` left once per entry walked past; on a miss, allocate 0x80
bytes, `strncpy` the name in, append, and return the shifted value. So the bit is `1 << slot`, the 32nd
name gets `0x8000_0000`, and **every name from the 33rd on gets zero** — the shift walks the one off the
top, and nothing clamps. A zero kind matches no group's head and no search mask.

Two registries, both `{count, head, tail}` globals: `0x004e1e88` holds actor *type* names (an actor's
kind) and `0x004e1e68` holds *class* names (the "is-a" mask).

### Created

`./kit fn 0x00245120` is the record's constructor, taking `(record, type_name, arg3)`. It installs its
vtable at `+0x48`, constructs three empty sub-lists (`+0x24`, `+0x30`, `+0x3c`), and then:
`+0x00 = name_bit(type registry, type_name)`, `+0x04 = 0` (the class mask), `+0x08 = 0`,
`+0x0c = arg3`, `+0x10 = 1`, `+0x14 = 1`, `+0x18 = 0` (scene node), `+0x1c = 0` (manager),
`+0x20 = 0` (player). `+0x10` is the word both searches gate on.

The class mask is filled in by one descriptor object per level of the hierarchy: `./kit fn 0x002450c0`
takes `(descriptor, class_name, &record->classes, arg4)`, keeps `arg4` and the class-name bit in itself,
and ORs that bit into the record. An actor three classes deep comes out with three bits; levels shared
between two actors share a bit. **R6 read this function as the record's constructor and read the bit it
ORs as a second type id; it is neither.**

Nothing in the constructor chain registers the actor. R6's guess that construction and registration were
one step is wrong, and the distinction is the reason iteration order is the world build's property.

### Registered

`./kit fn 0x0021a8d0` — the manager's `Add`, vtable slot `+0x74` of `0x00452ae0`. `find` the actor in the
master list (`manager+0x30`) and **return immediately if it is there**, so a second registration is a
no-op; otherwise append to the master list, store the manager into `actor+0x1c`, then walk the group list
(`manager+0x3c`) comparing each group's **head member's kind** with the actor's and append to the first
match; failing that, `operator new(0xc)` a group, construct it, put the actor in it, and append the group
to the tail of the group list. So groups carry no key of their own — the kind of a group is read fresh
out of its head member every time — and groups are ordered by the first time each kind was registered.

`./kit fn 0x0021a9d0` — `Remove`, slot `+0x78`: find in the master list and return if absent; otherwise
unlink there, clear `actor+0x1c`, then find the group by head kind and unlink the actor from it. The
actor is not freed and nothing else about it is touched. **An emptied group stays in the group list.**

### Iterated

The master list, head to tail, is the order everything sees: two per-frame dispatch passes
(`0x0021aa80` calling vtable `+0x28` on every actor, `0x0021aef0` calling `+0x58`) and the two searches.
`0x0021ad50` is not a list walk — it is a two-actor exchange that walks one actor's sub-list offering
each entry to the other, then the same the other way round.

`./kit fn 0x0021aad0` (nearest) and `0x0021ac00` (nearest of kinds, which reads its mask through a
pointer, once per actor) are one walk each: skip an actor whose scene node (`+0x18`) is null or whose
gate (`+0x10`) is clear — the masked one tests `kind & *mask` first — take the node's world bounding-box
centre from `0x001b4560`, and measure `vsub.xyz`, `mula.s`/`madda.s`/`madd.s` over x, y, z in that
order, `sqrt.s`. Keep the candidate when `c.olt.s` says it is **strictly** nearer than the best so far
and `c.ole.s` says it is not within 1e-6 (`0x358637bd`) — so **equal distances go to the actor
registered first**, which is the whole reason this module exists. The out-parameter (optional; callers
pass 0) gets `FLT_MAX` (`0x7f7fffff`) when nothing matched, because that is what the search starts with.

### Freed

`./kit fn 0x002451c0`, the record's destructor, frees the items of the sub-list at `+0x24` and then all
three sub-lists — and says nothing to the manager, which is left holding an entry into freed memory.

`./kit fn 0x0021a7d0`, the manager's destructor, releases in exactly this order: for each group in index
order, its member entries from the head down and then the group object; then the entries of the group
list; then the entries of the master list; then each name registry (`0x004e1e88` before `0x004e1e68`) as
its name blocks in index order followed by its nodes (`0x00281df0`). The actors are never freed: the
manager does not own them. The player manager is the same machine with the key at `player+0x34` and the
back-pointer at `player+0x58` (`0x002459b0`), and it *does* own its players (`0x00245d00`).

## How it was proved

`crates/game-sim/tests/actor.rs`, sixteen tests, all against the original in `ps2-oracle`. Fifteen of them
need the disc and say `SKIPPED` without it rather than passing vacuously; the run that landed this task
printed no such line, so all fifteen really called the game. The harness hooks four functions
and no more: `operator new`, `operator new[]`, `operator delete` and `operator delete[]` become a bump
allocator **that logs every allocation and free in order**, and the scene-graph call that turns a node
into a bounding-box centre becomes a closure returning a centre the test chose. Everything else — the
container, `Add`, `Remove`, both searches, both dispatch passes, the record constructor, the name
interner, both destructors — runs as shipped.

- **400 generated container operations** (append, append a duplicate, remove present, remove absent,
  find, index, count, clear) against the master list's own instantiation, comparing after each one: the
  items head to tail, the count word, both directions of the chain, and the operation's return value.
  Also that an append is exactly one twelve-byte allocation that becomes the new tail, and that a failed
  remove frees nothing.
- **300 generated `Add`/`Remove` operations** over twelve actors of six kinds (including kind zero),
  comparing the master list's order and count, the group list's order, every group's membership, the
  soundness of all those chains, and every actor's `+0x1c` back-pointer after each step. The generated
  sequence never removes the last actor of a kind — see the deviation below — and includes
  double-registrations and removals of unregistered actors as no-ops.
- **Both dispatch passes**, over 24 rounds of registering, re-registering and unregistering: the actors
  they visit, in order, are the port's `Registry::iter`.
- **The emptied group**, in its own test: the original reads the group's kind through the null head, so
  the test plants a word at address zero and watches the original file the next actor into the empty
  group. The port skips it instead; the master list still agrees.
- **45 name-interning calls** over 37 distinct names (four past the cliff, one exactly 0x80 bytes, one
  that agrees with it for 0x80 bytes and differs after, and repeats): every bit, the number of names,
  the 0x80 bytes actually stored, and that a name already registered allocates nothing.
- **The record constructor**, four records over three overlapping hierarchies: every field, including
  the two words the port does not model, and that the class mask is exactly the OR of the descriptors'
  bits.
- **600 generated searches** (300 points through each search) over nine actors on an integer grid — ties
  happen, one point in five sits exactly on an actor, one actor has no node, one has its gate cleared,
  one is of kind zero — comparing the actor found *and* the bits of the distance written. Plus dedicated
  tests for the tie going to the first registered actor (and changing hands when it re-registers), the
  1e-6 rejection across seven offsets, the two gates, and the optional out-parameter.
- **The teardown order**, address by address: the port's `Released` list, translated through the
  addresses the bump allocator handed out, equals the exact sequence of frees the destructor performed —
  and no actor record appears in it.
- **The record destructor**, which leaves the manager holding a freed entry; the port reproduces the
  state and refuses the read.
- One port-only test for the state hash (order, holes and names all change it).

No emulator run and no capture: nothing in this module needs a running game, and `./kit check` is the
whole gate. `./kit pcsx2` was never started, so there is nothing to stop.

## Not verified / not 1:1

- **An emptied group.** The original leaves a group with a null head in the group list and the next
  lookup reads the kind through it — `*(int*)**(group+4)`, a read of the word at address zero used as a
  pointer. On the console that word is the kernel's and the dereference faults. The port skips an empty
  group. The difference is unreachable from anything the game does (reaching it means a kind went from
  one registered actor to none and then something else was registered or removed), and
  `an_emptied_group_is_where_the_port_stops_following` measures both behaviours rather than assuming
  them — but it *is* a deviation, and it is the only one in the module.
- **`ActorId`s are never reused**, where the original's entries are heap addresses and a freed address
  can be built at again. Nothing the simulation observes distinguishes the two (order in a list, and
  whether two entries are the same entry), and a reused id would turn the original's dangling entry into
  a silently valid actor.
- **`Actor::node` is a centre, not a node.** The scene node is reduced to the one thing the searches read
  through it, because the getter is scene-graph work (transform × bounding box, with a lazy refresh
  through the node's own vtable) and belongs to the task that ports the scene graph. The test hooks it.
  Until it is ported, whoever moves an actor must keep this field in step.
- **`+0x08`, `+0x0c` and `+0x14` of the record are not modelled.** The constructor's values for them are
  asserted in the test (0, its third argument, and 1), so they are measured rather than guessed, but
  nothing in this task reads them and the port has no field for them. `+0x14` in particular is a second
  flag word beside the searches' one, and what it is for is unknown.
- **The two dispatch callbacks and the two-actor exchange are not ported**, only their walk order. What
  vtable `+0x28`, `+0x58`, `+0x2c`, `+0x30`, `+0x34`, `+0x40` and `+0x44` of an actor do belongs to the
  tasks that port those actors.
- **`List::append` cannot fail.** The original returns false and changes nothing when the twelve-byte
  node cannot be allocated; the port's allocation cannot fail, so it always returns true. Every caller
  in this module ignores the result.
- **The derived count.** The original keeps the count in a word; the port derives it from the entries.
  They agree on every path the original can reach, the failed allocation included (which changes neither).
- **The player manager is documented, not ported.** It is the same machine keyed on `player+0x34` with
  its back-pointer at `+0x58`, it also links the player into a scene tree, and its teardown destroys the
  players — unlike the actor manager, which never owns an actor. Whoever ports players should read
  `0x002459b0` and `0x00245d00` rather than assume this module covers them.
- **No test connects the order to a random draw.** That iteration order decides RNG draw order and
  collision tie-breaks is the task's premise and is how the module is documented; what this task proves
  is the order, and that an equal-distance search resolves by it. The first subsystem that draws a
  random number per actor is where the rest of that claim gets tested.
- **Registration order itself is not proved here.** That the iteration order is registration order is
  proved; *what* the world build registers, and in what order, is the world build's, and no test in this
  task builds a real level. The first task that loads a track should compare the master list against a
  live capture.
- **The record's three sub-lists are described from their constructor and destructor only.** What the
  module's docs say each one is *for* (one cleared at the top of a dispatch pass, one walked to notify
  whatever is attached to the actor) comes from reading their call sites, not from a test; the one thing
  proved here is that the list at `+0x24` owns its items, because the record's destructor frees them.
- **Two interpreter instructions were added** (`psubb`, `psubw`) to make the string functions run. They
  are tested directly, but they are my implementation of the manual rather than something compared
  against hardware.

## Next

Nothing in S2 is outstanding; the entry can be renamed `-FINAL`. For whoever comes next: the actor
record's remaining words (`+0x08`, `+0x14`) and the three sub-lists at `+0x24`/`+0x30`/`+0x3c` are the
obvious next read — the one at `+0x24` owns its items, which makes it the only one of the three that is
anything more than a list of borrowed pointers.
