# C24 — the Kiowa's own rays on its thrown blades (FINAL)

2026-11-03. Task: `./kit plan C24`, C21's follow-up: the helicopter's collide pass never offered the
thrown blades' collidables (`Blades::knocks`) to its rays, and no blade's search offered the other
blades'. The task: find a strike where the helicopter's record holds a kind-0 entry on a blade (or a
blade on a blade), capture it, and port it.

## Result

**No such strike exists in eight captured wrecks, and the reason is the kept nearby list.** The port
now offers the blades exactly when the original's list holds them. In the captures that is never
while the wreck stays put, and from the next fresh gather on when it slides. The port's rays meet no
blade in any of the eight captures, and the original's records hold no kind-0 entry on a blade.

- `Collision::search_knocking` (`crates/game-sim/src/kiowa.rs`): `Collision::search` with blades
  offered as collidables after the world's (`Scene::knocking_gathered`).
  - The heli's entries on blade `k` name `k` as their other side, for `Blades::with_others`.
  - The mirrors on the blades come back by blade, for `Blades::file_struck`.
  - The scene's sweep takes the Kiowa's sweep category list (`prop_world::sweep_categories`, which
    leaves out 0x8 and 0x800), so the heli's sweep never meets a blade, as in the original.
- `Blades::gather_round`, run straight after each post's refresh. It models whether the
  helicopter's kept list holds the blades:
  - **fresh gather** (the list's centre moved): every blade out, each with its distance;
  - **re-sort** (centre unchanged): the blades it already had, each measured again;
  - distances use `flight_search::knock_distance`, clamped at 0 as the gather's are.
- `Blades::gathered` gives what to offer.
- `Blades::file_struck` puts the heli's mirrors ahead of each blade's own entries, up to its capacity.
- The shell (`KiowaRide::step`, `crates/openmx/src/vehicle.rs`) is wired the same way as the tests.

## Why the original never shows one

The collision node's update clears every record, then collides each in list order: the heli's
record, then the four blades'. The heli's rays search its **kept** nearby list. Its post's refresh
gathers that list afresh only when the probe's reach leaves the bound round where it was last
gathered:

- the test is `moved + probe radius >= bound`;
- otherwise it only re-sorts, recomputing the listed candidates' distances (`broad_phase::gather`
  and `resort`).

Read off the C14 and C21 RAM images (`c14`, `c21-5`, `c21-6`, `c21-7`):

- **The heli's search.** Centre (3034.0, 132.11, 4053.75); bound 30 in the C21 images (10 in C14);
  least 10, extra 30, capacity 128. The heli's ray radius is 19.66, so a fresh gather needs the probe
  to move more than about 10.3 ft.
- **Its list.** 9 entries (static items and toys), never a blade.
- **The blades' lists.** These hold the other blades, the heli's collidable (distance 0), and 12
  objects of category 0x100020, which the heli ray's category list `[0x100000]` drops.
- **The pair filter** lets a heli ray meet a blade's collidable:
  - the blade's collidable has category 0x18, is enabled, and has an empty mode-1 category list;
  - the heli ray has category 0x8 and the mode-1 list `[0x100000]`;
  - the exclude lists hold only each collidable's own probes and body.

So the heli's rays can only meet a blade after a fresh gather while the blade is out. A wreck that
stays where it fell never gathers again.

The blades are moving objects; the gather walks the moving structure after the static one. Their
collidables lack `WITHIN_BOUND` (0x08; their flags are 0x14), so the bound does not limit them.
That is why a fresh gather takes in every blade out.

## The captures

**How each was made.**
- Every tape starts from C14's state (`context/recordings/c24/*.p2m2`, each with C17's
  `_SaveState.p2s` beside it). Made by `research/c14/make_tape.py`; the specs are in
  `context/recordings/c24/tapes.txt`.
- Captured with `research/c19/capture.sh` and C19's spec (contacts of every record) into
  `context/capture/c24/`.
- Run on copy 3 (`KIT_PCSX2=3`). An `agent-runner` shim on `PATH` (in the scratchpad) answered
  `./kit pcsx2`'s slot check.

**Tapes.** The make_tape fields are fields (2 per tick).

