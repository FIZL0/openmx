# P298a — ios background gpu

2026-11-04. Task: `./kit plan P298a`.

The user's JetsamEvent (2026-11-04: openmx frontmost at 5.2 GiB, vm-pageshortage) said memory, so a ride
now gives its models, materials and textures back to the GPU on iOS's DidEnterBackground
(`Renderer::give_back_models`), inside the save wait before suspension. The first frame back, under
the pause, uploads them again from the assets still held. Decoded audio was measured at 5.9 MB for a
ride and kept. File 1 was the wait for the report; file 2 is the work. Details in the numbered files;
the newest one's suffix says whose move it is (FINAL · PART · READY).
