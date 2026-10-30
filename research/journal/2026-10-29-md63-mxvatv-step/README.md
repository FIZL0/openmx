# MD63 — mxvatv step

2026-10-29. Task: `./kit plan MD63`.

MD63 was the rest of SLUS-21104's bike step, diffed against MX Unleashed's clause by clause and ported
under Handling::MxVsAtv. That is too much for one session, so it was split. MD63 ports and proves the
clutch and the preload: both differ, both are now SLUS-21104's under MX vs. ATV, each with a 10,000-state
oracle against the ELF's own code. The input limiter and steer ramp went to MD63a (diffed already, see
the entry), roll/pitch, balance and stand-upright to MD63b, steer integrate and the wreck test to MD63c,
and GravityScale with the friction zones to MD63d. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).
