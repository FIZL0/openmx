# 1 — atv clutch writers (FINAL)

Task MD178a. 2026-11-17.

## What I did

- Read both writers of the quad's clutch lever that MD178 left (research/oracle.txt `# MD178a`).
- Ported the vehicle update's states-3/4 clear on the quad: `Ride::remote_let_go` in
  `crates/game-sim/src/quad.rs`. With the vehicle state's mode 3 or 4 it zeroes the stick, the brake
  and the clutch lever. `Ride::tail_on` and `Ride::tail_on_untamed` call it after the tail's update.
- Did not port the gate clutch hold on the quad: it is the AI driver's habits step, and no AI drives a
  quad in the port (see below).
- The shell's quad is always driven (mode 1), so nothing it rides changes: every quad replay hash
  stays as it was.
- Proof: `crates/game-sim/tests/atv_quad/remote_let_go.rs`, mounted in `tests/atv_quad.rs`.
- opentennis: nothing to take (no vehicles, no remote states).

## What the original does

- **The vehicle update** (`./kit fn 0x004e1b90 -o atv`, the quad's update calls it first). With a
  vehicle state and the still camera's skip bit clear, it runs the ghost step, then reads the state
  again. Kind 3 or 4 (a remote vehicle): stick x and y, the brake and the clutch lever are zeroed, then
  the listeners, the node, the stunt clocks and the remote wreck setter run. Any other kind: the wreck
  test and the stunt idle. The port already had this for the bike (`handling/tail.rs` `inner`, its
  `cleared` words); the quad's tail only handed the call on.
- **The gate clutch** (the stores near 0x004e1538, `./kit fn 0x004e1460 -o atv`). Ghidra splits it off
  as its own function, but `./kit dis` shows it is the tail of one function from 0x004e0f50 to
  0x004e1870 (no return between). That function reads the driver object at +0x48c and its +0x38 bits
  exactly as MXU's AI habits do (`ai.rs` `habits`: 8 clutch, 0x10 clutched, 0x20 gate clutch, the gate
  clock against the delay, the cross-up roll at 0x400). The bit-8 branch writes 1.0 to the lever, else 0.
  The quad's vtable holds it at +0x254 through a thunk that takes 0x48c off a0.

## How it was proved

- Function oracle, 10,000 generated cases: the vehicle update run in the interpreter on
  `context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`, on the image's own quad and vtable, every call it makes
  stubbed.
- The bike is in a0 (Ghidra's listing shows it as the second parameter: wrong). Words carried raw into
  the port (`F::from_bits` normalises a NaN word the update leaves).
- Each case: no state, or kind 0, 1, 2, 3, 4, 5 or a random word; the four words (stick x, y, brake,
  lever) random (0, -0, 1.0, part presses, any bits). The four words after must equal the port's
  `Bike::stick`, `Quad::brake`, `Bike::clutch` after `remote_let_go`, bit for bit. The remote branch
  must run (its listeners' step called) exactly when the port clears.
- Every path (no state, remote 3, remote 4, driven, another kind) taken at least 500 times.
- Mutant check: dropping the lever's clear fails at case 4 (kind 3). Restored.
- A held rider: the port leaves the lever (the quad's update oracle shows the vehicle update is not
  called then).

## Not verified / not 1:1

- **The gate clutch hold is not ported on the quad.** It is the AI driver's step; the port has no AI
  quad (AI riders ride MXU bikes, whose twin `ai.rs` already ports). "AI only" is by the twin, not
  traced: I did not find who calls the quad's vtable slot +0x254.
- **The states-3/4 clear is never reached in the shell.** The shell's quad is always mode 1; netplay
  runs every seat's sim locally, so no quad is remote. The port is proved against the function, not
  on a ride.
- **The still camera's skip bit** (which skips the clear in the original) is not modelled on the quad:
  the port's quad has no still camera. The oracle keeps it clear.
- **The ghost step may change the vehicle state** before the original reads it again; the port has
  no ghost step on the quad, so it reads the tail's mode once.
- **Untamed's twin** (MD93c) also works out the stick's angle and length again after the clear (for a
  zeroed stick: the arc cosine table's middle and 0). On the quad's Untamed step the port only zeroes
  the three words: which quad words Untamed's angle and length land in was not checked.
- **The next frame's pad read** overwrites the three words in the port (`Ride::step` lays the
  controls). Whether the original's pad reader runs for a remote quad (its live test) was not checked;
  it does not matter while no quad is remote.

## Next

Done when `./kit done` lands.
