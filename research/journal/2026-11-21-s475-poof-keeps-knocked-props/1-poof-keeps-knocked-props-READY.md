# 1 — poof keeps knocked props (READY)

Task S475. 2026-11-21.

The user (11-21) said the earlier static finding was wrong and asked for a live test of the original. The
live test agrees with the static one: a poof leaves knocked props where they lie. There is no original call
to port, so the task blocks again on a choice, now with the live proof.

## What I did

- Played tapes from save states on copy 1 (no virtual pad here: /dev/uinput is root-only) and logged the
  bike and the five free09 BeachBall bodies every frame (research/s475: run.sh, log.py, *.pad).
- **Wreck poof** from S240's drop3-2857 (rider wrecked, balls 1, 2 and 4 awake): blank tape, 255 frames.
- **Settled props, poof:** the same state and tape again. At frame 2865, log.py moved the two balls that were
  asleep (0 and 3, never woken) 10 ft along x by PINE. That gives props off their spots with nothing left
  to move them, so only a reset could put them back.
- **L1+R1** from F68's f68-2935 (balls 1 and 2 rolling after the strike): gas, then L1 and R1 pressed on it.
- Read the chord handler, the put-back request and the manager it tells.

## What the original does

- The wreck put-back lands at frame 2882 (the chassis jumps ~45 ft in one row). All three awake balls roll
  on through that frame unmoved, stay awake, and later sleep where they stopped (ball 1 ~3015, ball 2 ~3105).
  Nothing puts them back.
- The moved asleep balls stay 10 ft off their spots through the put-back (frame 2882, as before). They
  are still there at the last row, 3056, about 6 s later. So no reset touches settled props either.
  There was no writer, so a write watch had nothing to find.
- L1+R1 and the wreck share one respawn (`./kit fn 0x0027b9f0`, P57's note in research/oracle.txt), so the
  hand poof does the same.
- In free ride the chord (`./kit fn 0x0027bef0`) puts the bike back only when the player has a
  "StuntCompetition" or "HitFreeride" activity; otherwise it restarts that player's challenge
  (`./kit fn 0x003a6dc0`) and the bike stays. That is why the f68-2935 tape did not poof.
- The put-back request tells one manager (`./kit fn 0x003a7f80`): gate/flag marks, no props.
- Free09's props come back only by each body's own ResetTime once asleep (signs 5 s, crates 30 s, balls
  60 s; the port has this), or the edge check when a body leaves its bounds.

## How it was proved

PINE recordings of the original, one row a frame: context/capture/s475/wreck.csv (255 rows), poke.csv
(200 rows, asleep balls moved at 2865) and poof2.csv. Remake poke.csv with
`./kit pcsx2 with -- bash research/s475/run.sh PID research/s475/wait.pad context/s240/drop3-2857.p2s OUT 200 2865`. Plus the decompiled handler and request above, and the earlier whole-ELF scan of every
put-back call (S475's first block in research/oracle.txt).

## Not verified / not 1:1

- **A sign or a crate knocked, then a poof:** not run. Knocking one needs an aimed ride (research/s195/aim.py
  aims at balls only) and there is no pad. Signs, crates and balls are the same body and collection classes
  with only ResetTime different, and the ball run shows no reset, so I expect the same.
- **A working L1+R1 poof live:** not seen. In f68-2935 the chord restarts a challenge instead (above). It
  runs the same respawn as the wreck poof, which was seen.
- **Crossing the start:** not run. Free ride on free09 has no start or lap line. The race's lap line (S13c,
  ported) has no prop call in the whole-ELF scan. No race state with knocked props exists to test it live.
  The one whole-track prop reset is the score manager's "took the lead" (rank goes to 1). In a race that
  can happen near the start, and may be what was remembered. It needs a race with more than one rider,
  and the port keeps it.
- **The poke moves only the body's position**, not any copy in the scene node. A reset to placed poses
  writes the body, so it would still have shown.

## Next

The person decides (pick sheet context/notes/sheets/S475.json): drop the ask, or add the prop reset on a
poof and at the start line as a switchable addition (off in original mode, so the replays stay exact).
