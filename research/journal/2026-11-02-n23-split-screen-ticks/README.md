# N23 — split screen ticks

2026-11-02. Task: `./kit plan N23`.

Split screen judders on the Deck because each tick runs every seat's whole pass inside one drawn frame.
A 2P riding profile on the desktop puts ~99% of a tick in the seats' passes, split evenly between the
seats. On free06, riding built the bike's search world afresh about every other tick: a physics path car
searches with itself hidden, and the bike's searches see it, so the kept world flipped between two
states and refused the update. `kept_world` now tries every kept world laid out alike before it builds
one. Builds fell from 72 to 3 in 900 riding ticks, the worst ticks' sim fell 15–30% at p90, and the
ride is the same bit for bit (1800-tick `--trace-picture` match, 1P and 2P). Parallel seats or a sim
thread need a thread-safe place and an ordered shared prop set first. That is N23a, and `lend`'s full
copy is N23b.
