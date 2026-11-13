# S297 — race gate props

2026-11-13. Task: `./kit plan S297`.

Ported the supercross loader's prop lookups (scene and toy list, with the game's high-byte `tolower`) and the prop bound query, and proved sx01's finish-line width and height. The finish gate is a toy whose fireworks dummies were animated away from the gate box after the load; at the model's rest pose the bound is the gate box, and the line matches S196's image to the bit.
