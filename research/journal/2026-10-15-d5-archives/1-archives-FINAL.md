# 1 — the archives (FINAL)

Task D5. 2026-10-15.

## What I did

### `MX.PSR` — the outer container

2,215,571,456 bytes, which is 99.95% of the disc. A directory at the front, then every member laid out on
2048-byte boundaries, uncompressed. Sector alignment because members are read straight off the disc: one
seek, one whole-sector read.

```
00000000  02 00 00 00 7b 02 00 00 ba 05 00 00 f6 25 01 00  |....{........%..|
00000010  c1 4e 00 00 78 33 00 00 05 00 00 00 3c 50 41 44  |.N..x3......<PAD|
00000020  3e 00 00 35 0c 0a 00 00 00 05 00 00 00 3c 50 41  |>..5.........<PA|
00000030  44 3e 00 00 9f 24 aa 86 01 00 0a 00 00 00 6e 6f  |D>...$........no|
```

```
0x00  u32  2            version
0x04  u32  635          entries, the two <PAD>s included
0x08  u32  1466         unknown
0x0c  u32  75254        unknown
0x10  u32  20161        directory length in bytes, measured from 0x18
0x14  u32  13176        the name pool: sum of (namelen + 1) over every entry
0x18       635 x { u32 namelen, char name[namelen], u32 size, u32 first_sector }
```

Read the first entry out of that dump: `05 00 00 00` namelen 5, `3c 50 41 44 3e` = `<PAD>`,
`00 00 35 0c` size 204,800,000, `0a 00 00 00` sector 10. Then the second: namelen 5, `<PAD>`,
size 614,400,000, sector 100,010. Then namelen 10 and the name beginning `no` — `normal.ico`.

Three things fall out of that:

- **`<PAD>` entries are reserved gaps, not files.** There are exactly two, and they are the first two
  entries: 204,800,000 bytes (exactly 100,000 sectors) at sector 10, and 614,400,000 bytes (exactly
  300,000 sectors) at sector 100,010. Round *decimal* sizes, so they were written by a build tool
  reserving space rather than by anything structural. Together they cover sectors 10–400,010, and the
  first real member starts at sector 400,010 — i.e. the first 819 MB of the archive is deliberately
  empty. (A guess worth nothing until checked: the far side of a DVD reads faster, so a build may have
  pushed the data that matters outward. Not verified.)
