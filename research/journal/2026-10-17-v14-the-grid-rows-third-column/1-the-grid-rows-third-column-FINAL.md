# 1 — the grid rows third column (FINAL)

Task V14. 2026-10-17.

The `turn_still` row's back column. V12 found the port posing something 0.61 of rotation away from
the original on 26 ticks and proved it was not the weight, not the pairing and not any of the 63
clips the rider's `.mcf` loads. It is the loader: on this disc the player's rider `.mcf` aliases
`turn_still_Bck` to `turn_still_Mid.MOT`, but the original's animation set is **one global cache
keyed by clip name**, and with AI riders on the track the AI's `.mcf` has already claimed that name
for `turn_still_Bck.MOT`. The player's alias is then never read. Three clips move; all 26 ticks pose
to 1e-4 and the test's skip is gone.

## What I did

- `crates/game-render/src/anim.rs`, all additive:
  - `Set::claimed: Vec<Motion>` — what each clip *name* means, taken from **every** `.mcf` in the
    archives handed to the set that names the same `ContentDirectory`, in the archives' own member
    order, first claim of a name kept.
  - `Set::claims` builds it (and carries the three-row table below in its doc comment);
    `Set::claim` reads it, falling back to the set's own `motions` so hand-built fixtures with no
    archive still resolve.
  - `Set::member`'s first rule (name → `VUE=`) and the loop flag in `impl Animations for Set`'s
    `load` both go through `claim` now instead of this set's own `.mcf`. The loop flag follows the
    claimant because the original writes it when it *creates* the clip and never again.
