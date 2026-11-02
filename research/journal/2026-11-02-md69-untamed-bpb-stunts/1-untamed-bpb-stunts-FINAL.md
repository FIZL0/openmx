# 1 — Untamed's BPB stunt table (FINAL)

Task MD69. 2026-11-02.

## What I did

- `crates/game-sim/src/bpb.rs`: a reader for Untamed's binary INI (BPB), from the format openmx-mods
  wrote down (`notes/mxvatv-later.md`, *Scenes: binary (BPB)*; its `tools/bpb` decoder): header,
  16-byte section and key records, the four value types (1 string, 2 short string in the record,
  4 i32, 8 f32). A section's title is name + `%d` of its index (`Stunt` 1 is `Stunt1`). It builds the
  same `phy::Ini` the text reader gives (new `Ini::from_parts`, crate-private), so every getter reads
  a BPB as it reads text. `bpb::ini(bytes)` takes either, as SLUS_217.01's loader (`0x001425c0`) does.
  A float goes in as the shortest decimal of it as a double, which the text reader's `strtod` +
  truncating double-to-single reads back to the same bits (tested on 0.4, 0.1, 1e-7, ...).
- `trick_pack::table` reads through `bpb::ini`, takes `[StuntNN]` or `[StuntN]`, and takes the
  gesture from `DefaultKeyMappingClassic` (Untamed's key name, per openmx-mods' import-tricks) where
  `DefaultKeyMapping` is missing. The untamed_tricks mod.json names the `.ini.bpb` itself, so this
  alone fixes the upload's "has no [Stunt01]".
- `crates/openmx/src/trick_packs.rs`: the pack's table path takes `<table>.bpb` first when it exists,
  then the named file (the loader's order), for a pack whose mod.json names the text file.
- `crates/game-sim/src/trick.rs` (the original's own `stunts.ini`) is unchanged: MX Unleashed's disc
  has only the text table.

## How it was checked

- `crates/game-sim/tests/bpb_stunts.rs`: a BPB generated in the test (Stunt1..Stunt54, every value
  type) loads as 54 rows bit for bit; the same table as an MX vs. ATV text table (`[Stunt01]`)
  reads to equal rows; a cut-short BPB and a type-3 key are refused; Untamed's own table, when
  `mods/tricks/untamed_tricks` is installed, loads with 54 stunts (Varial = Stunt37, Duration 1.84;
  the 360s at 53/54, HoldPointValue 15000). That last one was run once with the real file copied in
  from the main checkout's mods/ and passed; it skips without it.
- `cargo test --release -p game-sim --no-fail-fast`: 944 passed, 0 failed (the existing text-table
  pack tests in ride.rs and rotation_modifier.rs among them). `cargo check --workspace --tests` clean.

## Not verified / not 1:1

- The game looks a name up by its djb2 hash first; here names are matched case-folded, and the hash
  is not checked against the name. A BPB whose hash and name disagree would read differently (none on
  the disc does, per openmx-mods' proof).
- ZLIBME-wrapped BPBs are refused (none is on the disc).
- A float key read as text gives the exact decimal, not the game's `%.06f` get-as-text; no stunt
  table key is read that way.
- The whole Untamed pack was not ridden in the app (only its table read); the other BPB asks in
  openmx-mods notes/openmx-asks.md (materials, toys, particles, sounds) can now call `bpb::ini`, but
  none of them is wired here.
