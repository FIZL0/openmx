# MD138a — atv clips

2026-11-14. Task: `./kit plan MD138a`.

The quad's clips now play. A stunt's own `ATV_Bike_*` clip plays on the chassis wherever the original binds
one (rows marked UsedForBike). The rider's `ATV_rider_*` stunt clip plays from Call::StartStunt to
Call::EndStunt. The stunt word that MD65c's quad animation driver runs on is kept the way the original's
bike keeps it. The brief named ATV_Bike_BackFlip, but the original binds no quad clip for the backflip
(UsedForBike=F), so during a backflip the chassis plays the riding pose's middle clip at a half, as the
original does. tests/atv_clips.rs proves this with a real flip off a ramp. The original's choice of rider
row and lean is not ported (its reader was not found) and went to MD162. Details in 1-atv-clips-FINAL.md.
