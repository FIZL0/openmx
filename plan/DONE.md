# DONE — the archive

Finished tasks, newest at the top. `./kit plan tick <ID>` moves a line here from `plan/TODO.md`, with its
text intact, so a question like "when did the FPU model land, and what did it not cover?" is one `git log`
away. Nothing is ever edited by hand here either; `./kit plan <ID>` finds a task whichever file it is in.

### Next (in this order)
- [x] **D0** Do the loop once on a game that does not exist: `./kit demo --check` (12 steps, under a second, no disc) and read `docs/EXAMPLE.md` alongside it. You are learning what each command's output looks like before any of it depends on your own disc; the walkthrough's closing section says where the synthetic program is kinder than a real one → docs/EXAMPLE.md · docs/PROVING.md | t: `./kit demo --check` prints 12/12 | j: d0-the-demo
- [x] **D2** List and extract the disc: `./kit extract --list` shows the whole tree with sizes, `./kit extract <path>` writes into `context/disc/`. Note which files are archives (one container holding many), which look like code (ELF magic, or a plausible overlay size), and which are plain data → ps2kit/disc.py (read only) · context/disc/ | t: the extracted boot ELF's size matches the directory record | j: d1-the-disc
- [x] **D3** The boot ELF: `./kit elf` on it — segments, entry point, where the code lands. Put `[boot] elf` and, if the program headers are unusual, `[boot] load` in `kit.toml`. Say in the journal what the address ranges are: code, data, BSS, and the gap where overlays go → kit.toml · docs/CONTEXT.md | t: `./kit elf` prints segments that cover the entry point | j: d3-boot-elf
