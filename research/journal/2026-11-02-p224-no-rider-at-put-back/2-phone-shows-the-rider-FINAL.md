# 2 — the iPhone draws the rider on 0.18.0 (FINAL)

Task P224. 2026-11-21.

## What I did

The iPhone 15 Pro is on omarchy-desktop's USB, so no person was needed (user, 11-21). Driven over ssh
with iFizBox's `tools/ui-15.sh` under lock `P224`, given back at the end. Logs pulled with
`ios fsync … pull --srcPath=/Documents/openmx-log.txt` (and `openmx-log-previous.txt`).

The phone runs openMX 0.18.0 build 5260 (bd79f93). It has P212a's line, P212's `gpu error:` lines and
part 1's skip of the thrown body (736dd67 is an ancestor of bd79f93). So no new build was needed.

Two touches at once (Gas held, the stick swung) went through WDA's `/actions` with two pointers; a
one-off script in the scratchpad, not committed.

## What the logs say

- **The launch before mine** (`openmx-log-previous.txt`, 6784 lines): about 2300 s on the freeride maps,
  2106 one-second lines, 3 wrecks and put-backs, 526 s away in the background and back. No
  `no rider drawn`, no `gpu error:`.
- **My cold launch** (`openmx-log.txt`, 1063 lines): app closed through WDA and launched again, title →
  Free Ride → the freeride maps (45 s with Gas and steering), then pause → Main Menu → Free Ride →
  Cleveland (40 s). 248 one-second lines, 3 wrecks and put-backs. No `no rider drawn`, no `gpu error:`.
- Screenshots on the freeride maps (resumed, after a cold start, after 45 s) and on Cleveland (at the
  start and after 40 s) all show the rider on the bike.

## Outcome

The 10-28 symptom (a bike with nobody on it for a whole launch) did not come back in about 45 minutes of
riding on today's phone build, over a background/foreground round trip, a cold launch, a track change and
six put-backs. Its log is lost and no log since shows it, so there is no cause to fix. The meter stays in
the build: if it ever comes back, the 1 s line says ` (no rider drawn)` and the log carries it.
plan/HUMAN.md's step now says agents can pull the log off the phone themselves.

## Not verified / not 1:1

- The cause of the 10-28 launch is still unknown: it never came back, and its log was lost. Nothing was
  changed in code for it.
- Only build 5260 (0.18.0) was ridden, not today's main: the installed build already carries every
  check this task needs, and signing a new one for io.github.fizl0.openmx was not needed for that.
- Riding ragdoll, races with AI riders, split screen and online were not tried on the phone; only free
  rides on the freeride maps and Cleveland.
- The steering swings did not visibly turn the bike in the shots (the wrecks came from the ground), so
  hard-turn wrecks were not tried on purpose.
