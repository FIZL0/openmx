# MD123 — atv on mxu

2026-11-10. Task: `./kit plan MD123`.

MD123 is the quad stepped on MX Unleashed's worlds. Part 2 added `quad::Ride`, the type a shell steps once a
tick, along with the MXU World's ground query (`world_ground`) and sweep (`OnWorld`). The Desert Oasis chain
now runs bit-exact through `quad::Ride`, and the image-seeded quad rides free09 headless. The tail, the
wreck and the put-back were split off as MD123a. The rest, the quad made from its files, is file 4 (FINAL): `quad::Ride::spawn`
makes the quad from its `.phy` and chassis `.psg` alone, `quad::Ride::rig` lays MD123a's record, tail and
wreck from the files (with the bike's `m.col` riding rays). It matches C28's spawned quad and MD123a's
rigs word for word apart from MD134's one float, and rides free09 headless: gas, steer, a lip jump, a
wreck on a prop and the put-back. Details are in the numbered files; the newest file's suffix says whose move it is
(FINAL · PART · READY).
