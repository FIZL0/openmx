# 1 — symbols (FINAL)

Task R2. 2026-10-15.

## What I did

Named 79 functions — the SDK and library ones Ghidra could not — in `kit.toml`'s `[symbols]` and, because
that file is git-ignored, in `research/oracle.txt` as the committed copy. `./kit fn @printf`,
`./kit xref @snprintf`, `./kit fn @scePadRead` all resolve; `./kit oracle list` reports 79 functions.

The image is stripped (D3) but it is *not* quiet: it still contains Sony's own debug strings, the C++
`__FILE__` strings its allocator and assert macros pass, and the SDK's startup banner. That is the whole
method — every name below came from a string, a syscall stub Ghidra had already named, or a function's
shape being unmistakable (a zero-byte search returning `end - start` is `strlen` and nothing else).

### printf, first, as the brief said

`./kit data --strings 'rror'` finds `ERROR: BAD GS register 0x%016lx 0x%016lx` at `0x4293e0`.
`./kit xref 0x4293e0` says one function reads it; `./kit dis` around that call site shows
`a0` = a handle, `a1` = the string, then `jal 0x356580`. `0x356580` turns out to be
`DebugBuffer::Printf(obj, fmt, …)`: `vsnprintf(buf, 0x104, fmt, va)` then append. Two more functions fell
out of its two calls — `0x1e6f60` = `vsnprintf` and `0x1e5e80` = the formatting engine behind it — and
the engine's *other* caller, `0x1e6eb0`, is `snprintf(buf, size, fmt, …)` with **1,850 call sites**: the
most-called function in the image, and the one that makes the rest of the dump readable.

The SDK's `printf` is a different function, and the proof is as good as it gets without an emulator:
`./kit data --strings 'sceCdSt'` gives libcdvd's own debug lines, each printed by the function that bears
its name — `sceCdStPause` at `0x41a3e8` prints `"sceCdStPause call"` — and all of them call `0x3fe080`.
`0x3fe080` installs an output hook (`0x3fd778`, which buffers a char and flushes the line at `'\n'`) and
calls the formatter at `0x3fda58`. Its first caller in the whole program is `main`, at `0x16c748`, with
`0x428170`:

    0a 0a 2d 2d 2d 2d 2d 2d 0a 45 6e 74 65 72 20 65   ..------.Enter e
    78 65 63 75 74 61 62 6c 65 0a 2d 2d 2d 2d 2d 2d   xecutable.------

That is the Sony SDK's boot banner, printed by `printf` from `main`. It also independently confirms R1's
identification of `main` at `0x16c720`, which R1 had only by position and shape.

### Everything else, by lever

- **The allocator names itself.** `0x43f490` is `"DebugBuffer.cpp"`, and `./kit data --strings 'cpp'`
  finds **357 source file names** — `AgeManager.cpp`, `CollisionCharacter.cpp`, `CameraBehavior.cpp`,
  `PSXModuleUtils.cpp`. They are there because every allocation passes `__FILE__, __LINE__`. So any
  function whose last two arguments are a `.cpp` string and a small integer is an allocator entry point,
  and the four at `0x1e50b0`/`0x1e51a0`/`0x1e5130`/`0x1e5220` — two taking `(size, file, line)`, two
  taking `(ptr)`, with 1,784 / 868 / 1,179 / 871 call sites — are `operator new`, `new[]`, `delete`,
  `delete[]`. `malloc`, `memalign`, `realloc` and `free` sit just below them and go to the same pools.
- **libc is hand-written in MMI.** `./kit fn` over the block at `0x40d400`–`0x411c00` reads like a
  string.h: the `(x + 0xfefefefefefefeff) & ~x & 0x8080808080808080` zero-byte trick in 64-bit and in
  128-bit `pcpyld`/`psubb`/`pand` form. `strlen` returns `end - start`; `strcat` finds the end and then
  tail-calls `strcpy`; `stricmp` and `strnicmp` fold case through the ctype table at `0x44aaa9`.
  `0x40fd40` is newlib's `rand`: seed × `0x5851f42d4c957f2d` + 1, high word, masked to 31 bits.
