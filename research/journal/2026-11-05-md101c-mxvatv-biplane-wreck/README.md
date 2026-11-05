# MD101c — mxvatv biplane wreck

2026-11-05. Task: `./kit plan MD101c`.

The task asked for SLUS-21104's biplane wreck/recover machine to replace MX Unleashed's 2.5 s / WreckTime
rule when `mxvatv` is set, with recovery at MaxWreckTime − 2.0. The plane's own crash test does not do
that: it is MX Unleashed's machine on MXVehicle's words, with WreckTime = MaxWreckTime. The −2.0 belongs
to the *car's* crash test (a flipped car, wreck 14). One real difference turned up and is fixed: MXVehicle's
wreck entry starts the clock at zero on every wreck. It is proved by an oracle of the crash test over
10,000 generated cases, and by a new crash capture (biplane3crash) that the interpreter flies word for
word while the port's `Crash` runs beside the original every frame. Details in 1-mxvatv-biplane-wreck-FINAL.md.
