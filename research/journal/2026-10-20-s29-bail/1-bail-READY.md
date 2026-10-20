# 1 — bail (READY)

Task S29. 2026-10-20.

## What I did

Looked for the original's bail, as the brief asked, before porting anything. **There is none.** MX
Unleashed has no button, chord or stick gesture that puts a riding rider down on purpose, and nothing in
recording2 is one. Nothing was ported, because there is nothing to port: a bail in this port would be an
invented behaviour, and whether to add one (as an addition behind a flag that is off by default, the way
P17 added its flat spin) is the user's call. The task is blocked on that question.

The things the search turned up that *are* real are written down below and opened as two tasks: reason 6
has no port at all (**S37**), and the original's reset chord is L1+R1, where the port's is L3+R3
(**P20**).

No C6 capture was in `context/capture/acceptance/` here. P15's slot (s3) had already captured recording2 off
the original with `watch/acceptance.toml`'s columns (1,894 rows, 30 Hz, frame_counter 2374–4267, no gaps,
no dropped frames, not suspect). That has every column this task needs (the sixteen controls, both libpad
halves and `wreck_reason`), so I copied it into this slot as `context/capture/s29/recording2-freeride.csv`
rather than record it again. It is in a subfolder of its own so the older readers stay green. C6 still owns
the full index.

## What the original does

**Every wreck entry, enumerated.** The rider's wreck entry is vtable slot +0xfc (`./kit fn 0x252ca0`, the
player's override `./kit fn 0x27b810`, which is its only direct caller). A search of the decompilation for
every call through a slot +0xfc finds rider wreck entries in two functions only. The rest are UI widget
classes that happen to share the slot index:

| reason | where | what triggers it |
| --- | --- | --- |
| 13 | the wreck test, `./kit fn 0x2522c0`, first block | the bike's +0x570 is set and a proximity query (radius 150) finds something: the cannonball, with its three `rand` lanes. **Not a bail.** It is a contact, and S26 already knows it |
| 4, 1(?), 2, 3 | the wreck test, attitude block | pitch and roll past the limits |
| 5, 1, 8, 0xb, 0xc, 7, 2/3 | the wreck test, landing block | the hard landing, the attitude at contact, the crossup, the trick's own clause. Reason 7 here is the **throw-away** taken past 95% (S11 ported it, and on the retail disc only a backflip reaches it), and 0xc is the no-hander or a trick landed unsafe |
| 6, 7 | the wreck test, contact-list loop | a contact over 6 (4 with +0x588 set). **6** when nothing is on the other side (scenery), 7 when it is another actor (joust) |
| 5 | the wreck test, end | a contact flag against +0x398 |
| 14 | the ride state machine, `./kit fn 0x27bc90` | a bike that is neither a local human's nor on an AI route is put down at once |

None of these reads the pad. The only pad-driven path anywhere near the wreck is the **hand reset**
(`./kit fn 0x27bef0`). It reads bindings 0x4c and 0x4d, is gated on no AI route, its own latch, the mode
and the reason not being 13, and asks `./kit fn 0x27b9f0` for kind 1 with no page, which respawns on the
spot. It never enters a wreck; it ends one, or re-places a riding rider. `./kit data --strings '(?i)bail'`
finds nothing in the executable either: there is no bail string, binding name or HUD text.

**The rider file's throws.** `[Wrecks]` names seven throw groups (front, side, back, bottom, fell off the
front, fell off the back, stunt). None of them is voluntary. So the brief's last question, whether a bail
would land on a throw the file names, has no answer, because there is no bail. The nearest thing, the
throw-away's reason 7, lands on `Hit::Stunt` in `ride::hit_of`, which is still the port's own inference.

## How it was proved

**recording2's three wrecks, read off the capture's own columns**, with press edges taken from the
sixteen `ctl*.held` columns (the game's controls, wire bit N = control N per S8's test):

| entry row | reason | press edges in the 60 ticks before | length | how it ended |
| --- | --- | --- | --- | --- |
| 641 | 8 (nose-dive) | square down and cross up at −40, R2 tapped at −38/−37, then nothing | **124** | by itself |
| 1188 | 12 (trick / no-hander) | cross up −46, circle −43…−31, cross down −20, then nothing | **24** | L1 at +23, R1 at +24, and the reason is 0 on the R1 row |
| 1750 | 6 (hit scenery) | cross up −51, cross down −28, then nothing | **80** | by itself (an R2 at +24 changed nothing) |

So on the tick before each reason goes non-zero, the pad holds exactly what it had held for 20 to 40 ticks.
No press precedes any of the three wrecks, and all three happened in the air or at a lip with the
throttle or a trick button held. The user's word "bailing" for this recording (2026-10-20) is these three
crashes, not a command.

The second row is the only press-to-state latency the recording has. The reset chord cleared reason 12 on
the very tick its second button went down: **0 ticks**, which matches a respawn with no page.

No oracle run, because there is no bail function to run; the enumeration above is a read of the
decompilation. `./kit check` was not needed for a change with no code in it, and `./kit done` runs it
anyway.

## Not verified / not 1:1

- **"There is no bail" is a read of the decompilation plus one recording, not an exhaustive proof.** The
  enumeration covers every call through a slot +0xfc that Ghidra decompiled as such. A wreck entry reached
  through a function pointer copied out of the vtable first, or in code Ghidra did not mark as a function,
  would not show up. The three captured wrecks and the absent string make that unlikely, but it is not
  ruled out.
- **The trick buttons (controls 0x11–0x13) are not in the capture.** The watch spec takes sixteen controls;
  the trick layer's three virtual controls sit past them. The wire word covers every physical button, so
  no physical press is missed, but what the trick layer believed it held is not captured. S30's columns
  will show it.
- **Bindings 0x4c and 0x4d are L1 and R1 by the recording, not by the binding table.** The human pressed
  L1 then R1 and the wreck ended on the R1 tick; the table that maps 0x4c/0x4d to buttons was not read.
  P20 is opened to read it.
- **That the reset chord re-places a *riding* rider is read from the gate, not seen.** The gate has no
  test on the reason being non-zero; nobody pressed the chord while riding in either recording.
- **The capture is P15's, not C6's,** and it was taken by another session. Its metadata says it ran to its
  frame budget with no gaps, drops or null samples. I did not re-record it.
- **Reason 6 and the hand chord's binding are not ported here**; they are S37 and P20.

## Next

READY for the human. Either:

1. **No bail, as in the original.** Close S29 as answered: the original has none, and the port matches it
   by not having one. S37 and P20 carry what the search found.
2. **An invented bail, as an addition.** Say which button and what it does: which reason, and so which
   throw, and whether it works in the air, on the ground or both. Then it goes in behind a flag that is
   off by default, like P17's flat spin, so the replay tests stay bit-exact with it off. There is nothing
   in the original to prove its latency or recovery against. The nearest measured numbers are recording2's
   recovery lengths above (124, 80, and 24 with the chord).
