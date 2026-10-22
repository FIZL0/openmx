# S103 — camera out of the ground

2026-10-22. Task: `./kit plan S103`.

The chase camera's eye went under the terrain where the original's does not. The original's race camera
keeps it out through a class slot the base filter calls on the raw eye before the lags and on the lagged
eye after them (S24 had it as uncalled): a floor of twice the near clip or the camera's minimum height
(4.8 ft on the disc) over the ground under the eye, under a fog-set ceiling. Ported bit for bit
(`Race::filter`, `Race::keep_off_the_ground`), proved on the EE over 4000 cases and 4000 whole frames,
and on two PCSX2 captures of the human's free09 ride replayed over `world::Ground`: as the disc has it
(the floor never engages; eyes match to 0.01 ft) and with the floor poked to 12 ft (it engages over half
the ride; the port matches up to `world::Ground`'s own blend, up to 0.38 ft off the game's ground — S104).
Details in `1-camera-out-of-the-ground-FINAL.md`; `bike-camera.toml` and `play_camera.sh` recapture.
