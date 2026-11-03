# N23a — the fixed update on its own thread

2026-11-02. Task: `./kit plan N23a` (N23's second half).

Split screen judders on the Deck because a tick frame runs every seat's whole pass and then draws. The
seats can't step side by side and stay the same ride: they share one world, and each seat reads what the
seats stepped before it wrote that tick. So the whole tick moves instead. A tick a picture follows now
runs on a sim thread kept for the ride (a one-thread rayon-core pool, `src/sim_thread.rs`). Meanwhile the
loop draws the picture the last tick ended on, from a copy of the seats. `--sim-thread auto` (the default)
does this when a window's split screen can't fit its slowest tick and a picture in a picture's time.
`on` forces it, `off` never does it. Per-tick hashes of every seat (`--trace-hash`) are identical with the
thread and without it over the 1800-tick 2P gas tape, headless and windowed. On this desktop the loop's
own sim time falls from ~0.8 to 0.01 ms a frame. Not measured on a Deck.
