# 1 — bike tracks (FINAL)

Task V40. 2026-10-25.

## What I did

- Read the original's tire tracks: the vehicle particle manager's `TireTrack` entries, their update, the
  strip they lay and the ring that keeps it. Notes are in `research/oracle.txt` under V40.
- Ported them as `crates/openmx/src/tire_tracks.rs`. It reads the entries from the track's
  `mxparticles.dat` (with `mxparticlezones.dat`), steps them off each wheel after the frame's
  simulation, and hands the strips out as glows cut from `bike\tex\ttrack.pst`.
- `crates/openmx/src/main.rs` loads the entries and the tread, keeps one `Tracks` per seat, and steps
  every seat that is riding on a bike, not paused or waiting. Each view's glows start with all the seats'
  marks, so both renderers draw them through the glow path the portals already used. The software path
  now draws a view's glows from the joint texture list even without portal art.
- Tests in `crates/openmx/tests/tire_tracks.rs` (8):
  - the disc: every freeride's two entries, their zones and the tread;
  - a replayed ride of the stand-in bike over a kicker;
  - the ride untouched, and the same tape laying the same marks;
  - the ring wrapping and a stopped bike's marks holding still;
  - speed, zone, wreck, object and air gates, the compression gate and spin;
  - the fade-out over its distance;
  - the break angle.
- `research/v40_tracks.py` is the live probe. It loads a state, plays a `./kit pad send` script, and
  logs the wheels and the entries' fades, strip and ring. `research/v40/*.pad` are its pad scripts and
  `research/v40/tape.py` writes the shell tape the by-eye shots ride.

## What the original does

Each track's `mxparticles.dat` (in `<track>_2.res`) has two `TireTrack` entries:

| entry | tire | Width | MinimumCompressionThreshold |
|---|---|---|---|
| Front | 0 | 0.26 | 0.85 |
| Rear | 1 | 0.275 | 0.75 |

Both entries share the rest:

- NumberOfStrips 32, HeightAboveGround 0.3, BreakAngle 10, MinimumSpeedThreshold 3,
  MaximumCompressionThreshold 1, BasePercentAlpha 0.4, FadeIn 1, FadeOut 10;
- Disable `NoContact | ObjectContact | Wrecking`, Material `ttrack`, ZoneDescriptor `TireTrack`.

The zones differ by track. `[TireTrack]` turns off nothing on free06 and free10, surface 0 on free07 and
free09, and surfaces 0 and 9 on free11. The material is `bike\tex\ttrack.pst`, 32×64 and black, with the
tread in its alpha (at most ~95 after the GS doubling). It is drawn with normal alpha blend and the alpha
test on.

The manager switches an entry off while its tire has no contact or an object contact, or while the bike
wrecks, and on a zone its descriptor names `None`. Off ends the strip and sets the fade-out left to 0 and
the fade-in left to FadeIn.

While on, each frame works as below.

- **Distance.** It takes d, the tire's distance from last frame. While the strip is ended, last is the
  tire itself, so d is 0.
- **Not marking:** the wheel is neither spinning nor past MinC, or the speed is ≤ MinSpeed.
  - f = out/FadeOut and out -= d. Below MinSpeed, out is also scaled by speed/MinSpeed.
  - in = f·FadeIn. After an off, out is 0, so this sets the fade-in to 0, and the next marking frame
    is at full strength at once.
- **Marking:**
  - f = (FadeIn − in)/FadeIn, in -= d, out = f·FadeOut.
- **Strength.** a = f·(Base + clamp01((comp − MinC)/(MaxC − MinC))·(1 − Base)). Above 0.005 the strip
  takes a row; at or below it, the strip ends.

The strip:

- **Texture.** u += d_since_last_row/2 every call.
- **Rows.** A row is laid once the tire is 2 ft on, or u + 1 ≥ 8. Its two edges are
  pos + n·h ∓ (dir × n)·W, where h = Height + rand()·2⁻³¹·0.005 from the C library's rand.
- **Segments.** One ends past BreakAngle or at u + 1 > 8: u = −8, a restart row, and a new alpha byte
  (a·128). Only then, and at a strip's start, does the alpha change, so a segment is one alpha all along.
- **The ring.** It is NumberOfStrips batches of 16 vertex slots. A batch begun mid-strip starts with the
  last row again. ADC bits keep the batches and the restarts from joining. The whole ring is drawn every
  frame and nothing fades with age: old marks go only when the ring comes round onto them, so one entry
  keeps at most 32 × 16 points.

