# MD71 — MX vs. ATV's wreck clock and put-back

2026-11-02. Task: `./kit plan MD71`.

The tail of SLUS-21104's wreck test (`./kit dis 4e2c68 --len 0x170 --program atv`) is MX Unleashed's ride
machine with its clock and put-back changed. The clock adds `dt` before the settle window is asked. One
limit, MaxWreckTime, covers every reason. The put-back is the bike's own unwreck, with no request, no page
and no owner. Its base half zeroes the clock as well as the reason. This is ported as
`handling::step::machine`, and `rider.rs` runs it on MX vs. ATV rides. A new oracle in
`tests/mxvatv_step.rs` runs frame sequences through the real wreck test and the real unwreck off
`state04.ee.bin`, and they match the port. Details in the numbered file.
