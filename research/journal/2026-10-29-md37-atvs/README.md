# MD37 — atvs

2026-10-29. Task: `./kit plan MD37`.

The user asked for MX vs. ATV's quads as a machine. That is a port of a whole vehicle class (four-wheel
step, tire read, wreck, model, clips, cameras, sound, particles, garage, netplay) proved against SLUS-21104,
and no MX vs. ATV save state with an ATV exists yet. Too big for one session, so per AGENT.md it was split.
This session did the first part, reading the ATV in both builds: every per-frame function of
`MXVehicleAtv` found in the PC exe and in SLUS-21104 (30 oracle rows, `# MD37`), and how each one differs
from the bike's (research/mxvatv-pc.md, "The ATV (MD37)"). The rest is filed in order: MD37a captures,
MD37b the bit-exact step (after MD40, whose SLUS-21104 moto steps it runs on), MD37c the asset pack,
MD37d the garage and pick, MD37e online seats. No product code changed.