- **The SIF layer comes from the serve ids.** `0x3fef68` is `sceSifBindRpc` — 14 call sites, each with a
  `0x8000????` constant — and the constants name the service: `0x80000001` fileio, `0x80000003` iopheap,
  `0x80000006` loadfile, `0x80000100`/`0x80000101` padman. `0x3ff138` is `sceSifCallRpc`, nine arguments,
  103 call sites, and its `fno` argument identifies the one function in each of those wrappers.
- **libpad from one failure string.** `"libpad: sceSifSetDma faild"` at `0x44bc38` lands in `0x41a640`,
  which gives the per-port state block: base `0x4e9390`, `0x70` per port, `0x1c` per slot. Every libpad
  entry starts with `if (*(port*0x70 + slot*0x1c + 0x4e93a0) != 0)`, so the whole public API is the set
  of functions beginning with that test, and each one is then named by its arguments and its one field:
  `scePadRead` memcpys the buffer and returns its length; `scePadGetState` returns the byte at `+0x71`
  with the one remap; `scePadPortOpen` demands a 64-byte-aligned buffer; `scePadInfoPressMode` is
  literally `button_mask == 0x3ffff`; `scePadEnterPressMode` is `scePadSetButtonInfo(port, slot, 0xfff)`.
- **IOP module loading, end to end.** `0x1f1cd0` (the `PSXModuleUtils.cpp` one) reads an IRX out of the
  game's own archive, `sceSifAllocIopHeap`s room for it, DMAs it across with `sceSifSetDma`, and then
  calls `0x403c20`/`0x403c40` — three arguments and four — which are `sceSifLoadModuleBuffer` and
  `sceSifLoadStartModuleBuffer` sharing one body. This is the path D3's `modules.res` takes at runtime.
- **libdma and libgraph.** `"libdma: sync timeout"` at `0x4495d0` names `sceDmaSync` (`0x3f1600`);
  `0x3f1780` zeroes CHCR/MADR/QWC on all ten channels; and the only function that calls both `GsPutIMR`
  and `SetGsCrt` — stubs Ghidra had already named from their syscall numbers — is `sceGsResetGraph`.

## What the original does

Boots on the Sony SDK: `sceSifInitRpc`, then `printf` the banner, then one IOP service at a time
(`sceSifLoadFileInit`, `scePadInit` binding padman, `sceMcInit`), loading its IRX modules from its own
archive rather than from the disc root. On top of that sits the game's own layer: pooled `operator new`
with file-and-line tracking, a 1,850-site `snprintf`, and a `DebugBuffer` that formats into a growing
in-memory log instead of the TTY. None of its behaviour is ported here — this task only puts names on it.

## How it was proved

Nothing here is a behavioural proof, and nothing here claims to be: this is a naming task, and the test
the plan asks for is `./kit fn @printf` resolving, which it does. What each name rests on:

- **A string the function itself prints** (the strongest available without an emulator): `printf`,
  `sceCdStRead`/`StPause`/`StResume`/`StStat`, `sceMcInit`, `mcUdCheckNewCard`, `sceDmaSync`,
  `debug_printf`, and `main`'s banner.
- **A constant that names the thing**: the SIF serve ids, `rand`'s LCG multiplier.
- **A stub Ghidra had already named**, reached from the candidate: `sceGsResetGraph` via `SetGsCrt` and
  `GsPutIMR`, `pad_send_cmd` via `sceSifSetDma`.
- **Shape alone, where the shape is unambiguous**: the libc block, the allocator block, the libpad API.
- Call-site counts come from `boot.xrefs.tsv` and are in `research/oracle.txt` where they matter; they
  are a sanity check, not a proof (`snprintf` at 1,850 and `operator new` at 1,784 are the right order of
  magnitude for what they claim to be; a 3-call-site `memcpy` would not have been).

