# 1 — mesh walk setup globals

Task MD142. 2026-11-17. Slot s4.

## What I did

MD118 wrote that the original sets up the mesh tree walk's globals only once on the palm ride (frame
81), while the port writes them on each of its 277 kind-0 part pairs. That was wrong. The original
has no guard: it writes them on every walk setup, 1342 times on the ride, and the port's count per
frame is the same.

- **`record_outside.rs`:** the port still writes the four globals on every mesh pair. The comment now
  says the write has no guard. A thread-local mutant (`WALK_SPOIL`) can skip the next setup's four
  writes, or set one of them wrong.
- **`sim_frame.rs`, the palm chain:** with the port's world step, both machines log their stores each
  frame. The stores to the walk's mode global must be equal in number on the original and on the
  twin, every frame (the twin's count is the original code it still runs plus the port's Rust).
  `the_palm_strikes_contacts_push_the_ports_quad` also checks that there are more setups than the
  port's mesh pairs, so the pairs are only part of the total.
- **New test `a_spoiled_walk_setup_goes_red`:** the port's first mesh pair (frame 81) is spoiled five
  ways: all four writes skipped, the mode 4 over, the descriptor 4 over, and each tree pointer set to 0
  (off the stack). Each run goes red on frame 81, with exactly the spoiled words listed (all four
  when skipped).

Nothing was taken from opentennis: it has no mesh tree walk.

## What the original does

- The part pair (vt `+0xb4`, `./kit dis 0x001bbe60`, which jumps to 0x001bc880) calls the mesh
  descent for a kind-0 mesh at 0x001bc974. The only branch before that call is the profiler check
  (`+0x188`), and it rejoins before the call.
- The descent (inside `./kit fn 0x00108340 -o atv`; the setup is at 0x00108820) stores, with no
  branch before them: the descriptor `0x3dce38` (pc 0x10882c), the mode `0x3dce30` (0x108834, from
  `$t2` = 1), and the two tree pointers `0x3dce40`/`0x3dce48` (0x108964/0x108968, from `a2`/`a3`,
  the caller's `sp+0x158`/`sp+0x150`).
- On the palm ride the descent runs 1342 times, first on frame 81. In the twin, 1065 of them run
  from original code that is still the original's, and 277 come from the port's part pairs, frame by
  frame. Two more stores write `0x3dce38` alone (pc 0x1095c4, 0x10e574); they are not part of this
  setup.
- Why MD118 saw "once" (likely, its watch is gone): frame 81 is the first frame with a setup, and
  its one setup is the port's part pair. From frame 82 on, original code in the twin also writes
  the globals each frame, so leaving out the port's writes would no longer show.

## How it was proved

- A temporary log of the interpreter's stores (`Ee::log`) on both machines through the 300-frame palm
  ride: the original made 1342 setups; the twin made 1065 + 277 Rust ones; per-frame counts were equal
  on all 300 frames. The log is now a check in the palm chain.
- Mutants: `a_spoiled_walk_setup_goes_red`, 5 mutants, each red on frame 81 on the spoiled words.
- The C28 replays run by the port (`replay::s_*_replays_by_the_port`) go through the same chain, so
  the per-frame count is checked on them too.
- `./kit check -- -p game-sim --test atv_quad -- frame::sim_frame`: 40 tests pass.

## Not verified / not 1:1

- **The tree pointers are still "any stack address".** A tree pointer spoiled to another stack
  address would not go red: the comparison treats all stack addresses as equal, because the two
  machines' call depths differ (MD118's rule).
- **The per-frame count checks only the mode global.** The other three are written by the same
  straight run of code, so their counts follow. The two other writers of `0x3dce38` (0x1095c4,
  0x10e574) are original code in both machines and are not counted.
- **What the 1065 setups the twin still runs as the original's belong to** (which callers) was not
  traced. They are the same on both machines, so they do not affect this proof.
- **The interpreter is the oracle, not PCSX2.** Only the palm ride on Desert Oasis is covered.

## Next

none
