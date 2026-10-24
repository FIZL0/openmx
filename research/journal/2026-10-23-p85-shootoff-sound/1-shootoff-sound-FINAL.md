# 1 — shootoff sound (FINAL)

Task P85. 2026-10-23.

## What I did

- Found the shootoff's sound in the decompilation. It is a sound child of the player's bike, not
  part of the wreck code. It is read from `Aud\event.aud`'s `[Cannonball]` and holds the boom (`Cannon`)
  and a rising rumble (`Rumble`). Its update also drives the music fade. The addresses are in
  research/oracle.txt under "P85".
- Recorded the original riding off free09's west edge with PINE (`watch/p85-edge-sound.toml`,
  `research/p85_edge_sound.py`), twice: `west.csv` (throw at tick 4242, put-back at 4601), and
  `west_av.csv` with PCSX2's Video Capture running for the sound (`capture/run.sh`, `capture/events.py`;
  throw at 4383, put-back at 4736). The captures are in context/capture/p85, which is git-ignored.
- Ported it to `crates/openmx/src/audio.rs`:
  - `edge_distance`, `rumble_volume` and `music_scale`, the edge query and the two curves.
  - `Shootoff`, the `[Cannonball]` reader, read in `Sound::read` through `Reading::shootoff`.
  - `EdgeSound`, one per `SeatSound`, which is the child's update.
  - `Mixer::music_edge`, the music's scale, which `music_gain` applies.
  - `Mixer::gain_of`, so the tests can read a voice's gain.
  - `impl Drop for Sound`, which puts the music back at the ride's end.

  `main.rs`'s `seat_sound` hands each seat's edge gate, wreck reason and its chassis' cofg distance
  to the edge (`EdgeFrame`).
- Tests are in `crates/openmx/tests/audio.rs`:
  - `a_replayed_shootoff_booms_and_fades_the_music_as_the_original`: 64 rows of west.csv, compared tick
    by tick.
  - `every_captured_shootoff_replays`: every csv in context/capture/p85 when present. Both captures pass,
    1951 and 2182 ticks, one boom each.
  - `the_edge_is_asked_as_the_original_asks`
  - `a_split_booms_for_the_rider_thrown`
  - `the_shootoff_sounds_are_the_discs`

## What the original does

