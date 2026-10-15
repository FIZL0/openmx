# DONE — the archive

Finished tasks, newest at the top. `./kit plan tick <ID>` moves a line here from `plan/TODO.md`, with its
text intact, so a question like "when did the FPU model land, and what did it not cover?" is one `git log`
away. Nothing is ever edited by hand here either; `./kit plan <ID>` finds a task whichever file it is in.

### Next (in this order)
- [x] **D0** Do the loop once on a game that does not exist: `./kit demo --check` (12 steps, under a second, no disc) and read `docs/EXAMPLE.md` alongside it. You are learning what each command's output looks like before any of it depends on your own disc; the walkthrough's closing section says where the synthetic program is kinder than a real one → docs/EXAMPLE.md · docs/PROVING.md | t: `./kit demo --check` prints 12/12 | j: d0-the-demo
- [x] **D2** List and extract the disc: `./kit extract --list` shows the whole tree with sizes, `./kit extract <path>` writes into `context/disc/`. Note which files are archives (one container holding many), which look like code (ELF magic, or a plausible overlay size), and which are plain data → ps2kit/disc.py (read only) · context/disc/ | t: the extracted boot ELF's size matches the directory record | j: d1-the-disc
- [x] **D3** The boot ELF: `./kit elf` on it — segments, entry point, where the code lands. Put `[boot] elf` and, if the program headers are unusual, `[boot] load` in `kit.toml`. Say in the journal what the address ranges are: code, data, BSS, and the gap where overlays go → kit.toml · docs/CONTEXT.md | t: `./kit elf` prints segments that cover the entry point | j: d3-boot-elf
- [x] **D4** Overlays: `./kit elf --scan` guesses which disc files are code loaded over a fixed address; confirm each by looking for a jump table or a string that belongs to that part of the game, and add an `[[overlay]]` block per confirmed one. Most PS2 games have at least a menu overlay and a gameplay overlay, and the gameplay one is where you will spend the whole port → kit.toml | t: an overlay dumps with sensible function boundaries at its `load` | j: d3-boot-elf
- [x] **D5** The archive format, if D2 found containers: the header layout (count, offsets, sizes, names or hashes), enough to list and extract members. Write it up with a hexdump in the journal before writing any code; formats are usually obvious once you can see three entries side by side → crates/game-sim/src/ (new `archive.rs`) · docs/CONTEXT.md | t: every member of one archive extracts and the sizes add up to the file | j: d5-archives

### Phase 2 — a decompilation you can read
- [x] **R1** (after D4) Ghidra: `./kit ghidra` on the boot ELF and each overlay, with the EmotionEngine extension, producing `context/decomp/<program>.c`. Check the dumps are at the right addresses (a function in the overlay dump should sit inside the overlay's load range) and note how long a full analysis takes on this machine → kit.toml `[ghidra]` · ghidra_scripts/ | t: `./kit fn <addr>` prints one function from each program | j: r1-decompile

### Phase 4 — the arithmetic, before any ported math
- [x] **F1** The EE FPU in `crates/ps2-float`: add, sub, mul, div, sqrt with the PS2's rules — round toward zero, no denormals (flushed), no NaN or infinity (clamped to the largest magnitude), and the smaller addend masked to the exponent difference minus one guard bit. Port physics before this and you will not know which of the two is wrong → crates/ps2-float/src/fpu.rs | t: crates/ps2-float/tests/fpu.rs | j: f1-ee-fpu
