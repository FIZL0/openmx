# 1 — the main loop and the tick order (FINAL)

Task R3. 2026-10-15. Static only: there is still no BIOS, so nothing here was watched running.

## What I did

Walked out from `main` (`0x0016c720`, found by R1) to the frame loop, then read every function the loop
calls directly, in order, until each one was either (a) a leaf that touches hardware, (b) a generic
tree-walk that dispatches to subsystems, or (c) plainly somebody else's task. The result is the numbered
tick order below. Nothing was renamed, nothing was ported: this task is notes.

Tools: `./kit fn`, `./kit xref`, `./kit dis`, `./kit data` (to read vtables as u32), and capped greps of
`boot.c` for struct offsets (`grep -n "0x3c4" … | head`).

## What the original does

### The loop itself

`main` ends in six instructions' worth of loop:

```
uRam0047b188 = 1;                                        /* "running" */
do { r = app->iface[0x40](app); } while (r != 0);         /* the frame */
app->iface[0x6c](app);                                    /* shutdown  */
```

- **The app object is a static at `0x00479990`** (bss), reached through the constant pointer
  `0x0041f770` in .data — read-only at runtime, 2,800 reads, never written. Every subsystem gets at the
  app this way, so `0x0041f770` is the single global the port has to model.
- It is constructed **before `main`**, by a static constructor: `0x0044c840` → `0x00200b90` →
  `0x0016a1e0` → `0x001300c0`-ish base chain. Three levels deep, and each level installs its own
  dispatch table at **app+0x3c4**: `0x0044da60` (engine base) → `0x0044eec0` (middle) →
  `0x00452370` (the game's own). The most derived one wins, so the live table is `0x00452370`.
- **This is not a C++ vtable at `+0`** — the table pointer is a *member* at `+0x3c4` and the callee gets
  the app as its first argument. Read it as a struct of function pointers. Slot numbers below are byte
  offsets into it; `./kit data 0x452370 0xa0 --as u32` prints the lot.
- Loop exit: the tick returns 0. Then slot `0x6c` = `0x00131cc0` runs the shutdown and `main` returns 1.

The three tables, for later tasks: the game table overrides 15 of the base's 40 slots (and fills three
slots the base leaves null, which is how you can tell the base alone is never instantiated). The ones
that matter per frame: `0x40` tick (`0x00201d00`, base `0x00131100`), `0x38` render-all
(`0x0016a7c0`, a one-instruction `j 0x00130b50` — so: base behaviour), `0x34` a stub that returns 1
(`0x0016a7b0`), `0x80` frame-marker notify (`0x001321d0`, not overridden).

### 1. The tick order, top level — `./kit fn 0x00201d00`

The game's tick, slot `0x40`. Everything the port does per frame hangs off this.

1. **Service one pending mode change**, from the request code at app+0x404 (`switch`, cases 1–8):
   load a level, re-enter one, restart, tear one down, quit (case 8 → return false). Cases 2 and 7 check
   "still loading" (`*(app+0x3d0)+0x100 != 0 || **(*(app+0x3d0)+0x104) != 0`) and fall through to the
   frame without acting, so **the frame keeps ticking while a level loads** — the loader is not a
   blocking call. Cases 4 and 6 deliberately burn one extra frame (`+0x578`/`+0x57c`) between tearing
   down and building up.
2. **Run the engine tick** (`0x00131100`, below).
3. **If a quit was requested** (app+0x454 != 0): flush the display-list DMA (`0x00179a10(gfx,1)`), wait
   for the VIF1/GIF path to go idle (`gfx->driver[0x28](gfx,1)`), and return 0 — which is how `main`'s
   loop ends. Otherwise return 1.

### 2. The engine tick — `./kit fn 0x00131100`

In order:

1. **Sample the clock** into app+0x338 (ms, integer). The clock is **EE Timer 0**, not vsync: `main`
   programs RCNT0 with CLKS=BUSCLK/256 (576,000 Hz), TARGET=0xe100 (57,600 = exactly 100 ms), CUE,
   CMPE, and installs INTC handler 9 at `0x0016cbf0` (`./kit dis 0x16cbf0 0x16cc40`), which does
   `COUNT %= 57600`, `++(*0x0047b190)`, clears the compare flag, `EI`. So wall time in ms is
   `*0x0047b190 * 100 + RCNT0_COUNT * 256000 / 147456000`, computed inline — in **float** — at six
   places in the engine. First frame only (`app+0x33c == 0`) it calls `0x00131800` to prime the
   baseline instead.
