# D3, D4 — the boot ELF, and the overlays that do not exist

2026-10-15. Tasks: `./kit plan D3`, `./kit plan D4`.

One static EE executable, 3.5 MB, loaded once at `0x100000` and never paged. **No overlays.** The only
other code on the disc is seven IOP modules and 108 VU1 microprograms, neither of which is EE code the
port has to reimplement as a loaded image. That makes the rest of the port simpler than the kit's backlog
assumes: one address space, every function always resident, and `[[overlay]]` blocks in `kit.toml` stay
empty. Both tasks done.