A live free09 run on copy 1 (`research/v40_tracks.py`, state `first_recording.p2m2_SaveState.p2s`):

- a launch's wheelspin laid the rear at alpha byte 51 (0.4 of 0x80), and it faded once the spin
  stopped;
- the ring had 32 batches;
- both wheels' zone bytes read 13 on the dirt.

Side by side, the marks are faint and thin and lie behind the bike (screens c3/c4 in the scratchpad, not
kept).

The reader skips `TireTrack` entries when the event has more than one human player, so the original
draws no tracks in a split.

## How it was proved

- `tests/tire_tracks.rs::every_freeride_lays_the_discs_two_tracks_with_the_bikes_tread`, against the disc:
  the 5 freerides' (not `_ss`) lists, zones and tread parse to the numbers above.
- `a_replayed_ride_lays_marks_under_each_wheel_in_contact_and_none_in_the_air`:
  - 900 ticks of the stand-in bike flat out over a 4.5 ft kicker, then braked;
  - every point a strip writes on a tick is within √(W² + (h + 0.005)²) of its own wheel and above the
    ground, on a tick that wheel has ground contact;
  - both wheels are in the air for ≥ 10 ticks, and nothing is laid then;
  - both wheels mark before the ramp and again after the landing.
- `the_marks_are_the_rides_own_and_touch_nothing`: the ride's transform, velocity and speed are bit for
  bit the same with and without the tracks, and two rides lay equal tracks.
- `old_marks_go_by_the_ring_and_not_with_age`, `a_tire_under_the_speed_or_off_the_zones_or_wrecking_lays_nothing`,
  `a_mark_fades_out_over_its_distance_a_segment_at_a_time` and `a_turn_past_the_break_angle_starts_a_segment`
  test the rules above on hand-made tires.
- By eye: `openmx --track free09 --play <tape.py's tape> --shot … --shot-frame 190 --frames 195` with the
  GPU renderer. A faint, narrow dark line runs back from the rear wheel of the stopped bike, and fades
  out short of it, as the original's did in c4.

## Not verified / not 1:1

- **Split screen is an addition.** The original draws no tracks with two or more human players. The brief
  asks for them per rider in a split, so each seat lays its own.
- **The jitter's RNG is the port's own** (a xorshift per entry, seeded by seat and entry). The original
  draws it from the C library's global `rand()`, shared with everything else. It is 0.005 ft at most, and
  the sim's RNG is not touched.
- **When the tracks step.** They step once per simulated frame, after the frame, off the wheels' end
  state. The original's manager runs once a game frame too, but where in its frame against the sim's two
  passes was not traced.
- **`wheel.surface` as the zone byte is not proven.** The port's per-wheel surface is taken as the
  original's record zone byte. Free09's dirt read 13 live, and no test compares the two.
- **`slip.spinning` as the "spinning" word is not proven.** The port reads `slip.spinning` for the
  record's +0xe0 word. Its meaning was read from the code, not compared live.
- **The glows are unfogged**, as all the port's glows are. The original's strips are fogged geometry, so
  far marks are darker than the original's. Marks are faint and near the bike, so this barely shows.
- **The software renderer averages alpha per triangle.** Its glow path uses one mean alpha per triangle
  where the GS interpolates it. A segment carries one alpha, so only a strip's very first triangles
  differ.
- **The software renderer was not checked by eye.** Its camera framed the bike at the bottom edge and the
  marks were off-frame. It draws through the same `glow::draw` the portals use.
- **No side-by-side match beyond "faint and thin".** The original was not shot from the port's exact
  pose, and the textures were not compared pixel for pixel.
- **Only local seats lay tracks.** Online remote riders are not stepped, and there are no AI bikes in the
  port.
- **NumberOfStrips and HeightAboveGround have no constructor value.** The original leaves them
  uninitialised and every file gives them. The port falls back to 32 and 0.3.
- **The stand-in bike rides the ride test, not race125p.** Its compression (~0.23) never passes the disc's
  0.75/0.85, so the test uses a 0.2 threshold. It also lays the first row's u from a stale start, as the
  original does.

## Next

- Look at the marks beside the original from one pose (free09, a hard launch and a braked stop).
- Compare `wheel.surface` with the zone byte in a disc or PINE test.