2. **Derive dt.** raw `dt = (now - prev) / 1000.0`; if `dt > 0.1` the history is thrown away and
   `dt = 0.033`. Raw dt is stored at app+0x358. Then dt is pushed into a **30-entry ring at `0x0046a830`**
   (index at `0x0046a8a8`, filled with the first dt on the first frame) and the **mean of all 30** is
   what the simulation gets. Then: if the 30 fps cap flag `0x0046a808` is set, clamp to 0.03333 (and the
   ms delta to 0x21); if a time-scale object exists at `0x004e24c0`, `dt += dt * (*(obj+0x40))` with a
   floor of 0.001 — that is slow motion / fast forward.
   **So the sim's dt is a 30-frame moving average of wall-clock time, not a fixed 1/60.**
3. `*(app+0x104)->table[0x65c][0x10](dt, mgr, 1)` → `0x0011d910`: **the input / device manager update**.
   It walks two 2-entry arrays (mgr+0xc and mgr+0x14 = two ports × controllers and memory cards —
   R2 names `0x0041a758` scePadInit and `0x0041b9d8` sceMcInit, both called from `main`), calls
   `dev->vt[0x6c](dt)` on each and `dev->vt[0x60]()` to ask whether anything changed, then
   `0x0011ddf0` on change and `0x0011dab0` (which fires app slots `0x4c`/`0x50` — connect/disconnect
   notifications). The game's pad class's `vt[0x6c]` is `0x0016ab10`: it calls **scePadRead**
   (`0x0041ad08`) and normalises buttons and sticks into floats. **The pad is read here — before the
   simulation, once per frame.**
4. `app->iface[0x80](app, 1)` → `0x001321d0`: **frame marker**. Walks the observer list at app+0x38c and
   hands each one the current time in seconds and the marker id. Instrumentation only — but it reads the
   Timer 0 clock, so it is on the timing path.
5. **If bit 3 of the byte at app+0x324 is clear** (it is: the constructor clears it and *nothing in the
   image ever sets it* — a debug freeze that was compiled out), run the simulation and the render:
   1. phase (app+0x32c) = **2**; app+0x344 = dt; app+0x348 = 1/dt (or 1.0 if dt <= 0);
      `++app+0x374` (the frame counter, wrapped at 0x7fffffff).
   2. **The simulation**: `root->node[8][0x38](dt)` where `root = *(app+0x378)`. Returns 0 →
      the tick returns 0 and the game exits.
   3. phase = 0; `app->iface[0x80](app, 2)`.
   4. **The render**: `app->iface[0x38](app)` → `0x00130b50` (below).
   5. The countdown at `0x0046a7f8`: when it reaches 0 and `0x0046a800` is set, free a one-shot object
      (`0x001fd2e0`) — a deferred "drop the boot/loading screen" of some sort.
   6. `0x001a1c40(*0x0048a560)`: advance the display-list buffer ring (`+0x10 = (+0x10 + 1) % +0xc`)
      and bump its total counter.
