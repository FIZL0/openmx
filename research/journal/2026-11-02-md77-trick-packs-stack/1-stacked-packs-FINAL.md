# 1 — stacked trick packs, combined extras (FINAL)

Task MD77. 2026-11-02.

## The gestures: Cross and Square combine (`game_sim::trick`)

MD16 gave the two extras one group each: 7 for Cross and 8 for Square, so 8 slots. Now they combine the
way the original's three combine among themselves:

- 7 is Cross alone, 8 Square alone and 9 both. MD16's two keep their numbers.
- 10..=16 is Cross over each of seven under-patterns: the original's groups 1..=6 plus **R1 alone**.
  17..=23 is the same over Square, and 24..=30 over both.
- That makes 24 groups and **96 slots** (`EXTRA_GROUPS`, `EXTRA_SLOTS`, `LAST_GROUP = 30`), so extra
  `Trick` ids run 32..128.

R1 alone is group 0 in the original, because it is the original's modifier and never a group by
itself. Without `under_extras`, Cross + R1 would have collapsed to Cross alone, and the brief names R1
outright. While both extras are up, `group_with` is still `group`, so no original gesture changes
meaning and the original's tricks and every replay are untouched. Square + Cross needs only one of
these slots, and every pairing the brief names (Cross/Square with each other and with R1, Circle and
Triangle) is one of the 24 groups. `extra_group_name` spells a group out for the shell, e.g.
"Cross + R1".

`EXTRA_SLOTS` also sizes the per-pack animation states. `vehicle_anim::SPIN_LEFT/RIGHT` now sit at
`PACK_EXTRA + 96`, and `PACK_STATES` and `PackClips::fill` follow. These were already parametrised on
the constant.

## Stacking (`trick_pack::stacked`, `anim::PackClips::stack`)

- `trick_pack::candidates` is the old `extras` without its cap. `stacked(packs, set)` walks the packs in
  order, takes each stunt name once (case-insensitive, the first pack wins), cuts at `EXTRA_SLOTS`, and
  returns how many were cut.
- `PackClips` gains `more`, the packs after the first. A stunt's clip comes from the first pack whose
  resources hold one of its member names, the same pack whose stunt row won. Without one, it comes from
  the first pack, as before.
- The stated order is the folder-name order (`trick_packs::listed`), and that is the order the ride
  uses. The settings only say which packs are on. A room carries the host's order, and `trick_packs::on`
  stacks in the ids' order, not the guest's folder order, so both machines build the same set.
- The handling (MD40) names one pack: `handling_pack`, or the first trick pack on. Bike `tricks` lines
  (`trick_packs::lay`) come from the first pack that sets them.

## The shell

- `settings.txt`: `trick_packs = a, b`, plus `handling_pack = id` (only when named). A file with
  `tricks = mxvatv` and no list still means the first pack in the folder, as before MD77.
  `tricks = original` keeps the list so switching a pack back on restores it.
