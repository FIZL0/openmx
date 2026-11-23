# S508 — truck mounts and inertia

2026-11-23. Task: `./kit plan S508`.

R48's plane stopped matching the original at row 1587 (S489's last difference). The cause was not the
truck's rays or brakes. The Sim's truck had its tire mounts and inertia from the unplaced model, not
the placed one the original uses. The test now seats both off the image, and R48's push holds to the
capture through row 1640. The shipped game still builds them the old way (S512). Details in file 1.
