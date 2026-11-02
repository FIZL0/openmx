# S205 — biplane spawn up

2026-11-02. Task: `./kit plan S205`.

The plane's spawn now carries an up, the ground query's normal under a `machine.txt` bay, squared
against its nose the way the original's physics place squares it (nose kept, up re-crossed, both
rsqrt-normalised, in place on every reset) — proved against the original's instructions over 10,000
inputs. `SetGroundHeight = 0` keeps a bay's file height. Every biplane bay on the disc is flat, so on
the disc nothing visible changes; the S143 put-back captures still match bit for bit. Online seats'
bays are filed as B5.
