# 1 — unplug race (FINAL)

Task K47. 2026-11-11.

## What I did

- **What the unplug raced.** A `FakePad` is a fifo, and `unplug` removes the node and then closes the
  writer, so the seats' next `drain` should read end-of-file. A fifo only reads end-of-file once *every*
  writer is closed, though, and a process forked while the test is running keeps a copy of every open fd
  until its exec. That window is tiny but wider under load. `FIFOS` already serialises the pad tests' own
  `mkfifo` spawns. `tests/online_car.rs` spawns its `openmx` shells (`Command::spawn` in its shell
  helper) without that lock, and `pad.rs` is mounted in that binary. So a shell spawned at the moment of
  `a.unplug()` held the pad's writer, `drain` read "nothing waiting", and 1P stayed `Playing`. That is
  S288's failure.
- **The fix removes the dependency on fds.** `Seats::poll` now also treats a pad whose node is gone from
  the directory (`symlink_metadata` → NotFound) as gone. This is also true of a real pad, because udev
  removes `/dev/input/jsN` when the pad goes. `unplug` removes the node synchronously before the next
  frame, so the test no longer depends on who holds a writer. Unix-only; phones, the web and Windows have
  their own pad lists.
- **The test proves it deterministically.** `pads_join_drop_and_come_back_to_their_seats` now keeps a
  second copy of A's writer open across the unplug (`FakePad::writer_copy`), which stands in for a forked
  child's copy. With the poll change taken out, the test fails exactly as S288's gate did (`left:
  Playing(...)`, `right: Waiting` at the first unplug). With it in, all 61 `pad::` tests in online_car
  pass.
- opentennis: not consulted. This is a race in this repo's fake device, not a problem it has solved.

## What the original does

Not applicable: this is test and host-pad plumbing, nothing the PS2 game has.

## How it was proved

Ran the test with and without the product change, with the writer-copy stand-in in place: it fails
without the change and passes with it (`./kit check -- -p openmx --test online_car -- pad::`).

## Not verified / not 1:1

- The load failure itself was not reproduced by racing a real spawn. I reproduced it by holding the
  writer copy explicitly, which is the state the fork leaves behind. Racing a real fork under load would
  be the flaky reproduction this task is meant to get rid of.
- The new check was not tried on a real pad being unplugged. On real hardware the kernel's ENODEV still
  reports the pad gone as before, and the node check only adds a second way to notice.

## Next

Nothing.
