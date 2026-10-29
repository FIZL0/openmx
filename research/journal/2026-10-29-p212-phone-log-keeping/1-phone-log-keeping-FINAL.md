# 1 — phone log keeping (FINAL)

Task P212. 2026-10-29.

## What I did

The two items P212's line left open after f302ce4b (the controller hand-over):

- **The launch before's log is kept.** `openmx_ios::log_to` opened `Documents/openmx-log.txt` with
  `File::create`, so the launch the user saw "a bike but no rider and no controls" in was lost the moment
  they opened the app again to fetch the log (the log in research/p212 is a later launch that rides fine).
  It now first renames the old log to `openmx-log-previous.txt` (`crates/openmx-ios/src/log.rs`:
  `previous_log`, `keep_previous_log`, one launch deep). That module is built on every target, unlike
  `sys.rs`, so the desktop tests cover it.
- **GPU errors go to the log.** `ios_gpu.rs` now hands the device an `on_uncaptured_error` handler, as
  `web_gpu.rs` does for the page, that prints `gpu error: …` into the log. It writes the first 8, then a
  single "more than 8" line, since a broken pipeline raises the same error every frame. Before this, a
  Metal validation or pipeline failure (e.g. the rider's skinned pipeline being refused) would only have
  shown up as something missing from the picture.
- Filed **P212a**: a stats-line suffix or log line when the ride draws no rider. It needs a per-frame
  triangle count for the rider in the GPU path, which the shell doesn't keep yet.

opentennis: hst-base's `ios.rs` has the same File::create log (M105) and doesn't keep a previous one, so
there was nothing to take. The idea of an error handler comes from this repo's own web_gpu.rs.

## What the original does

Not applicable: this is the app's shell, not game logic.

## How it was proved

- `crates/openmx-ios/src/log.rs` tests: the name (`openmx-log.txt` → `openmx-log-previous.txt`, and a
  name with no extension), and three launches in a temp dir: the first finds nothing to keep, and later
  ones keep exactly the launch before's text (`./kit check -- -p openmx-ios`: 7 passed).
- `tools/ios-build.sh check -p openmx --bin openmx` (aarch64-apple-ios, rustup 1.99.1) builds with no
  errors and no new warnings in the files I changed.

## Not verified / not 1:1

- **Not run on a phone.** There's no iPhone attached and nobody to install the IPA in this unattended run.
  So the rename in a real Documents folder, Files showing `openmx-log-previous.txt`, and a real Metal
  error reaching the log are all unproven on a device. Only the desktop test of the same code and the iOS
  build back them up.
- **The no-rider / no-controls state itself is still not found.** Its log is gone, and the log we have
  rides normally (Riding throughout, touch pad as 1P from 1 s). Candidates are still the brief's list:
  rider pipeline failure on Metal, a touch overlay hidden while input works, or a seat that never got the
  touch pad. The next phone launch's `openmx-log-previous.txt` and its `gpu error:` lines are what tell
  them apart. The "no rider drawn" line is P212a.
- **Install the IPA and ride it with the touch pad and a pad** (the brief's first clause) was not done
  here, for the same reason: no phone.
- `rename` over an existing previous log replaces it, which is atomic on APFS. I didn't check that on a
  device.

## Next

On the phone: install a build with this, reproduce the no-rider launch, then open the app again and read
`openmx-log-previous.txt` for `gpu error:` lines and the `1P:` seat line. Then P212a.
