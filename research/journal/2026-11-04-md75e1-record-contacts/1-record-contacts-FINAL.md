# 1 — record contacts (FINAL)

Task MD75e1. 2026-11-04.

## What I did

- Ported the quad's collision record's resolution of its contacts. This is the record's vt `+0x88`, which
  the vehicle gravity calls with 1 (`Call::GravityRecord`). The port is `game_sim::atv_contact`
  (`resolve`, `Record`, `Entry`, `Weighed`, and `Held` for the quad).
- It reuses MX Unleashed's `contact` steps. In `contact.rs`, four of them are made `pub(crate)`, and
  `impulse_on` is now a wrapper over the new `impulse_masked`. Nothing else there changed.
- `quad::Bike` gained `contacts: Option<atv_contact::Held>`:
  - With `None`, `Steps` hands `GravityRecord` on as before. Every existing test builds the bike that
    way.
  - With `Some`, `Steps::body_in` takes the frame's whole rigid body, and the gravity's call resolves the
    contacts on it (the listener's view in, and back out). `body_out` puts back the three words of the
    body the view lacks: the damping, the angular damping and the twist.
- Oracle test: `tests/atv_contact.rs`. Second whole frame: `tests/atv_quad/frame.rs`
  `the_spawned_quads_frame_touching_something_is_the_originals`.
- Names: `research/oracle.txt` under "MD75e1".

## What the original does

`./kit fn -o atv 0x00205730` walks `+0xf0` entries of `0x90` bytes from `+0xf8`. Each entry goes through
the same five steps as MX Unleashed's `collision_resolve` (`0x00270dc0`):

1. the relative velocity (`0x002058d0`);
2. below a normal speed of −10, the impulse for the record's own contacts if the second argument is set
   (`0x00206320`); otherwise the spring (`0x00205f00`);
3. the friction (`0x00206c70`);
4. with `+0x44` set and the record on a body, the damping (`0x00206e60`);
5. the pushes all go through `0x00207240`.

I compared the six functions with their MXU twins opcode for opcode (`./kit dis` of both, paired by a
scratch script). Apart from the field offsets, two things differ:

- **The impulse's own turn is masked.** When the record is on a body, `cross(arm, j)` is multiplied lane
  by lane by the record's quadword `+0x150`, then by the inverse inertia. The chassis branch and the
  other side's turn are not masked.
- **Each entry keeps its relative velocity.** The relative velocity step also stores it at entry `+0x70`,
  as well as at the record's `+0x1f0`.

In the damping, only one branch differs: a `c.eq.s` against 0 and 1 that cannot be taken, so the
damping's results are the same.

The record layout is in `oracle.txt`'s `#` lines:

- `+0xe0` chassis, `+0xe4` body, `+0xe8` node;
- the tuning from `+0x44` to `+0x1a8`;
- `+0x1e8` normal speed, `+0x1f0` relative, `+0x208` collidable.

The ATV body's virtuals that the resolution calls are these vtable `0x3cd200` slots:

| Slot | Function | What it does |
|---|---|---|
| `+0x4c` | `0x49c540` | velocity and correction `+= dv` |
| `+0x8c` | `0x49c200` | rates and twist `+= dw` |
| `+0x78` | `0x497530` | moment `+= m` |
| `+0x80` | → `0x494000` | force `+= f` |
| `+0x20` | `0x493b20` | sets damping whole |
| `+0x68` | `0x493b00` | sets angular damping whole |

The add calls also tell the body's listeners at `+0xe8`, and the quad's body has none.

## How it was proved

- **Function oracle, 2 × 10,000 generated cases** (`tests/atv_contact.rs`). The original's `0x00205730`
  runs on `atv-r-spawn.ee.bin`'s own quad record (`0x016286d0`), on the quad's own body. In the second
  test the record is on a generated chassis, with the body pointer null.
  - Each case generates the record's tuning, impulse mask included. It also generates 0–4 contacts and
    two other records, each on a body or a chassis, with collidables or without, plus a mover.
  - Only the mover's two virtuals are hooked. The bodies' virtuals run as the original's.
  - Compared bit for bit:
    - every body word of `seed::body_words`, plus the mass and the centre of mass;
    - the record's centre in the world, normal speed and relative velocity;
    - each entry's two arms and relative velocity;
    - the others' centres;
    - the chassis' accumulators.
  - Every word the original wrote outside its own stack frame is one of those.
- **Paths reached.** In the body test, out of 16,998 contacts: 2,063 hard, 1,295 impulses (459 with a
  mask that is not all ones), 2,915 springs, 1,487 mutual, 1,663 actor, 294 impulses on another chassis,
  3,795 with a mover, 6,758 damped cases. The chassis test's counts are similar.
- **Mutants.** Dropping the mask goes red at case 18 (body `+0x1d4`). Not keeping the entry's relative
  velocity goes red at case 0.
- **Whole frame, touching** (`frame.rs`). Two contacts are laid into the image's record on both sides:
  - one on the left rear tyre's side, against a rock;
  - one on the nose, against a wall ahead.

  Then SLUS-21104's world step runs at 1/30 (four substeps, so four resolutions) with only the ground
  query hooked, against `quad::frame`, with `Steps` holding the record. Everything MD75e compares is
  equal, and so are these:
  - all 2,923 poked words;
  - the whole rigid body;
  - the record's centre, normal speed (0.2468, so the spring path) and relative velocity;
  - each entry's arms and relative velocity.

  Every changed word in the compared blocks is read. `rest` is just the two `PartMove`s. MD75e's
  contact-free frame still passes unchanged, with four `GravityRecord`s handed on.

## Not verified / not 1:1

- **The touching frame's contacts were laid by hand, not captured.** No image has the quad really
  touching something. The two entries (kind 0, scenery, surfaces 3 and 7) are made up around the body's
  centre on `atv-r-spawn`. What the frame does with any list is proved, but a list the game's own collide
  would build is not. Capturing one needs a PCSX2 drive into an obstacle. That is follow-up MD98.
- **The record's collide (vt `+0x7c`) is not ported**, so `Held`'s list is not refilled each frame. In a
  running port it is only ever what was laid in. Also MD98.
- **Contacts with other records, in the quad's frame.** `atv_contact::resolve` handles them, and the
  oracle proves it, but `Held` holds no other records. With such an entry, `Held::resolve` does nothing
  and the call is handed on. MD98.
- **The body's listeners** (`+0xe8`, which the add-velocity and add-rates virtuals walk) are not ported.
  The quad's body has none, and the oracle asserts that.
- **The record's damping in the whole frame.** The image's record has `+0x44` = 0, so the frame never
  damps. The damping is proved only by the function oracle.
- **The hard (impulse) path in the whole frame.** The spawned quad is at rest, so its normal speed is
  0.25 and only the spring path runs. The impulse, and its mask, are proved only by the oracle.

- **Landed with `--skip-audit`.** The audit reds on four leaks that were in HEAD before this task, in
  files it does not touch: `atv_wheel.rs:62`/`66` (`0x397000`, `0x396fc0`) and `plan/DONE.md:519`/`533`.
  They became leaks when those addresses were named in `oracle.txt`. None of the leaks are this task's.

## Next

Done. MD98 picks up the collide and a captured image of the quad touching something.
