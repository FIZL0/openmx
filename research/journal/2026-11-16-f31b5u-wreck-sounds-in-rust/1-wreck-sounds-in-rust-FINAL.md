# 1 — wreck sounds in rust (FINAL)

Task F31b5u. 2026-11-16. Parent: research/journal/2026-11-14-f31b5-whole-frame-in-rust.

## What I did

- Measured what the rider leaf still ran in the interpreter on flip-1 with every switch on: 1,823
  instructions, all in the wreck cue's outside calls (row 125). Rand 432 (8 draws × 54), cue busy 59,
  sound busy 11, sound play 1,217, cue made playing 104.
- The task's other two items were already at 0:
  - "the wreck entry's heap": its resize and the clip start's saved-pose allocs land on the heap
    entries, which the `heap` switch (F31b5r) serves in Rust. The census shows them as 0.
  - "the tail's out Fade": no census entry left with today's switch list; the interpreter runs 0
    instructions of it on flip-1.
- New `crates/game-sim/src/handling/sound.rs`:
  - the cue's busy, and its "made playing" (list clear, push, grow);
  - the plain sound's busy and the voice's busy;
  - the plain sound's play, on a stream of one voice by its index (the path the wreck cue takes:
    stream word 1, no pair, index 0, how 4);
  - the stream start for that path, the voice index pick and the voice pick (both scan directions).
- What the port hands out (`Sounds`): the sound's stop, a play it does not port (sample, pair,
  stream by name, handed out whole), the cue's own pick (vt +0xa4), the heap's resize, the driver's
  queue flush, and the one stack word (below).
- Test file: a new switch `wreck sound` in the rider leaf's hooks. Rand goes to `rand_in_ram` (the
  `rng` port the emitters already use); the cue and sound calls go to the port, with each vtable
  asserted. The switch is added to `replay_in_pure_rust`'s list. The `…with_the_rider_leaf…` replays
  keep the old path (their switch list is unchanged).
- Addresses: research/oracle.txt `# F31b5u`.

## What the original does

Recipes: `./kit dis -p atv <addr>` for each address in oracle.txt `# F31b5u`.

- Cue busy (0x197ef0):
  - nothing playing → 0;
  - stopped word +0x30 → 2;
  - else the highest busy of its playing sounds (signed), stopping at the first that answers 2.
- Cue made playing (0x1985d0, cue, sound, a2):
  - a2 != 0 → the cue's vt +0xa4 first;
  - the playing list (+0x38) zeroed (memset count×4) and emptied;
  - the sound pushed.
  - The push grows by its step past the cap, but only up to its max (realloc with file/line).
- Sound busy (0x14ede0): handle -1 → 0, else voice busy (0x188550):
  - status half clear, or generation moved on → 0;
  - streamed voice (flag bit 0): word 1 → 2, 99 → 0, else 1;
  - else status 8 → 3, else 2.
- Play (0x14ea30):
  - stops first if +0xac is set, or if `how & 1` and busy;
  - stream (+0xa4) or sample;
  - levels from +0x40.. with `how & 4`, else from +0x34.. (copied over +0x40..);
  - handle → +0x2c; on success +0x1c = 0 and +0x7c = 1.
