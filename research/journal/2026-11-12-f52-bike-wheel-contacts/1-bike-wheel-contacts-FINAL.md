# 1 — bike wheel contacts (FINAL)

Task F52. 2026-11-12.

## What I did

- `game_sim::handling::bike::wheel_contacts`: the bike's walk of its trigger probe's record, two wheels
  (front, rear), a `tell(owner, i)` per listener, the rear's node kept. Reads each ray's `z` as the FPU
  does (`F::from_bits`), so denormals and NaN patterns go the original's way (an oracle case with a
  negative NaN `z` caught the first host-compare version).
- `quad::Sweep::two_wheels` (default false). `quad::Steps` answers `Call::Contacts` with the bike's walk
  (`bike_contacts`, slots FRONT/REAR) when the sweep says so, and with the quad's four-wheel walk
  otherwise.
- `tests/mxvatv_bike_ride.rs`:
  - `TriggerInRam` is the Force body's sweep. It stands the trigger at the bike's node frame (`atv_sync::stand`,
    written back), runs F46d's `part_search_in_ram` over the bike's nearby list, then reads the record
    with `trigger_contacts`.
  - `StepsInRam` keeps `Contacts` in Rust under `in_rust("wheel contacts")`, and always in the net
    unpack.
  - `UpdateInRam` serves the net unpack's `Out::Contacts` in Rust.
- `research/oracle.txt` `# F52`.

## What the original does

`./kit fn 0x004dccd0 -p atv` (and `./kit dis`), the stand slot `./kit fn 0x001bd180`, the set-contact
`./kit fn 0x004a0c80`, the list getter `./kit fn 0x004bf960`. In words: zero the rear node, stand the
trigger probe at its node's frame, search it over the bike's nearby list, and if the hit bit is set walk
the record. For each contact, tell the struck object's owner's listeners (vt `+0xc`, args 0, 0), keep the
object's vehicle only if its kind is 2, and set the contact on the rear wheel when the ray's `z <= 0`
(the rear node becomes the object), else on the front. Details are in oracle.txt `# F52`.

## How it was proved

- Function oracle, 10 000 generated records: `the_bikes_wheel_contacts_walk_is_the_originals`. The
  original runs with its stand and search hooked to log; the port logs the same calls, then runs
  `trigger_contacts` + `bike::wheel_contacts` on the wheels in RAM and tells listeners through their own
  vtables. Calls (in order) and every byte outside the stack match.
  - Tallies: front 11190, rear 13169, told 9014, vehicle kept 5559, vehicle dropped 10578, z ±0 3527,
    z NaN/denormal 3510, no hit 1605, negative count 285.
  - Mutants killed: `>=`, host compare, no kind test, no rear node, reversed tells.
- Frame replay: `rides_replay_with_the_wheel_contacts_in_rust`. C15 ride-2 (459 rows) and flip-1 (465)
  run with the walk, stand and search in Rust, and every row matches the original's RAM. It served
  `Contacts` 1836 and 1860 times. The only pair kind is "wheel contacts pair, kind 5" (plant spheres). A
  mutant without the search, or without the stand's frame write-back, fails at row 0.
- Net unpack: `the_bikes_net_unpack...` now serves `Contacts` once and still matches.

## Not verified / not 1:1

- **The walk is not proved in frames.** Along both C15 rides the trigger only ever strikes plant
  spheres, which file no contact, so no ride exercises front/rear, owners or vehicles. Only the walk
  oracle proves those.
- **Some contacts are refused in the Steps route.** A contact with an owner, or with a kind-2 vehicle,
  is refused (test assert and debug_assert). `quad::Bike` and `steps::read` hold no wheel owner or
  vehicle (`steps::read` refuses nonzero `+0x1f0`/`+0x1f4`), and the listener tell has no Rust target
  there. `bike::wheel_contacts` itself handles both, and the walk oracle proves it.
- **Counts are read once.** The original re-reads the record count and each owner's listener count on
  every turn; the port reads them once (`trigger_contacts`). They only differ if a listener changes
  those counts mid-walk. The oracle's listeners are stubs, so this is not proved.
- **The net-unpack oracle can't tell the search apart.** State 4's RAM is already in the searched state
  there. The frame replay is what proves the search and stand.
- **The node frame is the port's scene update.** The stand's frame is the one the Steps route is handed
  (the port's scene update), not a separate call to the original's node-frame function.
- **Sources not used.** I didn't read the PC exe, and opentennis has nothing on this.

## Next

None. Follow-up F63 is filed in the plan: keep wheel owners and vehicles in the Steps route.
