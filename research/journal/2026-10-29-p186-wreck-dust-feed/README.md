# P186 — wreck dust feed

2026-10-29. Task: `./kit plan P186`.

The shell's live wreck feed for the rider's and the bike's dust. Rider Dust/Chunks/Foot Dust now take
each ragdoll sphere's zone from the byte its own ground query writes (`ragdoll_contact::Contact::surface`,
the original's bone collision +0x3c). Before this, the shell ran a P61 ground query at the contact point.
The new byte matches the original's solve on 10,000 oracle cases and P153's captured spheres on
1,888 rows. Vehicle Dust's velocity (`collider.relative`, the original's +0x1e0) needed no change. A
replay of the port's wreck bike from P153's state matches it bit for bit on all 127 ticks to the
respawn. Details in 1-wreck-dust-feed-FINAL.md.
