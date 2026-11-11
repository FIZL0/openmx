# MD127 — mxvatv car step

2026-11-11. Task: `./kit plan MD127`.

Ported the car's contact sort and its four tire probes into `atv_car::Pieces`, using MX Unleashed's on
the buggy image's words. Under the frame oracle in tests/mxvatv_car_frame.rs, they are bit-exact
unhooked: 3,000 frames with the sort alone, 3,000 with the sort and probes, and 3,000 with all eight
pieces unhooked together with MD124's tire step. The words the probe and the step share now live
once, on `Tire`. Details are in 1-mxvatv-car-step-FINAL.md.
