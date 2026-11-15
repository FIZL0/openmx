# F31b5r — heap in rust

2026-11-15. Task: `./kit plan F31b5r`.

The atv heap (alloc, free, realloc, and the new/delete/resize entries) is now ported to Rust in
`handling::heap`. Two oracles show it is exact on about 12,000 generated calls and 2,000 calls on the
real boot heap. The `"heap"` switch cuts the pure replays to 421,673 (ride-2) and 173,752 (flip-1)
interpreted instructions, and both stay bit-exact. Details in 1 (FINAL; the F31b5 acceptance still
waits on F31b5q..u).
