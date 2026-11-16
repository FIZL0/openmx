# 1 — draw the race's AI riders

Task V128. 2026-11-16.

## What I did

- main.rs: a new `AiRig` per AI rider of a one-player race (players 1.. of seat 0's Sim, P313a): its
  own `Body` and `Chassis`, loaded as a seat's are (same rider archive, bike archive, trick-pack clips,
  fork stop).
- Each frame the seat 0 rider is posed (offline, bike up, moving), `ai_ride` poses every AI rider the
  same way: `Body::ride` off its own rider, the riding ragdoll's skeleton and drawn nodes, then
  `Chassis::ride` with the same want/request as a seat, `bare`, and the riding ragdoll's holds.
  - The controls handed to `Body::ride` are `ai::ride_controls` off the pilot's levers as they stand.
- Drawing: `ai_shown` builds a `Shown` per AI rider (split out of `Shown::of` as `Shown::of_rider`,
  with `riding_bones_of` and `ride_holds_of` beside the seat versions). `ai_actors` pushes the bike,
  its parts and the body, in seat 0's models, posed by the AI rider's own node matrices. Blended
  between the last two ticks like a seat's (`before_ai`), cut on `drawn_jumps`.
- Took nothing from opentennis: its other players are seats, not AI riders on a shared model.

## How it was checked

- `openmx --headless --track sx01 --race --racers 4 --frames 40 --shot …`: the two AI riders beside
  the human are drawn on their bikes at the gate, posed and shaded like the human's rider.
- Same at frame 145 (`--shot-frame 145`), after the drop at tick 97: still drawn, posed, no panic.
- `./kit check -- -p openmx --test race_menu`: 3 passed (gate, lanes and drop lines unchanged).

## Not verified / not 1:1

- **Rendering is by eye only**, as AGENT.md says for visuals. No comparison against the original's
  race picture.
- **Every AI rider wears the human's rider model, gear and bike paint.** The roster's identities
  don't reach the lane yet (P313a/S331), so there is no per-AI rider or bike to load.
- **GPU path only.** The software renderer skins into one model per rider; the AI riders are not
  drawn there (a line says so at load). The software path is a fallback.
- **The body's controls are a tick late.** `Body::ride` gets the controls of the levers after the
  brain's think, while the bike's frame rode the ones before it (`Sim::think`). Only the body's stick
  lean reads them; a tick of lag is not visible. Not stored in the Sim, so not fixable from the shell.
- **The body machine, riding ragdoll skeleton and holds now run on the AI riders**, as the shell
  already does for a seat's rider offline. That writes rider state (body machine, holds a wreck is
  thrown from) that was untouched before. Same as the human; not proved against a capture of an AI
  wreck.
- **No wheel blur copies** on the AI bikes (the seats get V59's), and they are not in the
  `--trace-picture` lines.
- **Online, nothing is posed** (one-player races only have AI riders, and online rides pose inside the
  session).
- **They don't ride off after the drop** — not this task: S331b/S331c (race driver input).
- **A wrapping world's seam carry** is not applied to the AI riders' blend. Races are on tracks.
