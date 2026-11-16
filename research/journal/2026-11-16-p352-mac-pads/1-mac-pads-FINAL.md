# 1 — mac pads (FINAL)

Task P352. 2026-11-16.

## What I did

- The Mac app now reads game pads. `src/win.rs` (Windows' gilrs reader, P192) is mounted on macOS too, and
  the pad layer (`src/pad.rs`) takes its `PadNode` / `pads()` / `pad_name()` there, as on Windows.
- Cargo.toml: the macOS target gets `gilrs = "0.11"` with its default features, so its IOKit HID backend.
  gilrs maps PlayStation, Xbox and Switch pads to the standard layout through its SDL mapping database.
- Windows.Gaming.Input's Sony reader stays Windows-only; on macOS `sony()` returns nothing, since gilrs'
  IOKit backend already sees those pads.
- gilrs-core's macOS backend runs its IOHIDManager on its own thread with its own CFRunLoop, so the pad
  layer can poll it from any thread, same as on Windows.
- opentennis: its pads go through Bevy's gilrs plugin, so there was no reader to copy. The js-event bridge
  is this repo's own win.rs.

## What the original does

Nothing here: the PS2 reads its pad through the pad library. This is the shell's host input only.

## How it was proved

- Cross-built `tools/macos-build.sh build --release -p openmx --bin openmx --features ship,native-sim`
  for aarch64-apple-darwin: it compiles and links.
- `./kit check --all` on Linux (Linux keeps reading `/dev/input/js*`; nothing changed there).

## Not verified / not 1:1

- No pad was tried on a real Mac: there is no Mac here. The build compiles and links, but whether macOS asks
  for Input Monitoring permission for a HID pad, and whether a DualSense / Xbox pad arrives with the right
  button layout, is unproven until someone runs the .dmg with a pad.
- Pad rumble on macOS: not wired (gilrs' macOS backend has no force feedback; the Windows build has none
  either).

## Next

- Someone with a Mac: open the .dmg, plug in a pad, check the menus and a ride respond.