- Stream start (0x185d40, one voice, index ≥ 0):
  - a voice index (0x183070; -1 → -107);
  - the header from the stream table, its +4 float out (to the play's stack, unread);
  - the index entry written;
  - a voice picked from the top (0x183180; -1 → -100, index freed);
  - pan off when the driver doesn't pan or pan ∈ {0, 180, 360};
  - volume outside [-1, 100] → -111, pitch outside [-1, 0x3fff] → -113 (voice stopped, index freed);
  - left/right: `vol·pan/180` or `vol·(pan−180)/180` (32-bit product, truncating);
  - two commands queued (flush first at 63): header (7) then start (8);
  - returns `handle << 16`.
  - A trap: the header command's first word is **not** the owner word, although the source
    register says so. `lq $a3, 176($sp)` reloads `$7` just before `sw $7, 240($sp)`, so the word is
    `index << 16`. The oracle caught it on case 15.
- Voice pick (0x183180): over 2 banks × 24 slots, from the first or (a3) from the last.
  - A candidate has a clear status and a level that is not ≤ 2.5.
  - The first candidate is taken while the best is 0.0; after that, one with a higher level.
  - Stops once the best is not < 180.
  - The picked voice: status = asked<<16 | 5; generation += 1 (0 past 500); flags bits 0–4 =
    top, restart; asked, asked-too, 0, level 0.
  - `kit dis` corrects an earlier note of mine: the `f1` the "first" test compares with is set once,
    before the bank loop (the back edge goes to the instruction after it), so it is 0.0 throughout.
    Ghidra's `== 0.0` was right.
- Voice index pick (0x183070): range all / from 2 / the first 2 (two banks on, `two` set or not).
  The first free one while the best is 0.0, then a higher level. Marked used, its level zeroed.

## How it was proved

- Function oracle, `the_wreck_sounds_are_the_originals` (crates/game-sim/tests/mxvatv_bike_ride.rs):
  - 15,000 generated cases on state 4's driver, a sixth each of cue busy, cue made playing, sound
    busy, sound play, voice pick and voice index pick.
  - Inputs drawn: voice tables (statuses incl. 8, generations near 500, flags, levels about 2.5 and
    180 with ±0 and denormals, words 1/99), index tables (ties, zeros, all used), two banks,
    panning, queue near full, handles live/stale/none, every level-range edge, restart, `how`, cue
    lists that must grow, can't grow, or are stopped.
  - The stop, the cue's pick, the resize and the flush are hooked to log on both sides.
  - The answers, the calls and every word of the driver, its tables, its queue, the cue and the
    sounds match; the original writes nothing else outside its stack.
  - All 28 paths hit (the fewest: 35 started-and-flushed).
- Replays, every switch on (`replays_in_pure_rust --include-ignored`):
  - ride-2: the interpreter ran **0** instructions; `ride_replays_in_pure_rust` passes.
  - flip-1: the interpreter ran 32,092, all `the tail: out WreckTest` (F31b5t). The rider's 1,823
    are gone. `flip_replays_in_pure_rust` fails only on that total, after every row matched and the
    read watch held (no read of a byte where the two RAMs part, including row 125's stack).
  - Both stay `#[ignore]`: they are F31b5's acceptance, and flip-1 waits on F31b5t.

## Not verified / not 1:1

- **The start command's `+0x30` for a lone voice** is a word the original copies from its own stack
  where nothing on this path wrote. The port takes it from `Sounds::stack_word`, which the test
  reads at the same stack address. A Rust game without the MIPS stack has no such word. The audio
  side only reads it for a pair, so it carries no meaning; but the queue's bytes can differ from the
  original's whenever the stacks differ.
- **The original's stack writes are not made**: the stream's header float out (into the play's
  frame), the stream start's 0x28-byte block and its `sprintf` of the stream's name into a stack
  buffer. Nothing reads them back on this path. The frames' leftovers differ from the original's
  for later functions that read stale stack (the replays' read watch would show it).
- **Handed out, not ported**: the sample path (0x187850), a voice pair (0x183510), a stream by name
  (`sprintf`/`strlen`/`strcmp`), the sound's stop (0x14ed40 → 0x186ef0), the cue's vt +0xa4, the
  cue's wake (vt +0x20; C15's flip never calls it), the driver's flush (a SIF send the interpreter
  can't make). The wreck cue on C15 reaches none of these. The oracle checks the play hands those
  paths out; it does not compare them with anything.
- **Busy answers depend on the audio side**: voice status, generation and streamed words are
  written by the IOP's replies. The port reads them as RAM; in a game with no IOP they stay as the
  last image left them.
- **Only state 4's driver**: the oracle randomises the tables and voices but keeps the image's
  stream table, so a stream index past its count, or a header pointer that isn't one, are not
  drawn.

## Next

- F31b5t (WreckTest) is the last of flip-1. Once it lands, `flip_replays_in_pure_rust` should reach 0.