- **Names are `\`-separated and case is inconsistent** — `Tracks\sx01.res`, `UI\Vid\Attract3.pss`,
  `machine.res`. They are stored as the build tool found them on a Windows filesystem. `Entry::name`
  keeps them verbatim; `Entry::path()` converts for writing out.
- **`hdr[5]` is the name pool size.** 635 entries × 12 bytes of fixed fields = 7,620; 20,161 − 7,620 =
  12,541 bytes of names; 12,541 + 635 = 13,176 exactly. So the loader knows how much to allocate for the
  name strings before reading them, each NUL-terminated in memory even though they are not on disc.

The directory ends at 0x4ed9 with a zero namelen, and 0x4ed9 − 0x18 = 0x4ec1 = 20,161, matching `hdr[4]`.

633 real members, 1,396 MB of them:

| | files | size |
|---|---|---|
| `.res` | 523 | 602.4 MB |
| `.pss` | 44 | 788.0 MB |
| `.lsi` | 57 | 4.5 MB |
| `.ico` | 7 | 0.6 MB |
| `.img`, `.cmt` | 2 | 0.2 MB |

`.pss` is Sony's standard PlayStation Stream (MPEG video + ADPCM); it is three quarters of the disc and
none of it is simulation. `.lsi` are loading-screen images. Everything else the game runs on is in the
`.res` files.

### `.res` — the inner container

```
00000000  03 00 00 00 1e 00 00 00 40 03 00 00 00 00 00 00  |........@.......|
00000010  12 00 00 00 5b 12 00 17 22 11 ac 11 d2 00 8e 00  |....[...".......|
00000020  7c 01 87 03 00 00 04 00 00 00 7a 00 00 00 15 00  ||.........z.....|
00000030  00 00 61 70 70 73 74 61 72 74 5c 6d 61 74 5f 73  |..appstart\mat_s|
00000040  74 62 6c 2e 74 78 74 00 00 00 00 36 81 01 00 13  |tbl.txt....6....|
00000050  00 00 00 61 70 70 73 74 61 72 74 5c 64 66 6c 74  |...appstart\dflt|
00000060  6d 74 2e 70 73 6d 00 e0 01 00 64 00 00 00 13 00  |mt.psm....d.....|
00000070  00 00 61 70 70 73 74 61 72 74 5c 69 63 6f 6e 6d  |..appstart\iconm|
00000080  74 2e 70 73 6d 00 40 02 00 cf 03 00 00 0f 00 00  |t.psm.@.........|
00000090  00 75 69 5c 75 69 63 6f 6c 6f 72 73 2e 74 78 74  |.ui\uicolors.txt|
000000a0  00 a0 02 00 d8 10 00 00 78 9c ad 5c 5d 73 a3 3a  |........x..\]s.:|
```

That is `startup.res` entire, bar its data. Annotated:

```
0x00  u32  3            version
0x04  u32  30           = table_bytes + 12
0x08  u32  832          constant across every .res on this disc; meaning unknown
0x0c  u32  0
0x10  u32  18           table_bytes: the block table including its terminator
0x14  u16  0x125b 0x1700 0x1122 0x11ac 0x00d2 0x008e 0x017c 0x0387   8 blocks
0x24  u16  0            terminator
0x26  u32  4            entries
0x2a  u32  122          directory length in bytes, measured from 0x2e
0x2e       4 x { u32 namelen, char name[namelen], u8 0, u24 at/256, u32 size }
0xa8       8 x { u8 deflated[packed[i]], u8 0xff }
```

Read the first entry: `15 00 00 00` namelen 21, `appstart\mat_stbl.txt`, `00` the NUL, `00 00 00` offset
0, `36 81 01 00` size 98,614. The second: namelen 19, `appstart\dfltmt.psm`, NUL, `e0 01 00` offset
480 → 480 × 256 = 122,880, size 100. Then `ui\uicolors.txt` at 672 × 256 = 172,032, size 4,312.

Four things here are worth more than the layout:

- **The whole archive is one logical deflate stream cut into fixed 24,576-byte blocks.** Every block
  inflates to exactly 24,576 bytes — the last one too; the stream is padded. `0x6000` is six pages, and
  presumably the size of the loader's bounce buffer.
- **Every member starts on a block boundary**, which is why the offsets fit in a u24 of 256-byte units
  (480, 576, 672 are 5, 6 and 7 blocks). This is the design decision that makes the format good: reading
  one 433-byte file out of a 10 MB archive inflates *one* block, not four hundred. `Res::read` does
  exactly that, and `tests/archive.rs` checks that a member read alone is byte-identical to the same
  range of the fully unpacked stream — which is also the test that the per-member block indexing is
  right rather than accidentally starting from zero.
- **Blocks are separated by a single `0xff` byte.** It is not a length, not a checksum and not alignment:
  consecutive blocks sit at 0xa8 + 4699 + 1, + 5888 + 1, and so on, exactly. It is the one thing in
  either format that is not self-describing, and the reader errors if it is missing. (I first guessed
  4-byte alignment, which is wrong — the gaps happen to be one byte.)
- **The block count comes from the header, not from scanning for the terminator**: `table_bytes / 2 - 1`.
  A zero in the length table would otherwise truncate an archive silently.

Three archives were checked field by field, and the identity that makes the layout certain rather than
plausible is that every byte is accounted for:

| | blocks | entries | `table_bytes` | header claim | measured |
|---|---|---|---|---|---|
| `startup.res` | 8 | 4 | 18 | 21,308 | 21,308 |
| `Tracks\sx01.res` | 406 | 280 | 814 | 2,117,369 | 2,117,369 |
| `machine.res` | 430 | 213 | 862 | 2,825,749 | 2,825,749 |

`data_start + Σ(packed[i] + 1)` equals the file length exactly in all three. `Res::parse` refuses to
parse anything where it does not.

### What is in there

Enough of the archive was extracted to know where the port's first two tasks get their data.

`Tracks\sx01.res` — 280 members, one supercross track:

```
  .trn     1 file    1,624,255   the terrain
  .scn     1 file      297,947   the scene: what is placed where
  .pst    41 files   1,586,108   textures
  .psg    85 files     386,303   geometry
  .col    69 files       60,894   collision meshes
  .cub     1 file      166,940   ? (a cube map, or a spatial index)
  .atm     1 file      137,652   ?
  .psm    51 files      52,885   materials
  .emt/.prt/.mot/.pco/.mcf/.vut/.cmt/.aud/.dat
  bike\bikeriderconfig.dat       which bikes and riders appear on this track
