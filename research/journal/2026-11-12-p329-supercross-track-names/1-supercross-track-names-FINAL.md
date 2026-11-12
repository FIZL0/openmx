# 1 — supercross track names (FINAL)

Task P329. 2026-11-12.

## What I did

- `menu_race.rs`: `RACE_SUPERCROSS`, the 24 Supercross tracks by the disc's names (Los Angeles …
  Atlanta), replacing P327's "Supercross NN" placeholder in `race_tracks(0)`. Test pins sx01, sx24, sx16.

## What the original does

`appstart.res`'s `tracks\tracks.dat` gives each `[SXnn]` block `TextID=STR_SXnn` (checked all 24, sx01
to sx24 in order). The name comes from the UI string table:

- `loc\ui.loc`: u32 id count (1551), u32 size of the names block, the NUL-separated `STR_*` id names,
  then one u16 per id: that string's length in the English table.
- `loc\l_ui_eng.bin`: u32 size of the text block, then NUL-separated strings, padded with zeros.

The text block holds 1581 strings to ui.loc's 1551 ids, so pairing the two lists by index does not work
directly. Across the track/rider part the text runs 8 strings ahead of the ids: id i is string i+8.
Every pair from STR_NAT17 to STR_RIDER_SCOTT_THODE has a string length equal to ui.loc's u16 for that id.
That offset also gives the National names `worlds::NATIONALS` already uses (Waimea, Manassas Park,
Ensenada, Sliderock, Signal Butte, Racine: Short for NAT17..NAT22).

STR_SX01..24 → Los Angeles, Jackson, San Diego, Phoenix, Memphis, San Antonio, Lincoln, Denver, Provo,
Las Vegas, Detroit, Pittsburgh, Lynchburg, Charlotte, Chicago, Springfield, Dallas, Norfolk,
Bridgeport, Rochester, Cleveland, Cincinnati, Houston, Atlanta.

## How it was proved

Read off the disc files in `context/res/appstart` with a throwaway Python script (not kept). The checks
were the 24 TextIDs in tracks.dat, the per-id lengths in ui.loc matching each string, and the
Nationals overlap with the names already in the port.

## Not verified / not 1:1

- I didn't see the original's track-select screen show these names (no capture or screenshot). The
  mapping rests on the TextID and the string table as described above.
- I didn't work out how the game itself pairs ids with strings when the text block has 30 extra
  entries. The +8 offset is proved only over the run I checked (NAT17 through the rider names), by the
  length match. The extras come in earlier in the file (the first appears around STR_SPLIT_SCREEN,
  where an `" %"` string sits), not in the SX run.
- The names are hard-coded, as `RACE_NATIONALS` is. They are not read from the user's disc at run time.

## Next

Nothing.
