# R2 — symbols

2026-10-15. Task: `./kit plan R2`.

79 SDK and library functions named, in `kit.toml`'s `[symbols]` and in `research/oracle.txt` (the
committed copy, because `kit.toml` is git-ignored). `printf` is `0x3fe080`, proved by the Sony boot
banner `main` prints through it; the game's own `snprintf` is `0x1e6eb0` with 1,850 call sites. The
levers were the debug strings Sony's libraries print, the 357 `__FILE__` strings the C++ allocator
passes, the `0x8000????` SIF serve ids, and the syscall stubs Ghidra had already named. The libc and
allocator entries are left as oracle candidates for O1/O2 — that is where they get proved.
