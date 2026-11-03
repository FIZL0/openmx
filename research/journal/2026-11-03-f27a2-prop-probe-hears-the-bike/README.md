# F27a2 — prop probe hears the bike

2026-11-03. Task: `./kit plan F27a2`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

In the original, the ridden bikes' frame collidables are filed on the props' node. A knocked prop's
ray probe strikes them, and the material table's listener queues that hit. knock-2881 has two
entries: the bike on ball 2 (material 21) and ball 2 on the bike (16). The port's props never had
the ridden bikes in their search, so the second entry was missing. When the ball was the faster of
the pair, the table therefore kept the bike's entry at the wrong speed and place. Each prop's probe
now also tests the ridden bikes' `bikef.col` (`flight_search::bikes_struck`). This is for the
material listener only: a hit adds `Struck::Bike(player)` to the collection's `heard` and nothing
else. On knock-2881 the port's queue now equals the original's, entry by entry. On 2882, 2883 and
2897 neither side hears the ball strike the bike. Done; details in
`1-prop-probe-hears-the-bike-FINAL.md`.
