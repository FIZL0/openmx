# S239 — bikes wake props

2026-11-16. Task: `./kit plan S239`.

A prop's rays striking a ridden or AI bike's frame now wake an asleep prop and activate its
collection. A bike is struck only when the prop's kept list last gathered it within the probe's reach,
as in the original. The sweep list, the wreck hull and the trailer toys were already done (S243 and
earlier). Proved bit for bit on first_recording with the egg and an AI bike (`tests/prop_bike_wake.rs`).
Not proved: a trailer case and the ridden bike (see 1-…-FINAL.md).
