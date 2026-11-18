# 1 — untamed tests red with packs (FINAL)

Task K59. 2026-11-17.

## What I did

The three red tests, measured on this branch's base (9055a30f0) in slot s2, which has untamed_atv and
untamed_cars installed:

- **(1) game-sim untamed_race_quad: already green.** MD168b (b99d73efb, merged 11-17 11:02) seeds the
  Untamed quad's map-edge word from the image, so the 300 ft band no longer throws the replay's quad;
  MD198 then took hazard-time to row 254. `./kit check -- -p game-sim --test untamed_race_quad`: 4 tests
  pass, the captures (context/untamed/md147) and the pack are present, so nothing skipped. No change.
- **(2) openmx untamed_cars: the test's car had no collision record.** The installed pack's mod.json now
  lists a `col` for each car (`Veh/Col/bajaM.col`), and the shell rigs every car on its pack's `col`
  (MD149b, `with_record`). The test built its baja without it, so the two drives parted. The test now
  reads the `col` from mod.json and rigs it, as mxvatv_cars.rs's `riding` does; no `col` listed, no rig,
  as the shell.
- **(3) openmx untamed_monster: the slot's pack had no monster truck.** s2's untamed_cars was imported
  before MD161b (baja, golf cart, sandrail only), so the shell said "untamed_cars lists no monstertruck"
  and the test's first assert failed. The test already rigs MonsterTruckM.col. It now says SKIPPED when
  the pack lists no monster truck, as it does with no pack at all.
- Slot s2's untamed_cars replaced with slot s1's full one (monster truck included); s2's old one kept
  in the session scratchpad. With it both openmx tests ran and passed (3 tests, no SKIPPED in the log).
- Why packs differ by slot: openmx-mods main never got the openmx-* task branches (MD39c, MD81e/f,
  MD144, MD144b, MD161b, MD163 and more). Filed **MD244** to land them.

Nothing taken from ../opentennis (test-only fix).

## What the original does

Not touched: test fixes only.

## How it was proved

- `./kit check -- -p game-sim --test untamed_race_quad`: 4 pass, with packs and captures.
- `./kit check -- -p openmx --test untamed_cars --test untamed_monster`: 3 pass on the full
  untamed_cars pack (s1's); the baja and the monster truck meet the shell's body words bit for bit
  at step 250.
- The gate, `./kit check --all`, in `./kit done`.

## Not verified / not 1:1

- The monster-truck test's new skip (a pack with no monster truck) was not run on the old pack after the
  change; the earlier run on that pack shows the shell's "lists no monstertruck" line it guards.
- Slots other than s2 were not checked; any slot with a pre-MD161b pack now skips the monster test
  until MD244 lands the openmx-mods branches and the packs are reimported.

## Next

None; MD244 for the openmx-mods side.
