# 1 — phone log after p342 (READY)

Task P348. 2026-11-16.

## What I did

- Looked for a phone log from a build with P342 (landed 2026-11-14 03:49). None exists.
- `~/Downloads/phone-*-openmx-log.txt`: the newest are from 2026-11-13. Their first line names
  0.13.0 build e983cfc (4179), before P342. They have no `video:` line and no `uploaded in the draw`.
- `context/` has nothing newer that is a phone log (only slot, gate and PCSX2 logs).
- The task cannot go further without one. It needs the user to ride world_mxu on the iPhone
  and send the log.

## What the original does

Not applicable: this is about the phone app's frame times, not the original's behaviour.

## How it was proved

Nothing proved yet. No log to read.

## Not verified / not 1:1

- What the present-led frames over ~50 ms spend their time on: no phone log from a build with P342.
- Whether back-from-the-switcher frames stay near the 4 ms upload budget: same reason.
- Whether the `video:` line reads auto or chosen rows on the phone: same reason.

## Next

The human must: install a build with P342 (any build from 2026-11-14 on) on the iPhone, ride
world_mxu for a few minutes (several joins, one jump to a marker, one trip to the app switcher), and
send `openmx-log.txt` (LocalSend lands it in `~/Downloads`). Then the next session: check the first
line names a build after ecd76633f, grep `video:`, the per-second present split (upload / submit /
wait), and `uploaded in the draw`; name the leading stage of each frame over ~50 ms.