- `crates/game-sim/tests/motion.rs`: deleted the skip in
  `the_ports_machine_steps_to_the_originals_clip_phase_and_pose` — the shape is posed and compared
  like every other tick now (1251 posed ticks, the 26 among them), and `not_posed` has one entry
  left (the 248
  cross-fade ticks, whose snapshot buffer the capture does not carry; that is V15's line). The `gap`
  counter stayed, with its sense inverted: it now asserts the recording still *reaches* 26 ticks of
  that shape, because a capture that stopped reaching them would make this fix look proved when it
  had only stopped being exercised.
- Nothing in `crates/game-sim/src/vehicle_anim.rs`. The brief put it on the line, but the clip table
  and the grid are right — V12 pinned the state, the bands, the pair and the weight to 1e-6. The bug
  was one name resolving to the wrong file.

## What the original does

One animation set for the whole game: the word at `anim_set` **0x00479978**, and every lookup goes
through it. Clips in it are keyed by name, `stricmp`, in `anim_clip_by_name` **0x001426a0**.

Both routes that put a clip into it check that first and hand back what they find:

- the `.mcf` `[MOTION n]` loader, `./kit fn 0x00142220`: read `Name=` (default `NONE`, strings at
  0x00426ed0), call `anim_clip_by_name` — and **return that clip untouched if it exists**. Only when
  it does not are `VUE=` (0x00426ee0), `Looping` (0x00426ef0) and `TransitionTime` (0x00426f00) read
  at all; the file is `<ContentDirectory><VUE>` with its extension forced to `.MOT` (0x00426ef8).
  So the first `.mcf` to claim a name fixes both the file and the loop bit for every motion object
  built after it, and a later `.mcf`'s `VUE=` for that name is dead text.
- load-on-demand, `anim_find_or_load` **0x00142430** (what the 36 bare pose names of
  `vehicle_load_clips` 0x002483a0 / `rider_load_clips` 0x00258790 fall through to): the same
  name-first check, then `<ContentDirectory><name>` with the extension forced to `.MOT`. It never
  reads `VUE=`, so a clip reached this way ignores aliases entirely.

Every rider archive on the disc ships two sets — `airider.mcf` for the AI bodies, `mrider.mcf` for
the player's — 40 motions each, and they disagree about exactly three:

| name | `airider.mcf` | `mrider.mcf` |
| --- | --- | --- |
| `turn_stop_Fwd` | `turn_stop_Mid.MOT` | `turn_stop_Fwd.MOT` |
| `turn_stop_Bck` | `turn_stop_Mid.MOT` | `turn_stop_Bck.MOT` |
| `turn_still_Bck` | `turn_still_Bck.MOT` | `turn_still_Mid.MOT` (its own line left in, commented) |

Note that the disagreement runs both ways: the AI set aliases two of the `turn_stop` columns the
player's set spells out, and spells out the `turn_still` back column the player's set aliases. That
is what makes the capture able to tell which column won.

## How it was proved

**The capture.** V12's recording of the original posing the player's rider,
`context/capture/v12/rider-joints.csv` (1500 rows, `frame_counter` 2776..4275, 30 Hz), stepped one
tick at a time with `Driver`/`Params`/`Request`/`Player` reseeded from the previous row, in
`the_ports_machine_steps_to_the_originals_clip_phase_and_pose`. With the fix in and the skip out:
1499 ticks, every state, ramp, axis and clip time within tolerance; **1251 ticks posed and compared
joint by joint at 1e-4**, worst joint 2.16e-5 (`UprLegR`, tick 4107, state 1, mode 1 — a pre-existing
arithmetic residual, not this row); 26 of those ticks are the shape V12 skipped. Per arm, worst
rotation over 22 joints:

| state | band region | mode | ticks | worst rotation |
| --- | --- | --- | --- | --- |
| 30 `TURN_RIDING` | 0 | 4 | 67 | 6.6e-7 |
| 30 | 1 | 2 | 57 | 7.2e-7 |
| 30 | 2 | 4 | 41 | 7.7e-7 |
| 30 | 3 | 2 | 657 | 7.2e-7 |
| 35 | 0..3 | 2 | 283 | 7.2e-7 |
| 0, 1 | 0..3 | 1 | 146 | 2.2e-5 |

**Which claims win, by data.** Two variants, both run against the capture with the skip disabled:

- *A* — the three names resolved exactly as `airider.mcf` spells them: all 12 `game-sim --test=motion`
  tests pass, every arm as above.
- *B* — only `turn_still_Bck` moved (the one name the brief names), the two `turn_stop` columns left
  to the player's set: state 30, band region 0, mode 4 goes to **3.6e-4**, over the 1e-4 tolerance.

So three independent clips agree with the `airider.mcf` column and none with `mrider.mcf`. That is
the claim *order* established from measurement, not from reading a spawn order.

**That the watched actor really is the player's rider.** `watch/rider-joints.toml` reads the actor's
control source (`+0x20 → +0x34`: 1 = the local human, 2 = an AI) and the test asserts it is 1 on
every row. The skeletons could not have told them apart: `mrider.psg` and `airider.psg` have
bit-identical bind transforms on all 22 bones (checked under a scratch test, since deleted).

**Scope, over all 219 archives.** Every bike archive holds exactly one `bike.mcf`, so the rule is a
no-op for `Chassis::load`; all 81 `airider.mcf` agree with each other and all 81 `mrider.mcf` agree
with each other on these three rows; the freestyle `ff_*` sets agree with one another on the six grid
names. The two rider `.mcf`s differ in nothing else but `SLTFile=` and those three `VUE=` lines.

Gate: `./kit check` — 507 tests.

## Not verified / not 1:1

- **The claim order comes from archive member order, not from the original's object-creation order.**
  The port walks the `.mcf`s of the archives it is handed in member order and `airider` sorts before
  `mrider` in all 162 rider archives, which reproduces what the capture shows. I did not read the
  code that decides which body is built first, so "the AI set is always built before the player's" is
  an inference from one recording plus three clips agreeing, not from the spawn path.
- **A session with no AI riders would differ.** In the original, nothing would have claimed
  `turn_still_Bck`, so the player's own alias would win and the back column *would* be the middle
  clip. The port now always resolves it the AI way, because it reads claims from the archive and not
  from who is on the track. Likewise a custom rider archive shipping only `mrider.mcf` resolves
  differently in the original than in the port. Unmeasured: V12's capture is a track with NPCs, and
  that is the only recording there is.
- **One recording, one rider, one track.** Free09, 125cc, male rider, 1500 ticks; the AI set's own
  rider was never posed against the original, nor the `ff_*` (freestyle) sets, nor a female rider.
  The `.mcf` survey says their text agrees, which is not the same as having posed them.
- **Load-on-demand's claims are not modelled.** The original's set also gains clips from
  `anim_find_or_load` (name → `<dir><name>.MOT`), which can claim a name *before* a later `.mcf`
  that aliases it. The port does not model that ordering; for the 40 listed names of this disc's
  rider sets it makes no difference, because the `.mcf` walk runs at load.
- **248 of the 1499 ticks are still not posed**: the cross-fade's source is a pose snapshot the
  capture does not carry. Unchanged by this task, and the reason is counted in the test output.
- **The worst joint is 2.2e-5, not zero**, in states 0/1 under blend mode 1 — an arithmetic residual
  that predates V14 (rendering only has to look right, and 1e-4 is the test's line).
- **The renderer still does not pair these clips on screen** — the port's own draw path is V13's
  line. What V14 proves is the clip each name loads and the pose the blend produces, in `game-sim`.

## Next

Nothing outstanding for this entry. The one visible gap left in the capture test is the 248
cross-fade ticks, which want the pose-snapshot columns a wider capture would carry.
