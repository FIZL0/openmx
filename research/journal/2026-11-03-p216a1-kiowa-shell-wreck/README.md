# P216a1 — kiowa shell wreck

2026-11-03. Task: `./kit plan P216a1`.

The shell's Kiowa wrecks and is put back as the original's MX helicopter is: `vehicle::KiowaAround`
hands each crash-test answer to S220a1's `kiowa::Wreckage::act` (the wreck, the blade throw and its
rand draws, the node swap, the unwreck, the put-back), and the fresh-helicopter rebuild is gone. A
player's helicopter has no owner, so the put-back stands it at its spawn, where it was taken.
