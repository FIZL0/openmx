# V46 — path car root turn

2026-10-21. Task: `./kit plan V46`.

Do the `.toy` path cars, and the bike and rider, lose the file's root node the way V41's scene props
do? Yes for the cars: the original's car makes the same model object as a prop, and that object's node
reader takes the root node to be the object itself, so the car's pose overwrites the root's matrix.
The shell now loads cars through `Assets::load_placed`, so free06's train_car_03/04/05 (whose root is
a half turn) are no longer drawn back to front. This is proved against the biplane state's object
matrices and boxes in `tests/toy.rs`. The bike's and rider's roots are identity on the whole disc, so
nothing changes for them. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
