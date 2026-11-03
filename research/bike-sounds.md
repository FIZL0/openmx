# The ridden bike's sounds: the original's against ours

P285, 2026-11-03. Journal: `research/journal/2026-11-03-p285-bike-ride-sounds`. Function names are in
`research/oracle.txt` (the P285 block and the P66 / P145 / P85 blocks it points to).

Scope: every sound the original plays for a motocross bike that is being ridden and is not wrecking.
The wreck's own sounds ([Wreck], rider.aud [Fall] / [IMPACT]) are only listed where a ride sound shares
their code.

## Where the sounds can come from

The whole sound bank (`Aud\audio.res`, 279 files) holds these non-engine sounds, and nothing else:

| group | files |
|---|---|
| Crowd | boo1, ohh1, ohh2, cheer1, cheer2, crowd, cheer.vig |
| Rider | pre_fall1..5, FALL01..06 |
| Vehicle | WRECK01/04/05/06, suspen00..03, trkbtm_1/2, mchn_wreck1..5 |
| Impact | drv_drt1, 21 material hits (woodhit1/2, woodfence, mtlhit1/2, vhclhit1, polehit1, linkhit1, stonehit1, jonhit1, hayhit1, plastichit1, tirehit1, barbhit1, bikehit1, football, eye, egg, bowling, ball, treehit), wood.vag (no .aud or .cmt names it), plane_tire |
| SFX | cannonball, RumbleMono |
| other | Soultree world ambients, Freestyle UI tones, the engine sets |

There is **no** file for a gear shift, backfire, chassis rattle, skid, slide, roost, spray, splash or
water, and none per ground (mud, sand, grass, asphalt, wood floor). The four bike files
(`vehicle_125a`, `250a`, `450a`, `50a`) have only `[Params] [Panic] [Clutch] [Idle] [Decel] [LowAccel]
[HighAccel] [Power] [Sputter] [Wreck]`. The 125's `[Rev]` has `NumSounds = 0`, so nothing plays from it.

The bike's sound builder makes four emitters: the engine note, the wreck clank, the event emitter
(Cannonball) and the drive-on loop. It makes no machine emitter, so `machine.aud`'s `[Suspension]` and
`[TireTouchdown]` never reach a bike (P66). Only the car, the buggy and the biplane build one.

## The table

| sound | the original's trigger | ours |
|---|---|---|
| Idle, LowAccel, HighAccel, Power, Decel | the engine note's state machine on throttle and rpm (vehicle_*.aud) | **played** (`audio.rs` `BikeSounds`, `Note`) |
| Sputter | the engine's sputter state | **played** |
| Clutch (3 samples) | the clutch lever | **played** (`Note::Clutch`) |
| Rev limiter: rev01complete, panic01 | `[Panic]`, the rev to the limiter and its unwind | **played** (`Note::Rev`, `Panic`, `Unwind`) |
| `[Rev]` (125 only) | empty list | none in either |
| Gear shift, backfire, chassis rattle | none: no sample on the disc, no section | none in either |
| Landing thump by hardness, suspension bottoming, creaks | none on the bike: no machine emitter (P66; the matched filter below agrees) | none: **same** |
| Tyre / surface by ground | none: ground is material 0 and stops the drive-on loop; there is no terrain table | none: **same** |
| Drive-on loop (`Impact\drv_drt1`) | rear wheel on a prop of material 1..22, level and pitch by speed | **played** (P145, `DriveOn`) |
| Skids, slides, roost, spray, splash | none: no samples | none: **same** |
| Bumping props and fences | the material table's collision listener: an awake prop's probe strikes something, and the struck material's `CollisionSoundFile` plays when the prop's speed is at least `CollisionMinImpulse` (5), at level `clamp(speed / CollisionMaxImpulse × 100, 0, 100)` (Max 25), placed at the prop, 30..125 ft. Bike vs. bike (ID 16, bikehit1) uses 2 and 9. The struck object's foliage flag gives `SolidFoliageID` (22) | **missing**. Nothing in crates/ reads `CollisionSoundFile` |
| Rider grunt on a landing | none: rider.aud has only `[Fall]` and `[IMPACT]`, both wreck-only | none: **same** |
| Crowd bed (crowd, Ambient) | the track's `[Crowd] CrowdPresent`. It is commented out in all 54 track .aud files, so no track has a crowd | **same** (`WorldSounds` loads only when CrowdPresent is set) |
| Crowd "Oh" | the bike's wreck entry, at the chassis | **played** on a wreck, but there is no crowd to hear it |
| Crowd Cheer / PlayerCheer | a race place gained, once the gainer has ridden 5 s; a local human gets the unplaced PlayerCheer stream, an AI the panned Cheer | **missing**: the port has no race ranking. It could not sound anyway, with no crowd |
| Crowd Boo | a human loses a place to an AI who is then first | **missing**, as above |
| Cheer on a trick landing | none: the original never cheers for a trick | **different**: `audio.rs` plays `world.cheer` when `landed_trick` (P42). Never heard, because no track has a crowd |
| Cannonball | the event emitter (P85) | **played** |
| `machine.aud` Suspension tiers (buggy, baja, monstertruck) | the car's machine emitter on a landing | not the bike, so out of scope here. Unported (the `audio.rs` comment on those tiers) |

## Follow-ups filed

- **F, the material collision sounds.** This is the one missing group a rider can hear. The trigger is in
  the row above.
- **C, low, the trick-landing cheer.** Remove it, or replace it with the place-change cheer and boo once
  races rank riders. It costs nothing today because no track has a crowd.

The overtake cheer and boo are left inside that C task and not filed on their own: the port does not
rank a race, and no track enables a crowd, so they cannot be heard.

## By ear: the matched filter

A by-ear check needs a recording in which a sound is known to play. S109's `run2.mp4` (free09, a 44.1 s
ride that ends in a wreck; t ≈ (frame_counter − 1938) / 60) was searched with every candidate's first
0.25 s, decoded from its VAG, using a normalised cross-correlation. As a control, each clip was injected
at 0.2× the local level and found again.

| clip | best | where | what the capture says |
|---|---|---|---|
| pre_fall3 | 0.51 | 29.66 s | [Fall] index 2 at tick 3726 |
| WRECK05 | 0.78 | 30.82 s | the clank, about 3792 |
| WRECK04 | 0.50 | 30.82 s | the same clank (they share an attack) |
| FALL02 | 0.55 | 30.93 s | [IMPACT] |
| suspen00..03, trkbtm_1/2 | flat (the top three within 0.02) | none | no landing sound, as P66 found |
| drv_drt1, every Impact hit | flat | none | no prop touched or knocked in this ride |

The wreck sounds stand out against second-best peaks of about 0.2, which shows the method can find a
sound in this recording. The landing and suspension clips are absent even though the ride lands many
jumps. The prop hits were **not** exercised: no recording yet has a knock with audio.
