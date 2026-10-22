# 1 — copy sets pad2 (FINAL)

Task K4. 2026-10-22.

## What I did

- `./kit pcsx2 copy N` now writes `[Pad2] Type = DualShock2` and binds Pad2 to `SDL-1/…`, the same names
  as Pad1's `SDL-0/…` (`emu.PAD2_SDL`, derived from `PAD1_SDL`).
- New `_ini_set_in` / `_ini_get_in` in `ps2kit/emu.py`: set/read a key *inside one section*. The existing
  `_ini_set` edits a key wherever it first appears (fine for PINE/speed keys, which PCSX2 moves between
  sections), but `[Pad1]` and `[Pad2]` share every key name — `_ini_set('Pad2', 'Type', …)` would have
  edited Pad1's `Type`. Pad1's bindings now go through `_ini_set_in` too, so a Pad2 key above Pad1 can't
  catch them either.
- The user's own ini (`~/.config/PCSX2`) already has SDL-1 bindings under `[Pad2]` with `Type = None`;
  they are edited in place, extra keys there (`Analog = SDL-1/Guide`, motors) are left alone.

## What the original does

n/a — kit tooling, not game behaviour.

## How it was proved

- `tests/test_live.py`: "a copy plugs a DualShock2 into port 2 on SDL-1, and leaves port 1's bindings on
  SDL-0" — a user-shaped ini (Pad1 on keyboard, `[Pad2] Type = None`) is copied twice; each section holds
  its own bindings, one `[Pad2]`, the blank line before the next section kept.
- Real copy 3: `./kit pcsx2 copy 3`, booted it (PINE answered, v2.9.127), stopped it; PCSX2's own ini
  rewrite on exit kept `Type = DualShock2` and the SDL-1 bindings, Pad1 still on SDL-0.

## Not verified / not 1:1

- That a two-pad .p2m2 now rides both riders in a copy: not replayed here (no split-screen task in hand,
  and the tape lives in the main checkout). S93's hand fix in copy 4 was only `Type = DualShock2`, which is
  what this writes, so the bindings are extra; whether PCSX2 also needs them for playback is untested.
- Copies 1, 2, 4–7 still have their old ini until someone reruns `./kit pcsx2 copy N` (not mine to touch).

## Next

-
