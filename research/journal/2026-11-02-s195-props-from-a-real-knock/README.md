# S195 — props from a real knock

2026-11-02. Task: `./kit plan S195`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

A free09 beach ball struck by the bike in PCSX2, captured a row a tick, and the port's props tick
(`PropSet::frame_among`) replayed against it from states saved after the strike. With no pad, the bike
is put in line with ball 2 and set rolling at 25 ft/s by poking its velocity (research/s195/aim.py);
research/s195/knock.py records watch/s195-knock.toml and saves states around the strike. The port agrees
bit for bit on the struck ball's flight, landing, bounces and roll for 207 ticks (frames 2886–3092), until the bike touches
it again; on a second ball the bike nudged, through its sleep, and — set from the game's row where it
sleeps again — through the 60 s countdown and the put-back to the capture's end. The off-the-map edge
reset (reached by poking a knocked ball to x 6909, research/s195/edge.py) agrees too: disabled, put back
and the collection asleep on the crossing tick. The strike itself (the struck listener with the
contact's push) is S197's; one finding for it: the strike puts the object's listener on before the
collection's settle, the other way from `PropSet::wake`. Done; details in `1-real-knock-FINAL.md`.
