# 1 — r11 names (FINAL)

Task R12. 2026-11-04.

## What I did

- In this slot's context/ (nothing written outside s3): `SLUS_211.04` and `GAME_REL.BIN` copied from s1,
  folded with `tools/mxvatv_overlay.py` into `context/mxvatv-ps2/atv_game.elf` (byte-equal to s1's);
  `MXvsATV.exe` and the `pc.gpr`/`pc.rep` project copied from the main checkout (read only there).
- `[other.pc]` and `[other.atv]` from kit.toml.example added to s3's kit.toml (git-ignored, not committed).
- `./kit ghidra name -o pc`: 61 functions renamed from the oracle; the other 59 pc rows had their names
  already (main's project was named before R11's rows landed). A second run renames 0.
- `./kit ghidra analyse -o atv` (1m25s, NameOther as its post-script): 279 rows named, 34 had no function.
- Of those 34, 23 sit on an instruction (code reached only through vtables, which Ghidra never made a
  function of: the quad's init, wire/surface/strut slots, the list sub-database slots, the biplane/kiowa
  twins…); 10 are not instructions (`atv_net_clock`, `atv_const_w1`, `atv_const_up`, the two camera
  vtables, `atv_ground_ring`, `atv_normal_table_flag`, and the three `atv_quad_*_thunk` rows, whose bytes
  Ghidra never disassembled and which nothing references); `atv_world` is not in the ELF's memory.
- `ghidra_scripts/NameOther.java`'s oracle step: no function at the row → `createFunction` with the row's
  name if there is an instruction there, else a primary user label (once); an address outside memory is
  said so. Its summary line adds "(N functions created, N labels)". The pc project hits neither branch.
- `./kit ghidra name -o atv` with it: 23 functions created, 10 labels; the next run renamed 1 (a created
  function's name settled on the second pass), the third renamed 0.
- `./kit ghidra dump -o pc` and `-o atv`, so `./kit fn <row> -o pc|atv` works here.

## What the original does

Nothing new about the game: names only.

## How it was proved

- pc.funcs.tsv from the dump: all 120 distinct pc row names are function names (namespace stripped).
- atv.funcs.tsv: 302 of the 313 distinct atv row names are function names; the 11 not are the 10 labels
  and `atv_world`. Spot checks: `./kit fn atv_quad_init -o atv` (1372 bytes), `./kit fn atv_list_pick -o atv`
  (the 8-byte jump), `./kit fn atv_bike_put_back -o atv`.
- Idempotence: a repeated `name` renames 0 on pc, and 0 on atv from the third run.

## Not verified / not 1:1

- The projects are in s3's context/ only. The main checkout's `context/ghidra/pc.rep` was not touched
  (the task forbids it), so main still lacks the 61 new pc names until someone copies s3's
  `context/ghidra/pc.*` there or runs `./kit ghidra name -o pc` in main; there is no atv project in main.
- The 23 created functions take Ghidra's own bounds and signatures; none was read against the oracle rows'
  notes beyond the three spot checks.
- The three thunks are labels, not functions: their bytes were left undisassembled rather than guessed.
- `atv_bike_put_back` has two rows at one address (same name), harmless.
- Which created function's name changed on the second `name` run was not traced.

## Next

none
