# C41 — machine race brain

2026-11-20. Task: `./kit plan C41`.

Read the free-ride machine racer's brain and ran S339's path-branch proof on it. No new PCSX2 image was
saved: the human's free09 recording (`context/states/first_recording.p2m2_SaveState.eeMemory.bin`) is
already a machine-race session. Its event takes the MachineRacer course record, and its monster truck
racer is parked with its brain built and holding the MachineRace's `free09MR1.tdf` as its net. The race
had not started, and I did not find what starts one. The brain reads mode 4, seeker state 0, no target
jump, and net = the race file. Mode 4 is fixed when the machine is placed, and it drives through the gate
driver (`ai::drive::mode_four`), which never calls the seeker. So the free-ride path branch never runs on
this brain, and **the Sim needs no machine-race lane net**. A machine race in the Sim is S423's work: mode 4
on the race file and its gates. The port's branch still matches the original bit for bit on this brain and
net (2,602 branches, new test `c41_the_ticks_path_branch_rides_the_machine_racers_own_net`). Details in
1-…-FINAL.md.