The libc and allocator entries are deliberately left as oracle candidates — they are pure functions of
their arguments and RAM, so O1/O2 can confirm every one of them cheaply over generated inputs. That is
the real proof, and it belongs to those tasks.

## Not verified / not 1:1

- **Every name in this entry is an identification, not a proof of behaviour.** No function here was run,
  in the emulator or the oracle. A wrong name is worse than `FUN_…` because it stops the next reader
  looking, so each one is listed above with what it rests on; treat anything resting on "shape alone"
  as a strong hypothesis.
- **`operator new` vs `operator new[]` (and the two deletes) cannot be told apart from their bodies** —
  `0x1e50b0` and `0x1e51a0` are byte-for-byte the same work, as are `0x1e5130` and `0x1e5220` (one
  compares the "use libc heap" flag against 0, the other against 1). I assigned scalar-then-array by
  address order, which matches the `malloc`/`realloc`/`free`/`new`/`delete`/`new[]`/`delete[]` run the
  block is in, and by call count (1,784 > 868). If something later depends on the distinction, it needs
  a live check, not a reading.
- **`sif_load_module_buffer` (`0x403650`) is LOADFILE call 6 and its sibling `0x403858` is call 7**; I
  did not pin down which Sony name goes with which number, so `0x403858` is left unnamed. The two public
  wrappers above it are named from their argument counts (3 vs 4, the 4th being the result pointer),
  which is how `SifLoadModuleBuffer` and `SifLoadStartModuleBuffer` differ.
- **`0x41b768` is left unnamed**: it is a libpad RPC wrapper with command 11 taking a two-word struct, and
  I could not decide between the plausible Sony names without guessing.
- **`sceCdStPause`, `sceCdStResume` and `sceCdStStat` have zero call sites.** They are named from the
  strings they print, so the names are sound, but the game appears not to use them — do not assume the
  CD streaming path runs through them.
- **The rest of libgraph is not individually named.** `0x3f1000`–`0x3f3900` holds libdma and libgraph
  interleaved, around 70 functions; only the four with independent evidence are named. The GS packet
  builders the renderer will actually need are still `FUN_…`, and finding them is a V-phase job with a
  capture beside it, not a string hunt.
- **`pad_button_info`, `pad_dma_buf`, `pad_send_cmd`, `cd_stream_cmd`, `tty_write`, `heap_*`,
  `sif_fileio_init` and `printf_nohook` are descriptive names, not SDK names.** They are internal
  functions whose Sony name I do not know; the name says what it does so the dump reads, and it is
  deliberately not `sce…` so nobody mistakes it for a confirmed API.
- **`[symbols]` lives in a git-ignored file.** `kit.toml` is not committed, and `./kit run` symlinks
  `context/` and the ISO into each worktree but not `kit.toml` — so this worktree had no config at all
  until I symlinked the one in the main checkout. The names are therefore written twice: into that shared
  `kit.toml` (so `@printf` resolves for everyone now) and into `research/oracle.txt` (so they survive a
  fresh clone). If `./kit fn @printf` ever stops resolving, the `[symbols]` block has been lost and
  `research/oracle.txt` is where to get it back. Worth a `K` task to make the runner link `kit.toml`.
- Unrelated to this task, but it changed under me and the next agent should know: **`./kit doctor` now
  reports `BIOS present`**, which was D1's blocker.

## Next

R3 — the main loop — and it starts with `./kit fn @main`, whose 36 callees are now readable: the ones
that call `scePadInit`, `sceSifLoadFileInit` and `sceMcInit` are the startup, and what is left is the
loop. R5's RNG hunt should check `@rand` first and then look for the game's *own* generator, because
`0x40fd40` is newlib's and a racing game almost certainly has its own.
