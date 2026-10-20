# 1 — sound and music (FINAL)

Task P42. 2026-10-20.

## What I did

- **`crates/openmx/src/audio.rs`** (new) is the whole of the shell's sound:
  - the decoders (PS-ADPCM, `.vag`, `.vig`, the sound bank);
  - the `.aud` lists and a mixer with distance falloff and one listener per view;
  - the engine-note machine, the biplane's keyed loops, the rider's wreck voice, landings, the track's emitters and the crowd;
  - music behind one `MusicSource` interface: the disc's songs, then a folder of the user's own `.wav` files;
  - the output, which pipes 48 kHz s16 stereo into `pw-cat`, `pacat` or `aplay`, whichever is found first.
- **`main.rs`**:
  - `--no-sound`; a `--headless` ride is silent too.
  - The front end's `Speakers` play on across the menus and the rides. A ride without the front end makes its own, set from `settings.txt`.
  - `ride()` reads a `Sound` for its seats and feeds it a `SeatFrame` per seat each frame, after the cameras, with one `Listener` per view camera. A split screen is one mix of every local rider.
  - The ride's effects stop when it ends or pauses; the music does not.
- **`menu.rs`**: Settings gains three rows, Music (Off, Shuffle, each track), Music volume and Effects volume (0–10). They are written to `settings.txt` as `music`, `music_volume` and `effects_volume`.
- **`game-render/src/disc.rs`**: `Disc::member_part`, a ranged read. The sound bank is 163 MB and is never read whole.
- **`tests/menu.rs`**: the reachability walk's depth goes from 6 to 7. Settings has three more rows, so Controls is one press further down.
- **Taken from opentennis's `hst-base/src/audio.rs`**: the shape of the ADPCM decoder (filter table, shift, clamp) and the PCM16 RIFF walk for the folder's WAVs. Its Bevy playback is replaced by the pipe, since this workspace takes no outside crates.
- **Follow-up S55**: the engine note's random draws.

### The formats (all decoded here, nothing extracted)

- **`Aud\audio.res`** (an `MX.PSR` member) is the sound bank. Its layout:
  - The header is `u32 3, u32 0x370c, u32 0x100`.
  - The directory starts at `le32(4) + 8`. Its records are `[u32 offset][u32 size][u32 namelen][name]`, the first starting *at* the count word: the count is that record's offset field.
  - The data starts at the directory's end rounded up to 0x2000.
  - 279 records. The first, `RumbleMono`, is unused.
  - Check: the data at the base is 0x49b0 bytes long, which is WRECK01's size less its 0x30 header.
- **`.vag`**: a 0x30 header with its magic zeroed. Big-endian size at 0x0c and rate at 0x10 (22050 for everything read). Then mono PS-ADPCM, ending at the first block whose flags have bit 0 set.
- **`.vig`** (the songs): a 0x800 header with the per-channel size at 0x0c, rate 44100. Stereo, interleaved in 16-byte blocks L, R. `crowd\cheer.vig` is the exception: despite its name it is a mono VAG.
- **`.aud`** files are ini text:
  - `vehicle_*.aud` is in each bike archive: [Idle], [LowAccel], [HighAccel], [Power], [Sputter], [Decel], [Clutch], [Panic], [Wreck].
  - `rider.aud`: [Fall], [IMPACT].
  - `machine.aud`: the `*Suspension` tiers.
  - `carengine.aud`: `[biplane]` keyed loops, whose rate is a Hz offset from `OriginalSampleRatePS2`.
  - `event.aud`: Crowd*.
  - A track's own `.aud`: Soultree and Ambient emitters.
- **`appstart.res` `aud\music\music.pb`**: quoted strings, `[Artists]` then `[Song N]` with Filename, Title and Artist. 13 songs. College Kids' stream is a 28 ms stub and is dropped, which leaves 12.

## What the original does

- **The engine note** chains whole samples: when one ends, the next list's sample is queued. It is not a pitched loop.
  - The decider (`./kit fn @engine_sound_decide`) picks the next note from the hands: gas, clutch, on the ground, and speed against `LowTorqueSpeed`.
  - The chainer (`./kit fn @engine_sound_chain`) queues it. It picks through `sound_pick`, which draws the game's generator, and draws `rand` once more for a pitch variance.
  - The note states' own frames are `engine_note_*` in `research/oracle.txt`.
  - In this port the machine is the shell's (`Engine::update`). The draws are the host's own `Dice`, so the game RNG is untouched (AGENT.md: audio is presentation).