```

`Bike\250sx.res` — 54 members, one bike:

```
  bike\race250a.phy    33,706   physics parameters, plain text
  bike\race250p.phy    33,668
  bike\pro250a.phy     33,688
  bike\pro250p.phy     33,684
  bike\wreckbike.phy    1,043
  bike\col\bikef.col      900   front wheel collision
  bike\col\bikem.col      676   main body collision
  bike\anim\*.psg               frame, engine, plastics, fender, number plate
  bike\anim\*.mot               catwalk, nacnac, kod, tsunami, flatliner — the trick animations
  bike\anim\250sx.sub           the bike's part list
  rider\aichrctr.aip   85,703   the AI rider
```

`machine.res` — the other vehicles: `baja.phy`, `buggy.phy`, `monstertruck.phy`, `biplane.phy`,
`kiowa.phy` (a helicopter), and `tracks\*\machine.txt` placing them.

**The physics parameters are text.** `race250a.phy` is 1,493 lines of INI with these sections:

```
[Mass Properties]  [Front Tire]  [Rear Tire]  [Engine]  [Roll Control]  [Pitch Control]
[Aerodynamics]  [Airborne Control]  [Preload]  [Backflip]  [Donuts]  [No Hander Lander]
[Joystick]  [Wrecks]  [Collisions]  [FirstPersonCamera]  [Animation]
```

and reads like this:

```
[Mass Properties]
Weight = 400
Gravity = 1
Inertia = 22.4, 18.4, 10.9
CofG = 0.0, 2.0, 0.0
CofGTravelForward = 0.05
CofGTravelBack = 1.1
YawRateLimit = 3

