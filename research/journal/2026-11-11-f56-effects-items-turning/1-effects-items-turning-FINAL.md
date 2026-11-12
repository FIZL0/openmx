# 1 — effects items turning (FINAL)

Task F56. 2026-11-11.

## What I did

- **`game_sim::handling::effects`** gets the effects items' turning virtuals: `on` (vt `+0x18`), `off`
  (`+0x1c`), `key` (`+8`), and the `pause` (`+0x20`) and `unpause` (`+0x24`) that a key reaches. They
  dispatch on the item's `Class`, which the caller reads off the item's vtable through the new `Items`
  trait. The seven vtables state 4 holds come down to four behaviours:
  - `Clocked` (1 vtable): on only sets the bit. Off and pause halt the object and reset its clock and count.
  - `Lit` (3 vtables): light and douse the object. A key does nothing.
  - `Picked` (2 vtables): like Lit, but a key hands its pick to the object, and off/pause clear `+0x28`.
  - `Latched` (1 vtable): like Picked, but pause clears nothing and off clears `+0x90` after the douse.
  What an item hands its object (`Call::Light`, `Douse`, `Halt`, `Pick`) goes out to the caller. Those are
  the emitter objects' own calls (F57/F59).
- **`PartWalk`** (tests/mxvatv_bike_ride.rs) answers `Out::On`, `Out::Off` and `Out::Key` with the port,
  through a new `effects::Items` impl. Classes come from `FX_CLASSES` (vtable → class), and an unknown
  vtable panics. The objects' calls are the original's, run through `PartWalk::original` (so they are
  watched for bike writes and booked as `effects item object +<slot> <fn>`). Steps (`+0x14`) are still the
  original's (that is F58).
- `walk_served` now asserts that no `+0x8`, `+0x18` or `+0x1c` item virtual is handed on, and the flip-1
  every-port test asserts that the port's turning reached the objects.
- Nothing from opentennis: it has no effect items.

## What the original does

research/oracle.txt `# F56` has every class's five virtuals clause by clause (`./kit dis <addr> -p atv`;
none of them is a Ghidra function). Points worth knowing:
- The state byte is stored, then read back before it is tested.
- The object pointer is read again after a pick, because the unpause loads `+0x14` again.
- A Picked/Latched key calls the unpause with `a1` holding whatever the pick left behind. No unpause reads it.

## How it was proved

- **Oracle `the_effects_items_turning_is_the_originals`, 20,000 cases.** Each case is an item in scratch RAM
  with one of the seven real vtables, junk words, a state byte of 0–3 or junk, and picks that mix 0xff with
  junk. Each runs one of the five virtuals through SLUS-21104 (the objects' `+0x84`/`+0xa4`/`+0xa8` at fake
  hooked addresses, the pick function hooked) and through the port. The hooks change the item (halt flips
  `+0x48`, douse sets `+0x28` and `+0x90`, pick moves `+0x14` to a second object), so the port's order and
  re-reads are tested. The object calls match in order and arguments, and the item's and objects' RAM
  matches. All 31 class × virtual × call-pattern paths came up (a temporary probe printed them). A mutant
  (Latched off clearing `+0x90` before the douse) fails it.
- **`flip_replays_with_every_port_at_once`** and **`ride_replays_with_every_port_at_once`**, with the
  turning in Rust: C15 word for word, and RAM the same as the handing run's (flip-1 apart from the cue's
  word, as before). On flip-1 there are still 4,463 item calls (the same as F31b5n). The object calls the
  port handed on were 35 halts, 594 lights, 779 douses and 360 picks.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The objects' calls are still the interpreter's.** Light, douse, halt and pick are the emitters' code
  (F57, F59). The item steps (`+0x14`) are also still the interpreter's (F58). So the effects items are
  not yet pure Rust in the shipped game, and nothing outside tests calls `effects::on/off/key`.
- **The class map is test-side.** The vtable → `Class` mapping lives in tests/mxvatv_bike_ride.rs
  (`FX_CLASSES`). The shipped frame (F31b5) still has to give items a class.
- **One vtable is not classed.** `0x003d18d0` shares the Clocked on, but no item turned on flip-1 or
  ride-2 has it, so its other slots were not read. The test panics if it ever shows up.
- **The PC exe was not read.** research/mxvatv-pc.md has no notes on these classes. The PS2 code is 4–20
  instructions per virtual, read in full, and the oracle runs against the PS2 ELF.

## Next

Nothing for F56. The objects' calls go with F57/F59, and the steps with F58.