6. `0x00199b20(*0x00468220 + 0xc0)`: **ship the frame's queued commands to the IOP.** Runs whether or
   not step 5 ran. Behind it, `0x00198b90` does a `sceSifCallRpc` (`0x003ff138`, R2's name) of RPC
   function **0x20 / 0x21**, alternating between two 0x4340-byte buffers at `+0x1180` and `+0x54c0` of
   the 0x99c0-byte object at `0x00468220`, with a 0x4308-byte payload, then zeroes the buffer it just
   sent. One RPC per frame, double buffered, fire and forget. Almost certainly audio (whose module list
   D3 found in `modules.res`) — not proved here.
7. **Profiling** (only if app+0x3c0 != 0): re-read the clock, fold `(now - frame_start)/1000` into the
   running mean at app+0x3a4 with sample count app+0x3b0, and update min/max at app+0x3a8/+0x3ac.
8. app+0x33c = app+0x338 and app+0x334 = app+0x330 (this frame's clock becomes the previous frame's).
9. If bit 4 of app+0x325 is set: set bit 3, clear bit 4 (a one-frame-late latch, purpose unknown).
10. Return 1.

### 3. The render — `./kit fn 0x00130b50`

Called as app slot `0x38`. The app holds **up to four "views"** at app+0x35c…+0x368 (the current index
in app+0x370, the loop index in app+0x110); each view is an object whose own table is at its `+0x26c`
(`0x0044d130`) and which embeds a scene-tree node at its `+8`.

1. app+0x37c = app+0x380 (last frame's primitive/byte count), app+0x380 = 0, app+0x384 = 0.
2. For each of the four views: `view->vt[0x90]()` → `0x00113ad0` → node **disable** (`0x00132ae0`
   clears the two enable bits at node+0x29 and calls the node's `onDisable`).
3. `gfx->driver[0x14](gfx)` → `0x00179470`, four instructions: set bit 0 of gfx+0x9c ("inside a frame"),
   zero gfx+0x74, return 1. **If it returned 0 the whole render is skipped** (it cannot, here).
4. For each view index 0..3 that is non-null:
   1. if a previous view was active, `prev->vt[0x90]()` (disable it);
   2. `view->vt[0x8c]()` → `0x00113a80`: node **enable** + `0x001a1c80(gfx, view)` which stores the view
      in **gfx+0 (the current render target)** and, if its size changed, calls `view->vt[0xc8]`;
      then stamps view+0x1b0 = gfx+0xc + 1;
   3. `0x00179a10(gfx, 1)`: sync the DMA path and **swap the display-list buffer pair**
      (gfx+0x54 = 1 or 2 picks between the pairs at gfx+0x5c/0x60 and gfx+0x68/0x6c);
   4. phase = **3**; `root->node[0x40]()` — scene pass A;
   5. `0x001129b0(view)`: bumps two $gp globals and calls `0x001acaf0` on view+0x38 (a scene/visibility
      kick);
   6. phase = **4**; `root->node[0x44]()` — scene pass B;
   7. phase = 0; `app->iface[0x80](app, 3)`;
   8. **`0x00112710(gfx+0)`: the GS kick.** Stamps view+0x1ac/+0x1b0 with the ring index and calls
      `gfx->driver[0x38](gfx, view+0x180)` (`0x00179ef0`) — the view's display list goes to the GIF;
   9. `app->iface[0x34](app)` — a stub returning 1 in this build;
   10. phase = **5**; `root->node[0x48]()` — scene pass C;
   11. `0x00112a00(view)` → `0x001ad180` (the counterpart of step 4.5);
   12. if `0x0048ab90` is set, `0x00184bd0()` (a debug overlay — it reads uninitialised saved registers
       in Ghidra's output, so it is probably reached only through a sibling entry);
   13. phase = 0; `app->iface[0x80](app, 4)`.
5. app+0x380 += gfx+0x44 (the frame's cost counter).
6. **`gfx->driver[0x18](gfx)` → `0x001794a0`: end of frame.** It calls `gfx->driver[0x28](gfx, 1)`
   (wait for the DMA path to drain) and then **`0x00191da0(gsctx, 1)` — the flip**, where the vsync wait
   lives (below). If it returns 0 the render returns 0 (and the tick ignores that).
7. FPS bookkeeping: once a second, `app+0x340 = (frames * 1000) / elapsed` from the GS context's frame
   counter (gsctx+0x40); if profiling is on, min/max/mean of the GS context's instantaneous fps
   (gsctx+0x68) into app+0x3b4/+0x3b8/+0x3bc.
8. For each of the four views: `view->vt[0x8c]()` (enable again, for the next frame).
9. phase = **6**; `root->node[0x4c]()` — scene pass D. phase = 0. Return 1.

### 4. Where vsync is actually waited on

Not in the loop. The chain is: render step 6 → `0x00191da0` (the GS context's flip) → **`0x00191040`**:

```
while (*0x0048a520 < n) { ...spin... }
```

`0x0048a520` is incremented by the **GS INTC handler at `0x00191000`**
(`AddIntcHandler(0, 0x191000, -1)` in `0x001914f0`, with `GS_CSR = 8` and `GS_IMR = 0xfffff7ff` —
i.e. clear VSINT and unmask VSMSK). The handler: `++*0x0048a520`, `GS_CSR = 8`, `GS_IMR = 0xfffff7ff`,
`sync`, `EI`. So the counter is **vblanks since the last flip**, and `0x00191040(n)` means "wait until n
fields have passed".

Which `n` comes from the GS context's mode word gsctx+0xc: mode 1 → `0x00191040(2)` if fewer than two
fields have passed (a **30 Hz lock**); mode 2 → `0x00191040(1)` if none has (**60 Hz**). Then the
counter is reset to 0, the field parity gsctx+0x3c = gsctx+0x40 & 1, `0x001919e0` programs DISPLAY/DISPFB
for the finished buffer, and the frame counter gsctx+0x40 increments.

Two details worth keeping:

- While waiting, if more than 4 ms have gone by, it calls `app->iface[0x80](app, 8)` once — so marker
  id 8 means "idle, waiting for the field".
- If the wait exceeds `n * 17.0` ms it writes `GS_CSR = 8` and `GS_IMR = 0xfffff7ff` itself and keeps
  going: a deliberate recovery from a lost GS interrupt. A port must not hang here either.

The *other* GS wait in the renderer, `0x00179cf0` (driver slot `0x28`), is **not** vsync: bit 0 of its
flags waits for the VIF1 DMA tag address to leave the buffer being refilled, bit 1 sets `GS_CSR |= 2`,
sends a FINISH-setting packet (`0x00190df0`) and spins on `CSR & 2` — that is the **FINISH** bit, i.e.
"the GS has drained", and the per-frame calls all pass flags = 1 (DMA only).

### 5. The two enumerations the port should copy

**Phase, at app+0x32c** (written by the engine, read by subsystems that need to know what is running):
0 nothing, 1 initialising (set by `0x00130490`), 2 simulate, 3 scene pass A, 4 scene pass B,
5 scene pass C, 6 scene pass D.

**Frame marker ids, passed to app slot `0x80`**: 1 top of the frame, 2 simulation done, 3 mid-render,
4 view finished, 8 waiting for the field, 16..31 DMA stall retries (from `0x00177e30`).

### 6. How the per-frame work is actually reached

Both `*(app+0x378)` (the sim/scene root) and `*(app+0x114)` are 0x30-byte nodes built by `0x001328b0`
with table **`0x0044db10`** at their `+8`, linked by `0x00132f50`. A node is:

| off | meaning |
|---|---|
| +0x08 | its table |
| +0x14 | next sibling |
| +0x18 | first child |
| +0x20 | which passes *this* node wants (bit mask) |
| +0x24 | which passes its *subtree* wants (the OR of its children — an early-out) |
| +0x29 | bit 0 enabled, bit 1 self-enabled, bits 2–3 two more gates |

The base implementation of every pass is the same four lines: if the subtree mask has the pass's bit,
walk the children and call the same slot on each enabled one. The masks are
**0x38 → 0x2 (simulate), 0x40 → 0x1000, 0x44 → 0x4, 0x48 → 0x8, 0x4c → 0x10** — so the four render
passes are bits 2,3,4 plus a late-added 0x1000 that runs *first*. Subsystems are nodes that override
these slots; naming them is R4/R5's job, not this task's.

## What this means for the port

- The port's frame is: service one mode change → sample Timer 0 → average dt over 30 frames → read the
  pads → simulate the node tree → render four views in four passes with a GS kick in the middle → flip
  with a 1-or-2-field wait → one RPC of queued commands to the audio side.
- **dt is wall-clock, averaged, and clamped.** A replay test cannot feed a fixed 1/60 and expect a match;
  it has to drive the same ms clock, which means the replay fixture needs the ms value per frame, not
  just the inputs. This is the single most important consequence in this entry.
- Rendering is kicked once per view, inside the view loop, between scene passes B and C — not at the end
  of the frame.
- The quit path is a flag (app+0x454) checked after the frame, plus mode-change case 8. There is no
  other way out of the loop.

## How it was proved

Static reading only, with these cross-checks — each one is two independent sources agreeing:

- **The loop is the loop.** `main`'s only backward branch is the `do/while` over slot `0x40`;
  `./kit xref 0x00131100` finds exactly two references to the engine tick, both of them *data*
  (`0x0044daa0` and `0x0044ef00` = base and middle tables at slot 0x40) plus one call, from
  `0x00201d00` — which itself is only referenced from `0x004523b0` = game table slot 0x40. The table
  slot and the call site agree.
- **The table offsets are right.** The base table leaves slots 0x34, 0x74 and 0x84 null, and those are
  exactly the three slots the derived tables fill; read with any other alignment, live code would be
  calling null. `./kit data 0x44da60`, `0x44eec0`, `0x452370`, compared slot by slot.
- **The clock formula matches the hardware setup.** `main`'s RCNT0 writes (CLKS=2 → BUSCLK/256 =
  576,000 Hz, TARGET=57,600) say one interrupt per 100 ms; the handler's `++` and the engine's
  `*0x0047b190 * 100 + COUNT * 256000/147456000` are the same 100 ms in the other direction.
  `./kit dis 0x16cbf0` for the handler, `./kit fn 0x0016c720` for the setup.
- **The vblank counter is a vblank counter.** `0x001914f0` installs INTC handler **0** (GS) and unmasks
  VSMSK in GS_IMR; the handler at `0x00191000` increments `0x0048a520` and re-arms the same two
  registers; `0x00191040` is the only reader and it treats it as a field count.
- **The sim gate is dead.** `grep -c "0x324"` = 54 sites; none of them sets bit 3, and the constructor
  (`0x00130490`'s object, line "& 0xf7") clears it. So the branch is taken every frame in this build.
- R2's independently derived names agree with six functions this trace reached by shape alone
  (`sceSifCallRpc`, `scePadRead`, `scePadInit`, `sceMcInit`, `sceGsResetGraph`, `snprintf`).

No oracle run (nothing here is a pure function of its arguments) and no capture (no BIOS, so no emulator
— see `d1-the-disc/2-the-bios-READY.md`). `./kit check` passes: this task adds no code.

## Not verified / not 1:1

- **Nothing in this entry was observed running.** There is no BIOS, so the order is read off the
  decompilation, not off a frame of the real game. Every "once per frame" claim is "once per pass through
  this function". The whole entry is a hypothesis of the shape R1 warned about, and the first live
  capture should re-check it — in particular the dt pipeline, which is the part a port gets silently
  wrong.
- **The 30-frame dt average is read from Ghidra's float arithmetic**, which is exactly where the R5900
  decompilation is least trustworthy. Before anything ports it, the loop at `0x00131100` wants
  `./kit dis`-level reading of the actual `mula.s`/`madd.s` sequence, and ideally an oracle run of
  `0x00131800`/the averaging block.
- **`gsctx+0xc` (1 field vs 2, i.e. 30 Hz vs 60 Hz) — I did not find what sets it.** It is not set in the
  GS constructor `0x001912e0`. So I cannot say whether MX Unleashed runs its loop at 30 or 60 Hz, which
  is the single biggest open question left by this task.
- **The IOP RPC at step 6 is "almost certainly audio"** on the strength of its size, its double buffer
  and the module list from D3 — not proved. RPC function numbers 0x20/0x21, server unknown.
- **The four "views" are four render targets by shape** (loop bound 4, per-view enable/disable, one GS
  kick each). Whether the retail game ever has more than one non-null is not known; split-screen
  multiplayer would be the obvious reason it is four.
- **Scene passes A–D (node slots 0x40/0x44/0x48/0x4c) are not identified.** Masks and order are proved;
  what each pass *is* (cull / build list / submit / post) is R4/R5.
- `0x00184bd0` (the conditional overlay at render step 4.12) decompiles with uninitialised `unaff_s*`
  registers, so Ghidra has mis-detected its entry point; I read its callers' guard only, not the
  function.
- The marker ids passed to slot `0x80` are the five I found call sites for (1,2,3,4,8 and the 16..31
  stall range). There may be others behind function pointers.
- The purpose of the app+0x325 bit-4 → bit-3 latch at the end of the tick is unknown.
- Addresses from this task were **not** added to `research/oracle.txt` or `kit.toml` on purpose: R2 is
  editing both right now in another worktree, and none of these is an oracle target yet (the oracle needs
  pure functions; these are all stateful). They are all in this entry, with `./kit fn` recipes.

## Next

R4/R5 (the subsystems) should start from the node tree, not from `main`: every subsystem in this game is
a node under `*(0x00479990 + 0x378)` that overrides one or more of slots 0x38/0x40/0x44/0x48/0x4c of
table `0x0044db10`. `./kit xref` on each of those five base implementations lists the overriders, which
is the subsystem inventory, ordered by the pass they run in.

The two questions a first live capture should answer, in order: (1) what `gsctx+0xc` is at runtime —
30 Hz or 60 Hz; (2) the per-frame ms clock and dt, recorded alongside the inputs, because the replay
harness needs them as part of its input.
