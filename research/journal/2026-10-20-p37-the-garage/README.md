# P37 — the garage

2026-10-20. Task: `./kit plan P37`.

Nobody could change their rider or their bike. Now the front end has a Garage: one column per seated
player, each walked by its own pad, choosing class, model, livery, tier (Race or Pro), tuning (A or P),
rider and colours from what the disc offers. The picks are kept in `garage.txt` beside `settings.txt`.
Every ride loads what each seat picked: its own bike archive's `.phy` for the set (D7's loader name) in
its own `Sim`, its own rider's gear, and its own bike's chassis and parts. `--set` and `--garage` do the
same without the front end. Part 1 built the roster, then blocked on P36. Part 2 built the screen and
wired it into the ride. What is still open is in part 2's "Not verified" list: the window path was not
pressed by hand, and the original's own garage was never compared.