- Settings → Ride:
  - The Tricks row now reads "Trick Packs" for MX vs. ATV. With packs installed, it shows the pack's
    name, "N Packs" or "None On".
  - One On/Off row per installed pack, by its `mod.json` name. Its note is `tricks/<id> in the mods
    folder`, or the overflow warning when the pack is lit and over.
  - Handling, and "Handling From" ("First Pack On" or a pack's name).
  - Rows grey out with the mods folder off. The list is refreshed wherever the music packs' list is
    refreshed (start-up and every rescan), so a store install shows at once.
- The warning reads "N stunts of the packs on have no button: past the 96 gestures". With MX vs. ATV
  Unleashed plus Untamed, nothing is over.
- `--tricks` takes `original`, `mxvatv`, or comma-joined pack ids. `--handling-pack ID` is new.
- Online: `Room.tricks` is the comma-joined ids. `Handshake.tricks_hash` is a single pack's own hash,
  as it was before, or for several packs an FNV hash over each pack's id, a 0 and its hash. So a
  one-pack room is wire- and hash-compatible with a pre-MD77 peer.

## Checked

- `./kit check -- -p game-sim --lib --test trick --test mxvatv_ride --test vehicle_anim --test
  rider_anim --test trick_ragdoll --test ride --test event_ride -- --skip the_humans_tricks`: 144 passed.
  `the_humans_tricks` is red in this slot on C6's capture, a known issue that predates MD77.
  New: `the_extras_combine_into_groups_of_their_own` and `stacked_packs_give_each_stunt_once_and_count_the_rest`.
- `./kit check -- -p game-render --lib --test pack_clips --test trick_chain --test whip_tricks`: 156 passed.
  `trick_chain` and `whip_tricks` built their hands for groups 7 and 8 only. They now search
  `group_with` for any group.
  - whip_tricks now rides every MX vs. ATV extra, 17 instead of 8. The Ninjanac, which had no gesture
    before, comes 0.509 ft off in the whip fade against 0.432 + 0.075. The bound `OVER` went to 0.08,
    with the measurement written beside it. The other 16 new extras sit at 0.409–0.411.
- `./kit check -- -p openmx-net`: 25 passed.
- `./kit check -- -p openmx --test lobby`: 65 passed. The new test `a_room_carries_every_trick_pack_on`
  checks:
  - the host names "mxvatv,untamed" in folder order, with the handling from the first;
  - Handling From names its own pack;
  - a guest without untamed is refused with the pack list in the message;
  - the same two packs give the same hash, and a changed second pack changes it;
  - one pack names and hashes as before.
- `./kit check -- -p openmx --test menu`: 233 passed. The new test `each_trick_pack_has_a_row_and_they_stack`
  covers the rows, toggling, the saved line, keeping the last list, Handling From and the overflow
  warning. The round-trip settings test carries both new fields.
- `./kit check -- -p openmx`: 2586 passed.
- Headless ride (`--headless --frames 120 --mods <copy of mxvatv_tricks + untamed_tricks>`, the copy
  in a scratch folder):
  - With `--tricks mxvatv_tricks,untamed_tricks --handling mxvatv`, the log reads "from mxvatv_tricks
    then untamed_tricks", "19 extra stunts, each on a gesture", the handling from mxvatv_tricks, and
    clips from both folders. The rider has all 21 (19 + the two 360s).
  - **Each pack alone also gives 19.** Untamed's stunt names are all ones MX vs. ATV Unleashed already
    has, so stacking the two real packs adds no stunt: the first pack's wins every name. More slots pay
    off only with a pack that has stunts of its own.

## Not verified / not 1:1

- **Not ridden online between two machines.** Only the handshake and room naming are tested, in-process.
- **A pack ride's state numbers moved.** The 360s are `PACK_EXTRA + 96` where they were `+ 8`, so a ride
  on a pack hashes differently from one before MD77. Rides with no pack (every replay, every acceptance
  ride) are unchanged, because `group_with` with both extras up is `group`.
- **The stacking order is folder order, not something the player sets.** The brief says "by a stated
  order", and the stated order is the alphabetical folder order. No row reorders packs.
- **Which clip a stacked stunt plays** is the first pack holding one of its member names. Two packs with
  the same member file name for different stunts would play the first's. Neither MX vs. ATV pack does
  that, but nothing checks it.
- **Gestures past 8 are a reach on the stock pad.** Cross + Square + Circle + Triangle + R1 + d-pad is
  six inputs at once. The keyboard's extras were not changed, so the combinations are only as reachable
  as the keyboard's keys allow.
- **The 96-slot cap is the port's own.** The original MX vs. ATV games use their own gesture schemes,
  and this is not one of them.
- The overflow warning was checked with synthetic packs (120 stunts), not with a real pack set: the two
  real packs together come to 19, far under 96.
- The stacked ride was only loaded and run headless. No gesture past group 8 was played by hand on a pad.
