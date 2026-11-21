# 1 — poof keeps knocked props (PART)

Task S475. 2026-11-21.

The user (11-21) said the earlier static finding was wrong and asked for a live test of the original. The
live test agrees with the static one: a poof leaves knocked props where they lie. There is no original call
to port, so the task blocks again on a choice, now with the live proof.

## What I did

- Played tapes from save states on copy 1 (no virtual pad here: /dev/uinput is root-only) and logged the
  bike and the five free09 BeachBall bodies every frame (research/s475: run.sh, log.py, *.pad).
- **Wreck poof** from S240's drop3-2857 (rider wrecked, balls 1, 2 and 4 awake): blank tape, 255 frames.
- **L1+R1** from F68's f68-2935 (balls 1 and 2 rolling after the strike): gas, then L1 and R1 pressed on it.
- Read the chord handler, the put-back request and the manager it tells.

## What the original does

- The wreck put-back lands at frame 2882 (the chassis jumps ~45 ft in one row). All three awake balls roll
  on through that frame unmoved, stay awake, and later sleep where they stopped (ball 1 ~3015, ball 2 ~3105).
  Nothing puts them back.
- L1+R1 and the wreck share one respawn (`./kit fn 0x0027b9f0`, P57's note in research/oracle.txt), so the
  hand poof does the same.
- In free ride the chord (`./kit fn 0x0027bef0`) puts the bike back only when the player has a
  "StuntCompetition" or "HitFreeride" activity; otherwise it restarts that player's challenge
  (`./kit fn 0x003a6dc0`) and the bike stays. That is why the f68-2935 tape did not poof.
- The put-back request tells one manager (`./kit fn 0x003a7f80`): gate/flag marks, no props.
- Free09's props come back only by each body's own ResetTime once asleep (signs 5 s, crates 30 s, balls
  60 s; the port has this), or the edge check when a body leaves its bounds.

## How it was proved

A PINE recording of the original, one row a frame: context/capture/s475/wreck.csv (255 rows) and
poof2.csv. Plus the decompiled handler and request above, and the earlier whole-ELF scan of every
put-back call (S475's first block in research/oracle.txt).

## Not verified / not 1:1

- **A sign or a crate knocked, then a poof:** not run. Knocking one needs an aimed ride (research/s195/aim.py
  aims at balls only) and there is no pad. Signs, crates and balls are the same body and collection classes
  with only ResetTime different, and the ball run shows no reset, so I expect the same.
- **A working L1+R1 poof live:** not seen. In f68-2935 the chord restarts a challenge instead (above). It
  runs the same respawn as the wreck poof, which was seen.
- **Crossing the start:** not run. Free ride on free09 has no start or lap line; the race's lap line (S13c,
  ported) has no prop call in the whole-ELF scan. No race state with knocked props exists to test it live.

## Next

The person decides (pick sheet context/notes/sheets/S475.json): drop the ask, or add the prop reset on a
poof and at the start line as a switchable addition (off in original mode, so the replays stay exact).