The bike's builder (`./kit fn 0x00280fb0`) makes the child (`./kit fn 0x001fe7e0`). The child's update
(`./kit fn 0x001fea70`) runs among the bike's children, after the physics and before the ride's state
machine. So it sees this tick's position and **the last tick's wreck reason**. Each tick, while the bike's
edge gate (+0x570, the port's `edge_armed`) is set:

- **Reason 13 (cannonball) newly seen:** the rumble stops, the boom plays and the music stream's pause
  flag is set (`./kit fn 0x001686b0`). In the captures the flag is clear again the next tick, and the song
  plays on at volume 0. The boom therefore comes **one tick after the throw tick**: reason 13 at 4242,
  boom voice at 4243.
- **Reason 0:**
  - If the last reason seen was 13 (the put-back), the music is set back to the options' volume, at once
    with no fade-in (`./kit fn 0x001fdf90`). That happens one tick after the put-back.
  - Then the edge query (`./kit fn 0x001a6e50`, 350 ft) on the chassis' cofg. Its result is
    `d = min(x, W−x, z, D−z)` when the cofg is within 350 ft of an edge.
  - **Within 350 ft:** the rumble starts if it is idle, at volume `(int)((350−d)/200·75)+25` clamped to
    0..100. Every song's volume is set to `(int)(v·(1−(350−d)/200))/100` of the options' music volume v.
    That runs from v at 350 ft to 0 at the 150-ft throw margin, and goes a hair below 0 past the margin
    (−0.01 seen).
  - **Outside:** the rumble stops and the pause clears, and the music volume is left where the last tick
    inside put it.
- **Other reasons:** nothing.

The fade's **depth** is to 0. Its **length** is the 200 ft from 350 to 150 ft. It is linear in distance,
not time. The music **comes back** at the put-back.

Neither sound is placed in 3D, and the child makes no positional call on either. On the disc,
`Aud\event.aud`'s `[Cannonball]` names both sounds, and both are in the `Aud\audio.res` bank:

| Sound | Sample | Format | Playback |
|---|---|---|---|
| `Cannon` | `SFX\cannonball_stereo` | 22050 Hz stereo, 8.00 s | streamed, plays once |
| `Rumble` | `SFX\RumbleMono` | 22050 Hz mono, 1.50 s | looped |

The music calls are also gated on the game's own +0x570, the music being on.

In a split, every bike has its own child. The music is one for the whole game, so the last bike to
write it wins. The port does the same thing in seat order.

## How it was proved

- **Bit-exact replay fixture:** 64 rows of `context/capture/p85/west.csv` (PINE capture,
  watch/p85-edge-sound.toml), run through `EdgeSound` + `Mixer`. Each tick checks the rumble playing,
  its gain, a new boom, and `music_gain()` against the song's volume float, all bit for bit.
- **Whole-capture replay:** both captures, 1951 + 2182 deduplicated ticks, are replayed the same way
  when the csvs are present. They pass with no mismatches.
- **By ear (Video Capture, west_av.mp4/.pcm):** the loudness stays at about 73–78 dB through the fade.
  The rumble cuts out at the throw+1 tick, there are about 6 ticks of near-silence, then the boom, which
  decays over about 9 s. The music comes back at the put-back.
- **Disc:** the test reads both samples out of the bank and checks their rate, channels and length.

## Not verified / not 1:1

- **The stream pause is not ported.** The original sets it on the throw, and something clears it the
  next tick. I did not find the code that clears it, so the port lets the music play on at its faded
  volume (0), which is what the captures show.
- **The boom starts at once in the port.** In the original, the streamed boom is heard about 6 ticks
  (≈0.2 s) after the rumble stops, which is the stream starting up (Video Capture). The port does not
  reproduce that delay.
- **The rumble's volume is mapped to a linear gain (`vol/100`).** That mapping is assumed. The IOP's
  volume curve was not checked.
- **The distance is compared as floats.** The original compares the edge distances on their int bits.
  For the positive distances that matter they agree, and both captures match bit for bit.
- **The game's music-on gate (+0x570) is taken as always on.** The port has no "music off" that the
  original's gate would map to. With the music off, the port's Music is silent anyway.
- **In the plane the port stops the rumble.** The original's bike child presumably does not run then.
  That is the port's own choice, not proved.
- **A pause is the port's own.** The original's pause page pauses every stream. The port's pause stops
  the effects (as `quiet()` does for every sound) and keeps the fade and the reasons, so no second boom.
  The rumble starts again on the first tick after the pause.
- **At the ride's end the port restores the music** (`Drop for Sound`). That is the port's own. The
  original's quit path was not read.
- **Only one place was captured.** Both captures are free09's west edge, at music volume 70. The other
  edges use the same min(), and other volumes use the same formula (the `music_gain` cut to whole
  percent), but they were not captured.
- **The shell was not ridden to the edge live with sound.** The wiring in `seat_sound` (edge gate,
  reason, cofg, the place's area) compiles and is the same data the throw uses (game-sim rider.rs). There
  is no test that drives the real shell's loop to the edge and listens.
- **The tests' tone clips stand in for the disc's sounds.** The fixture tests do not use the real
  samples. `the_shootoff_sounds_are_the_discs` reads the real ones separately.
- **The music for the songs past the disc's 13** (the original's ripped-music volume) maps to the port's
  folder volume through `music_gain`'s existing split. It was not captured.

## Next

Nothing for P85. A follow-up could find what clears the stream pause the tick after the throw.
