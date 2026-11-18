# MD81c — untamed monster

2026-11-14. Task: `./kit plan MD81c`.

Untamed's monster truck as an off-by-default machine pack, on SLUS-21701's step. Entry 1 diffed the
step statically and split the work: capture (MD160) and port, replay and pack (MD161 → MD161a, MD161b).
Entry 3 checked they all landed and their tests pass: MD160's drives replay bit-exact beside SLUS-21701
(with the frames MD161a lists left out), and the shell rides the untamed_cars pack's truck.
