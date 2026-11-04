# 1 — rider writes into the bike

Task F41. 2026-11-04.

## What I did

- `crates/game-sim/tests/mxvatv_bike_ride.rs`: `the_riders_writes_into_the_bike_are_read_by_the_wreck_clock_alone`,
  through `riders_writes_read` and `RiderReads`. Both C15 replays run whole in the interpreter (as
  `replay` does, still checked bit-exact on every row). The rider's step is hooked to run as itself
  between two read watches (`Ee::reads`, `Ee::written`). Every byte the rider's step writes inside the
  two windows (taken from `OTHER_LEAVES`' rider entry) is marked. A read of a marked byte by anything
  else in that frame or any later one, before something else writes it again, is recorded with its row.
  The rider's own reads of marked bytes are counted apart.
- research/oracle.txt `# F41`: the writer and the flag's source, with addresses.
- Follow-up S278: the flag that the bike's frame does read.
- Nothing taken from opentennis: it has no counterpart (a read watch over an interpreter replay).
- No product code changed. The task's choice was "port it, or prove the bike's frame never reads those
  words". Every word but one is now proved unread. The one that is read (+0x920) cannot be ported without
  the rider's ragdoll world, which is a task of its own (S278), not a follow-up step.

## What the original does

The rider's step (`./kit fn 0x004f2ad0 -p atv`) first runs the rider's frame `0x004f1790`. That frame
runs the ragdoll pass `0x004ef320`, which sets the rider's +0x37c to 1 if any part in its list (+0x5b4,
count +0x5b0) has a nonzero +0x14, and to 0 otherwise. It also adds dt to +0x380 while the flag is set
and clears it when not. Then comes the rider's physics world `0x00505d50` -> `0x00496b70`. During that
world step the rider's vtable slot at `0x003d12f8` (`0x004f28f0`) runs `0x004f1210`, which copies into
the target record (the bike's, at bike+0x730):

- the flag to record +0x1f0, which is the bike's +0x920;
- a quad from rider +0x670 (or (0,0,0,1) with the flag off) to record +0x160, the bike's +0x890;
- a node's +0x70 quad to record +0x200, the bike's +0x930.

On a wreck (record +0x138 != 0) it also copies two node frames to record +0x220/+0x230/+0x240. Back in
`0x004f2ad0`, on the wreck, comes the camera block (record +0x140, +0x150). F39 credited all of this to
the world step. The physics world is in fact only the caller of the vtable slot that does the copying.

The port's wreck clock (`handling::wreck::clock`) starts the put-back count only when `SETTLES` (+0x920)
is nonzero: the bike is put back on its wheels only once the thrown rider's ragdoll touches something.

## How it was proved

- `the_riders_writes_into_the_bike_are_read_by_the_wreck_clock_alone`, on both replays:
  - ride-2: 459 frames and 459 rider steps. 18 360 bytes marked (40 a frame). No read by anything else.
    The rider reads none back.
  - flip-1: 465 frames. 24 040 bytes marked. 1 072 read back by the rider itself. 64 reads by the frame,
    all of the four bytes of +0x920, on rows 125..=140. That is the wreck row up to the row where the
    clock starts counting; after that the clock's `COUNTING` short-circuits the read.
  - Neither replay has anything but the rider write a marked byte again.
  - The test asserts the read set is exactly `{SETTLES}` on rows 125..=140 for the flip, and empty for
    the ride.
- The +0x920 read is itself the positive control: the watch does see reads of marked bytes, so the
  empty result for every other byte is a real result, not a watch that never fires.
- The writer was found with a temporary write watch in `ps2-oracle`'s `Ee::write`, printing pc and ra for
  writes to bike+0x920 and then to rider+0x37c. It was reverted and is not committed. bike+0x920 is
  written 465 times, all at pc 0x4f1228 (in `0x004f1210`): 0 on 128 frames, then 1 on 337. rider+0x37c
  is written in `0x004ef320` (pc 0x4f0004 sets it to 1, pc 0x4f0018 to 0).
- `./kit check -- -p game-sim --test mxvatv_bike_ride -- the_riders_writes`: passed (48 s).

## Not verified / not 1:1

- **The bike's +0x920 is read**, by the wreck put-back clock, and the port does not write it. The port's
  wreck clock is right only as long as something gives it the rider's ragdoll-contact flag. On the
  replays the original rider supplies it. The port has no rider ragdoll for MX vs. ATV, so the word is
  not ported. Filed as S278.
- Every result covers only the two C15 replays (ride-2, flip-1): one wreck of one kind (the flip,
  thrown front). A different wreck, a put-back taken through the tail's own put-back, or another state
  could read other words of the windows. Nothing here proves "never" beyond those 924 frames.
- The read watch is per byte at the granularity of the watch intervals (frame start to the rider's
  step, and the rider's step to the next one). A byte written and then read inside one interval by
  someone else would count as a read. None was seen, so this only ever errs towards failing.
- The rider's own reads back (1 072 bytes on the flip) are counted but not attributed. Only the rider
  depends on them, and the rider is not the port's.
- The camera-block words (+0x870..+0x8a0) and the frame copies (+0x950..+0x97c) are unread on these
  replays. Whether code outside the sim frame `0x00204040`, such as the camera, reads them was not looked at.
  The camera is presentation, not the sim.

## Next

None for F41. S278 holds the one word the bike's frame does read.
