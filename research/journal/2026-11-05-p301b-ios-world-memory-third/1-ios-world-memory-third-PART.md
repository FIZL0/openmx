# 1 — ios world memory third (PART)

Task P301b. 2026-11-05.

## What I did

- Looked for the phone log this task is defined by. There are four in `~/Downloads/` (`openmx-log.txt`,
  `openmx-log-2.txt`, `openmx-log (3).txt` and `(4)`). The newest is from 2026-10-30, and context/ has none.
  All four are from builds before P301: their memory line is the old `openmx … on iOS, memory N MB, N MB
  left before iOS closes the app`. They have no `load: … memory peak` line and no seconds'
  `, memory N MB (N MB left, GPU N MB)`. Their `pages 128x128` comes from 0.2.x builds, before the
  lean ground, so it does not show P301a.
- Cut nothing. P301a already made the cuts that the desktop's breakdown points to (half-size pages,
  no finest detail level, vertex cap). Without the phone's GPU and RSS split, a further cut would be a
  guess: the CPU-side world (tiles, props, joins) or the GPU. A wrong cut makes the picture worse and
  nobody can measure it. Blocked on the log. HUMAN.md's "What a world ride holds on the iPhone (P301,
  P301a)" already asks for it, so I added nothing there.

## What the original does

Not applicable: this is the app's memory on a phone, not game logic.

## How it was proved

Nothing to prove: no change.

## Not verified / not 1:1

- **Whether a world ride stays under ~2 GiB on an iPhone 15 Pro is still unknown.** There is no device
  here and no log with P301's lines, so nothing in this step was measured on a phone.

## Next

Human: run HUMAN.md's P301 steps on a build at or after 0.11.0 (it has P301 and P301a), and send
`Documents/openmx-log.txt` into context/. Then the next session should read the `load:` line's GPU
share against the rest. If the GPU part is large, cut the GPU side again (ASTC pages, a shorter
reach). If RSS is large (desktop ~1.4 GB on world_ultra), cut the CPU-side world: tiles, props, joins.
