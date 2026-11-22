# 1 — decisions (FINAL)

Task S496. 2026-11-21. Follows research/journal/2026-11-21-s496-lift-pilot/1-lift-pilot-READY.md.

## What I did

The user answered the pilot's pick sheet, all four on the recommendation:

- **Offsets:** yes. AGENT.md now has one line: in `crates/game-lift` only, the original's struct offsets
  (`ai + 0x1e0`) are allowed; addresses and decompiler names are not.
- **Vtables:** decide when the first lift that calls through a vtable comes up. Nothing changed.
- **Go on lifting:** land the pilot as it is. Lift new functions only where no port exists yet. No new
  lift task filed: no current task asks for a function with no port.
- **`--ai N`:** keep it, off by default. It already is (`crates/openmx/src/main.rs`, the `--ai` arm).

The pilot's code (game-lift, the bridge, `--ai N`) was already on main from the first session.

## What the original does

Nothing new; see the pilot entry.

## How it was proved

No new code. `./kit audit` passes with game-lift as it is; the gate runs at `./kit done`.

## Not verified / not 1:1

Everything in the pilot entry's list still holds, unchanged:
research/journal/2026-11-21-s496-lift-pilot/1-lift-pilot-READY.md. In short: no gate test repeats the
`--ai 1` lap; the gate's drop, route follower, avoid and an arena-resident bike are not lifted.

## Next

None for S496. Future lifts: only for functions with no port, per the user's pick.
