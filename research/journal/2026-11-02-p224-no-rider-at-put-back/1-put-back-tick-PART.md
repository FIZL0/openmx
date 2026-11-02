# 1 — the put-back tick is not a missing rider (PART)

Task P224. 2026-11-02.

## What the phone's log says

No `Documents/openmx-log-previous.txt` came back. `~/Downloads` holds four phone logs. Only
`openmx-log (3).txt` (10-30 15:06, openmx 0.2.4, 7d4cf6b build 2366, which has P212a and P212) has
the line, three times:

- `28s … 0.0 mph (no rider drawn in 3 of 120 frames)` and `29s … (no rider drawn in 1 of 120 frames)`,
  right after `wreck: reason 11`, the page's fade to 255 and `put back`;
- `220s … (no rider drawn in 3 of 79 frames)`, right after `wreck: reason 8` and its put-back.

There are no `gpu error:` lines. There is no whole second of ` (no rider drawn)`, so the 10-28 symptom
did not happen in that launch. `openmx-log (4).txt` (10-30 15:40) has no such lines.

## Reproduced on the desktop

A headless ride (`openmx --headless --size 393x852 --draw-fps 120 --play wrecks.tape --seconds 90`,
default track, a hand-written tape: throttle held, hard steering every 5 s, which wrecks 3 times) printed
`87s … (no rider drawn in 4 of 121 frames)` after its third wreck's put-back, with
`OPENMX_RIDING_RAGDOLL=0` as the phone build had it. A temporary print on each counted frame showed
4 frames (one 30 Hz tick at 120 fps), all in phase Riding, with `drawn_ragdoll()` Some. The body's root
was drawn 25 ft from the bike. The bike stood on the put-back record, the ragdoll lay where it was
thrown, and that view's cull (P223's `ACTOR_CULL_GROWTH`) left out the body's sphere.

This is the S74 port: `Rider::ragdoll_shown` keeps the ragdoll for the tick the wreck clock puts the
rider back, because the original's body request has not seen the put-back yet. On that tick the screen is
coming back from black (alpha 255 → 171). The other two put-backs left the doll in view, so they counted
nothing. At 60 fps a run showed nothing, and that is why P212a's own check never met it.

## What I did

- openmx main.rs: the P212a counter skips any frame where seat 0's body is drawn thrown (`Shown::thrown`
  for seat 0, kept as `rider_thrown`). A ragdoll lies where it lies, so it can be out of the bike's
  view. The line is meant for a bike with nobody on it.
- plan/HUMAN.md: the step now names P224, and says the 10-30 lines were this, so any line from a newer
  build is the real thing.

## How it was checked

- The same 90 s tape after the fix: 3 put-backs, no `no rider drawn` line, with the riding ragdoll off
  and on (today's default).
- P212a's throwaway check (not committed): with the body actor given the model `"nothing"`, a 5 s ride
  printed ` (no rider drawn)` every second, so the meter still sees a missing rider.

## Not verified / not 1:1

- The 10-28 launch's cause (a bike with no rider for the whole launch) is still unknown. The only log
  from a build with the line did not show it. This waits on the user's log from a launch that does
  (plan/HUMAN.md 'A bike but no rider on the iPhone').
- Not run on the iPhone: the reproduction is the desktop's (RADV) in a portrait size at 120 fps.
- Whether the original's put-back tick really shows the ragdoll on screen is S74's finding, not
  re-checked here. The screen is mostly black on that tick either way.
