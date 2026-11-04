# R31 — why the joust from 137845 parts on tick 137863

2026-11-03. Task: `./kit plan R31`.

After F32, C16's joust from 137845 held 18 ticks and parted on tick 137863. New images 137859..137866
show the port's plane and wreck bike bit for bit through tick 137862. On 137863 the original's plane
ray 33 strikes the downed wreck bike's mesh collidable (`wreckbikef.col`, category 0x9a058). The port's
plane search has no wreck bike in it. Follow-up F35. Details in
[1-joust-parts-on-the-planes-ray-FINAL.md](1-joust-parts-on-the-planes-ray-FINAL.md) and
`research/r31/index.md`.
