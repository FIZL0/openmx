# K63 — oracle test time

2026-11-17. Task: `./kit plan K63`.

The gate's three slowest oracle binaries spent their time on harness work, not on the code under test.
Each slow test the brief named was profiled with a stack sampler (perf is blocked here) and its cost was
cut, with the same cases and the same checks. Every test the brief named now runs well under 60 s. Some
mxvatv_bike_ride replay tests that the brief did not name are still 70–225 s under gate load: that is
K73. Details in 1-oracle-test-time-FINAL.md.
