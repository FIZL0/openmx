# F31b5 — every port at once, and what is left

2026-11-08. Task: `./kit plan F31b5`.

F31b5 was reopened after F31b5g, F43, F44, F45, F46, F47 and S278 landed. Before this session no
replay ran all of their ports together. This session added one that does: C15 ride-2 and flip-1
replay bit-exact with every port on at once, and the RAM matches the handing run's. The same replay
counts the instructions the interpreter still runs in each part of the frame. That count turned up
six places with no task, filed as F31b5i..m. Together with F54, they are what keeps the frame from
running without the interpreter, so the task is blocked on them. Earlier entries:
`../2026-11-04-f31b5-bike-on-quad-steps/`, `../2026-11-05-f31b5-still-waiting/`.
