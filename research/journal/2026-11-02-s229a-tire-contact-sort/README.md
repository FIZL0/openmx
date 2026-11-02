# S229a — tire contact sort

2026-11-02. Task: `./kit plan S229a`.

The cars' tire contact sort is ported. A second ray probe sends four rays down through the tires. It is
searched against the collision object's nearby list, and each hit goes to a tire by quadrant. The tire
probe then keeps the sorted contact or hands the tire back to the terrain. A new capture backs the baja
into free07's garage slab and out again (`make_tape.py`, `capture.sh`, `sort.toml`). It replays
bit-exact in car_drive.rs, row by row and carried. Details in `1-tire-contact-sort-FINAL.md`.