- **The rider's wreck voice**: `rider_wreck_sound_load` reads `[Fall]` and `[IMPACT]`. `wreck_sound` (already ported as `game_sim::wreck_sound`) sorts the reasons: 1–7 and 12 are crash, 8–11 and 14 are fall.
- **The crowd**: `crowd_sound_load` reads `event.aud` only when the track's `CrowdPresent` is set. Every freeride track has that line commented out.
- **The music list**: `music_list_load` reads `music.pb`.

## How it was proved

All by `./kit check`. The files are `crates/openmx/tests/audio.rs` (6 tests) and `tests/menu.rs` (18). The full check passed.

- **A hand-decoded block**:
  - Block `0x2a`, data `0x71 0xf8`, history `[100, 50]` gives `[143, 204, 218, 222]`, each worked out on paper.
  - Filter 0 with shift 12 gives `[-1, -7, 0]`.
  - Clamping is exercised at 32767.
- **The disc**:
  - The bank has 279 entries.
  - WRECK01 is 22050 Hz mono, 1.497 s, rms 9833.
  - `idle02` is found by any spelling.
  - `cheer.vig` is mono.
  - These disc halves skip without a disc. I proved they ran by temporarily making them panic, then reverted that.
- **Music sources**:
  - A temporary folder of generated WAVs plus the disc gives 12 + 2 tracks, labelled "Headstrong - Trapt".
  - Off is silence.
  - Switching to a track plays it and repeats it; the volume slider scales it; shuffle moves on.
- **A headless ride follows the throttle**:
  - A stock-physics bike on flat ground, with tones for sounds: engine loudness rises 4× on the gas.
  - The disc's 250 (`Bike\250sx.res`) on the same ride: the sounding clip is a throttle list's for 0.98–1.00 of the frames in seconds 3–5 and for none of them in seconds 0–1 and 8.
- **Split screen**: two seats into one mix; both engines are heard.
- **The live shell**:
  - A short ride with a stand-in `pw-cat` that wrote the stream to a file: 8.8 s of non-silent stereo (rms about 1900, then about 2600 once a song started).
  - The front end's Settings shot shows the three new rows.

## Not verified / not 1:1

- **Nobody has listened.** Every check above is numeric. The mix's balance, the falloff and the pan direction were never heard by a person.
- **The engine machine is approximate:**
  - no 8 ms crossfade at each chained sample's overlap;
  - no Doppler;
  - the clutch and rev transitions are read from the decider loosely, not ported line by line.
  - The rpm pitch bend (0.94 + 0.12 × rev) is ours: the original's pitch is the chained sample's own plus a random variance.
- **The engine's random draws are not made on the game RNG** (S55). Doing so here would move the sim's generator out of step. Until S55 lands, the samples picked differ from the original's.
- **The note-state addresses** in the oracle are matched to states by reading the decider's switch, not by running them.
- **The Fall/IMPACT split by wreck reason** is inferred from `wreck_sound`'s two lists and their names. Which list is which was not traced to the emitter's play.
- **The wreck sounds' volume** is flat, not scaled by the impact.
- **Landings use `machine.aud`'s `*Suspension` tiers** by downward speed. The tiers and their gains are our choice; the original's landing trigger was not read.
- **The distance falloff** is linear between each sound's MinDistance and MaxDistance. That is ours.
- **An ambient's `CycleTime`** is read as frames. Its unit is unverified.
- **There is no crowd in freeride**, because `CrowdPresent` is commented out in every freeride track's `.aud`. The cheer and "oh" code paths are untested on the disc.
- **free06 and free09's train, semi and car sounds** are in the bank, but no track `.aud` places them. They are probably attached to the moving actors, which this port does not run, so they are not played.
- **`streams.juk`** looks like the song list shifted by one. Unverified and unused.
- **`RumbleMono`'s offset** is odd, and eight unexplained bytes follow the directory's last record.
- **The folder source plays PCM16 WAV only**: no MP3 or OGG decoder without outside crates.
- **No UI sounds** (menu moves, confirms).
- **The output is paced by the wall clock**, not by the device: the writer runs 60 ms ahead, and the device's own buffer is not queried. On a stall of more than 0.5 s it skips ahead, a gap rather than a backlog.
- **The shell runs at about 1 fps** in a debug build here, with sound or without (the open plan item ahead of everything). So the per-frame engine updates were only seen at that rate live; the 60 Hz behaviour is proved headless only.

## Next

- S55, the engine note's random draws on the game RNG.
- Someone listening to a ride, with and without split screen, and to the pan direction.