[Front Tire]
UseParametricCurve = True
StrutCompression = 1
MaxCompressionRate = 20
TireRadius = 1.2
TireInertia = 0.7
Spring = 1.5
SpringWhenStopped = 1.1
BounceDampingRatio = 0.3
ReboundDampingRatio = 0.3
MaxSteerAngle = 30
SteeringTorque = 200
SteeringDamping = 20
SteeringInertia = 0.8
BodyTireForcePosition = 0.0, -0.25, 0.75
NumberOfSurfaces = 16
; Friction zone 1 - (black) Standard on-track dirt
MaxDriveForce1 = 2
SkidDriveForce1 = 1.9
```

Two tires with sprung struts, per-surface friction over 16 surface types, a centre of gravity that
travels fore and aft under throttle and brake, a steering model with its own inertia and damping. The
600 lines per tire are mostly those 16 surfaces' friction curves.

This changes what S3 is. The numbers are not something to recover from a decompilation — they are on the
disc in named fields. What still has to come out of the decompilation is the *code*: the order the forces
are summed, where each parameter is used, and the arithmetic, which has to be bit-exact.

`.phy` is `a`/`p` suffixed per machine (`race250a`, `race250p`) — presumably arcade and pro handling
modes, which the game offers as a setting. Unconfirmed.

### The code

`crates/game-sim/src/archive.rs`: `Psr`, `Res`, `Entry`, and a `inflate` module.

It parses byte slices and hands back offsets rather than reading files, because this crate does no I/O
(rule 4 in `lib.rs`). `Psr::parse` wants only the front of the file — the directory — so the app layer
can read a member from a 2 GB file without mapping it. `Res::parse` takes the archive whole, which is a
couple of megabytes.

The inflate is hand-written, in the shape of zlib's `puff.c`: a canonical Huffman code as a count per
length plus the symbols in length order, decoded one bit at a time. That is there because the port has no
third-party dependencies, which is what lets the simulation build for wasm and aarch64 without a C
toolchain. It validates the zlib header, checks Adler-32, and rejects over-subscribed Huffman codes.
Unlike everything in `physics`, there is nothing here that has to match the PS2 bit for bit: inflate is
lossless, so it either reproduces the bytes or it fails.

## What the original does

Not read yet. This format was recovered from the bytes, not from the game's loader — see below.

## How it was proved

`crates/game-sim/tests/archive.rs`, four tests, all against `context/iso/MX.PSR` itself, skipping with
`./kit extract MX.PSR` when it is not there:

1. **`the_containers_directory_accounts_for_the_whole_file`** — all 633 members are sector-aligned, none
   runs past the end of the image, and the last one ends within one sector of it. So the file is its
   directory plus its members, with nothing unexplained and no trailer.
2. **`every_member_of_an_archive_extracts_and_the_sizes_add_up`** — D5's own test. Reads `Bike\250sx.res`
   out of `MX.PSR` by offset, parses it (which *requires* the byte accounting to balance), inflates all
   54 members, and checks each one's length against its directory entry and each one's bytes against the
   same range of the fully unpacked stream.
3. **`an_inflated_member_is_really_the_file`** — inflating the right number of bytes is not the same as
   inflating the right ones: `bike\race250a.phy` must start with `[Mass Properties]` and contain
   `[Front Tire]`, `[Rear Tire]`, `[Engine]` and `[Collisions]`.
4. **`the_track_archive_reads_too`** — a different shape: 406 blocks, 280 members, and a scene file
   spanning a dozen blocks, so the per-member block indexing is exercised rather than the one-block case.

Plus three unit tests that need no disc: a hand-built stored block round-trips, a corrupted Adler-32 is an
error, and an empty or truncated header is an error rather than a panic.

`./kit check`: 60 tests pass.

## Not verified / not 1:1

- **The format was read off the bytes, not off the game's loader.** Everything above is consistent across
  three archives and accounts for every byte of each, which is strong, but it is still inference. The
  loader is in the decompiled image and I have not read it. It is worth doing when a member somewhere
  fails to parse — and it is the only way to settle the unknown fields below.
- **`MX.PSR` header words 2 (`1466`) and 3 (`75254`) are unexplained.** Not the entry count, not the
  directory size, not the name pool, not the largest member in sectors (79,992), not the total member
  sectors (681,812), not the directory count (11), not the longest name (36). They are stable and nothing
  in the port reads them. Same for `.res` header word 2, which is `832` in every archive on the disc.
- **`0xff` between blocks is a separator by observation.** Three archives, 844 blocks, every gap exactly
  one `0xff`. If it is really a flag with other legal values, the reader will refuse an archive it should
  have accepted, loudly.
- **Only 5 of 523 `.res` files have been parsed** — `startup`, `appstart`, `modules`, `shaders`,
  `machine`, plus `Tracks\sx01` and `Bike\250sx`. Not all of them, and in particular nothing in `UI\`
  (848 MB of it) and nothing in `Rider\`.
- **Nothing inside a member is understood yet.** `.scn`, `.trn`, `.col`, `.psg`, `.pst` are all opaque
  binary at this point; this task only got them out of the container. The terrain and collision formats
  are what S4 needs and they are not started.
- The `a`/`p` suffix on `.phy` files is assumed to be arcade/pro. Unconfirmed.
- `.cub` and `.atm` are unidentified. `.vut`, `.pco`, `.mcf` and `.emt` likewise, though all are small.
- `Psr::find` and `Res::find` are case-sensitive, while the game's own references to these names are not
  always consistently cased. Nothing has needed a case-folding lookup yet; something will.
- The 2 GB image is read with ordinary file reads in the test, not mapped; that is a tooling choice, not
  a finding.

## Next

R1, then R2: the image is stripped, so naming comes from strings and the call graph. Two specific things
to look for, both of which pay for themselves here:

1. **The `.res` loader** — settles the unknown header fields and confirms the `0xff` separator. Find it
   by the 24,576 block size, or from `./kit data --strings boot` on a `.res` name.
2. **The `.phy` parser** — it reads those INI sections into a struct, so it is a direct map from each
   parameter name to the field the physics code actually uses. That is the single most useful function in
   the game for S3, and it is readable text on both sides.