| capture | tape (make_tape fields) | wreck | put-back | heli entries on a blade | replay |
|---|---|---|---|---|---|
| roll | `22 300:-:255,127 250` | 1257 | 1408 | 2 × k1 (b3) | bit-exact to 1488 |
| try1 | `30 300:-:255,0 250` | 1263 | 1414 | k1 only (b0, b3) | bit-exact to the end |
| try2 | `14 300:-:0,127 250` | 1246 | 1397 | k1 only (b1) | bit-exact to the end |
| try3 | `14 300:-:255,0 250` | 1255 | 1406 | k1 only (b2) | bit-exact to the end |
| try4 | `40 300:-:0,0 250` | 1267 | 1418 | k1 only (b1) | bit-exact to 1488 |
| try5 | `40 300:-:255,0 250` | 1268 | — | k1 only (b1, b3) | parts on 1304 (below) |
| fast1 | `60 80:cross 200:-:127,0 200:square:127,0 400` | 1451 | 1602 | k1 only (b0, b2) | bit-exact to 1672 |
| fast2 | `60 80:cross 200:-:127,0 200:square:0,0 400` | 1442 | — | none | parts on 1416, in flight (below) |

In all eight, no blade's record holds an entry on another blade.

`scan.py` (scratchpad) decodes each row's records:
- an entry's kind is word +0x00;
- its other record is word +0x74;
- the records are named off the image's heli and blade pointers.

**Which captures are tests.** `crates/game-sim/tests/kiowa_capture.rs` has three, each through
`replay_wreck`:
- `a_rolled_wreck_replays_bit_exact` (roll): never offered;
- `a_tipped_wreck_never_meets_its_blades` (try4): never offered;
- `a_fast_wreck_offers_its_blades_bit_exact` (fast1): offered on 150 frames, the nearest at 0 ft,
  none met.

`replay_wreck` compares every row: before the wreck, after its frame; after the wreck, where it was
read (`compare_torn`).

**The rng.** The original's generator draws on input, so the tapes' states part from the port's at
about 1237–1267. The replay reseeds from the capture's `rng_state` on the row before the wreck. The
throw's 12 draws are then the original's.

**Negative control.** With `gather_round` taking every frame as a fresh gather (the blades offered on
every post from the throw on, which is C24's first version):
- the tip fails on 1270: speed 3.1627517 in the game, 3.219419 in the port. The port's rays meet
  blade 1, which the original's list does not hold;
- the roll fails its "never offered" assert.

The C21 strike test (`a_captured_rotor_strike_replays_bit_exact`) now runs the same wiring and still
passes; it asserts the heli's rays meet no blade.

## The put-back steps once here

C21 found that on its strike's put-back (1449) the original steps the placed body a second time
that frame. None of these captures does.

In try4:
- the row for 1418 holds the placed body, exactly as placed (3026.0, 132.64741, 4054.0), with
  that frame's pad poll (lateral -0);
- row 1419 holds the port's one step from there (collective 0.3029253);
- with the second step, the port runs one step ahead of the game on every row from the put-back on.

The other captures that reach their put-back are the same. All 151 frames after the wreck are on
the same clock as C21's (1298 → 1449). So `replay_wreck` steps once, and the shell keeps C21's second
step. What tells the two apart is not known (follow-up filed).

## Other partings, not C24's

- **try5** parts on 1304 (speed 2.388822 in the game, 2.5328536 in the port). Its heli's ground
  contact count already differs over 1297–1302: the game has 1, 0, 2, 3, 2 against the port's
  3, 3, 3, 3, 4 (counts as captured, read at mixed phases). No blade is in its list and none is
  on another blade. A ground-contact difference (follow-up filed).
- **fast2** parts on 1416, before its wreck, 42 ticks into a full-left descent at speed: the
  lateral cyclic is -0.08864668 in the game, -0.08283156 in the port. A flight-control difference
  (follow-up filed).

## Checked

- `./kit check -- -p game-sim --test kiowa_capture`: 7 passed. The new tests ran on their captures
  (they printed their summaries), not skipped.
- `./kit check -- -p game-sim --test kiowa_capture --test kiowa --test kiowa_phy --test prop_wreck`:
  all passed.
- `./kit check -- -p openmx --test kiowa_particles --test kiowa_pilot_seat --test kiowa_switch
  --test kiowa_picture --test bays`: all passed. These do not exercise a heli-on-blade strike.

## Not verified / not 1:1

- **No kind-0 heli entry on a blade, and no blade on a blade, was ever captured.** The contacts the
  port's rays would make on a blade (`search_knocking`'s entries and `file_struck`'s mirrors) are
  untested against the original. Blade-on-blade is not ported: `Blades::search_on` still offers a
  blade none of the others. That needs a capture that shows one.
- **The fast slide's list was not read off RAM.** That the original's fresh gather took in all four
  blades is inferred from the code (the moving structure, no `WITHIN_BOUND`). The capture shows only
  that no ray met one.
- **The order of the blades among the list's moving objects is not checked.** The port puts them
  after the world's collidables. Other moving objects in the original's list would come between.
- **The put-back's second step** is taken on C21's strike and not on these eight. What decides it is
  not known; the shell still always takes it.
- The captures are in `context/` of slot s3 and are not committed. The three new tests skip without
  them.
