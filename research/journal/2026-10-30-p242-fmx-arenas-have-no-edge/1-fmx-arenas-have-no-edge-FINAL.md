# 1 — fmx arenas have no edge (FINAL)

Task P242. 2026-10-30.

## What I did

- `track_mods::in_stadium(toy)` reads a mod track's `.toy` `[InStadium] InStadium=` with the port's
  bool reader (`phy::Section::flag`, missing → false).
- In the shell (`main.rs`, before `new_rider`), a mod track (`modded`, not a world mod) whose `.toy` says
  InStadium true clears `rider.edge_armed` on every bike it builds (player and the other riders — both go
  through `new_rider`). It prints `stadium: <track> has no map edge`. The S59 throw (wreck 13), the P85
  shootoff warning sound and the music fade all read `edge_armed`, so they all go.
- No openmx-mods change: the rule comes from the original's own `.toy` data, which the importer already
  copies. The importer's newer `category` field (openmx-MD66) is not used — the brief asks for what the
  map is, and the source game decides that by InStadium, not by category.
- Tests: `track_mods::tests::a_stadium_is_the_toys_in_stadium_key` (spellings, CRLF, missing key/section)
  and `tests/track_mods.rs::a_stadium_mod_has_no_shootoff_and_a_free_ride_mod_keeps_it` (a generated
  flat mod ridden west off its edge twice: without the key it warns and throws; with `InStadium=TRUE` it
  neither warns nor throws and rides past x=150).

## What the original does

MX vs. ATV Unleashed (SLUS-21104, `-p atv`):

- The vehicle wreck test `./kit fn -p atv 0x4e24a0` has MXU's S59 clause: when vehicle +0x308 != 0 and the
  reason isn't already 0xd, it asks the position query `0x31d410` with 150.0 against terrain +0x3b0, calls
  vtable +0x16c (off-map test `0x4a4f20`, itself gated on +0x308), then wrecks with 0xd.
- The vehicle base constructor (vtable 0x3cd730) sets +0x308 = 1. Vtable +0x168 is the setter `0x4a4f10`
  (`sw a1,0x308(a0)`); its one caller `0x4a6d50` (vehicle, controller) passes, by the controller kind from
  `0x249330`: kind 1 (human) → `*(event 0x42de70 + 0x428)`; kinds 2/3/4 (AI, ghost, …) → 0.
- Event +0x428 is written only by `0x268040`, called once from `0x2c7e80` at 0x2c93b8 with
  `*(*(game+0x68)+0x588) == 0` — when event[0x109] is 0; otherwise it decrements event[0x109] and leaves
  +0x428 alone.
- World +0x588 is cleared in the world constructor `0x323940` (vtable 0x3c5d60) and set by the toy loader
  `0x218560` (MXToyLoader.cpp): if the `.toy` has section "InStadium" (string 0x39f41c), the bool reader
  `0x144cb0`→`0x142ed0` reads key "InStadium" (default 0) into +0x588. Neighbour: CrowdPresent → +0x584.
- Bool reader `0x142ed0`: case-insensitive prefix "T"/"ON"/"Y" → 1, "F"/"OFF"/"N" → 0, else strtol-like
  `0x3593c0`.
- So: **a human's vehicle is edge-armed only where the track's .toy has InStadium false or missing.**
  "AllowCannonball" (0x39f843, an MXSettings key) has no reader; MXU's arc constants (1497.6, 6.21118)
  have no copy.

Data (installed openmx-mods `.toy`s): InStadium=TRUE in mxvatv_fs01–08, sx01–18, mc06–08 and untamed_fs_a–d,
sx_a–l, mt_a, sm_s1; FALSE in mxvatv_nat*; missing everywhere else (mc01–05, mc09–10, sm*, other untamed).

Untamed (SLUS_217.01): strings "MXToyLoader.cpp", "CrowdPresent", "InStadium", "Cannonball" are present —
the same loader. Its code was not traced.

MXU's own tracks keep S73's league rule (`off_map::armed`); they never come through `in_stadium`.

## How it was proved

- Read from decomp (`./kit fn -p atv`, `./kit dis -p atv`, atv.xrefs.tsv), not captured.
- Live release-shell rides: mxvatv_fs01 (real mods, `--trace-at 448,448,90`) prints the stadium line, rides
  through x=742 (the 150 ft margin on its 892 ft floor) to the wall at ~800 with wreck 0 and no warning;
  the same mod copied with `InStadium=FALSE` warns at tick 61 (349 ft from the edge, mid-floor — the
  complaint) and throws at tick 127 at x=749. free06 (`--mods off --trace-at 6640,3450,90`) still warns
  at tick 0 and throws at tick 70.
- The two tests above, through `./kit check`.

## Not verified / not 1:1

- Not live-captured in MX vs. ATV under PCSX2: the rule is read from decomp only (no state with a bike at
  an FMX arena's edge was made).
- Untamed checked by ELF strings only; its arming chain is presumed the same as MX vs. ATV's.
- The event[0x109] countdown branch in `0x2c7e80` is not ported: when it is nonzero the original keeps
  +0x428 from the previous load. What sets 0x109 was not traced; the port always goes off the current toy.
- The port reads the key with MXU's reader (`Section::flag`): ON/OFF as whole words and its own number
  parse, where MX vs. ATV matches prefixes ("ONX" → on) and uses strtol. No shipped toy uses anything but
  TRUE/FALSE.
- MX vs. ATV gates every vehicle's edge on +0x308; the port's biplane edge (flight.rs) and other machines
  are not stadium-gated. Not needed for the brief (bike), left as is.
- Beyond FMX: supercross stadiums, mxvatv mc06–08, untamed mt_a and sm_s1 also lose the shootoff, because
  their toys say InStadium=TRUE — that is the original's rule, not the brief's list.
- Rumble and music fade were not measured separately; they hang off the same `edge_armed` as the warning
  line, which is gone.
- MX vs. ATV's AI/ghosts are never armed (kinds 2–4 → 0); the port's other riders on open mod maps keep
  MXU's default arming — unchanged by this task.

## Next

Nothing; the task lands with this.
