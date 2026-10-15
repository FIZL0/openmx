# 1 — the disc (FINAL)

Tasks D1, D2. 2026-10-15.

## What I did

### The dump this port is built against

Everything in this repo is true of exactly this image and no other:

| | |
|---|---|
| title | MX Unleashed (USA) |
| serial | `SLUS-20767` |
| region | US (NTSC) |
| version | `1.00`, from `SYSTEM.CNF` |
| volume id | `RAINBOW` |
| image | 2,219,933,696 bytes |
| sha256 | `fc64c537824f616277b821fd24da58949d13fe83b2e3be89ddd58a89795e1aa2` |

`SYSTEM.CNF` in full — three lines, no surprises:

```
BOOT2 = cdrom0:\SLUS_207.67;1
VER = 1.00
VMODE = NTSC
```

`kit.toml` now carries the serial, region, version, the ISO path, `[emu] pcsx2 = /usr/bin/pcsx2-qt` and
`[ghidra] home = /opt/ghidra`. It is git-ignored, as it should be: it is machine- and dump-specific.

### The disc, in full

`./kit extract --list`, and this is the whole thing:

```
MX Unleashed (USA).iso  (2048-byte sectors)  volume 'RAINBOW'  SLUS-20767  boot SLUS_207.67  ver 1.00
4 files, 2116.5 MB
     LBA        bytes  path
    2002      249121  MODULES/IOPRP270.IMG
    2124  2215571456  MX.PSR
     274     3537456  SLUS_207.67
     273          57  SYSTEM.CNF
```

- **`SLUS_207.67`** — the boot ELF, 3,537,456 bytes. Its size matches the directory record exactly, which
  is D2's test. Covered in `d3-boot-elf`.
- **`MX.PSR`** — the archive, 2,215,571,456 bytes: 99.95% of the disc. Every track, bike, texture, sound
  and parameter file the game loads is in here. Format cracked in `d5-archives`.
- **`MODULES/IOPRP270.IMG`** — the IOP module pack (`IOPRP` + version 2.70): the SDK's standard bundle of
  `cdvdman`, `sio2man`, `mcman` and friends that the game hands to the IOP at boot. Not game code and
  not something a port reimplements — the host OS does what these did.
- **`SYSTEM.CNF`** — above.

So there is nothing on this disc that *is not* either the one ELF or inside the one archive. That is worth
knowing early: there are no per-level files to find, no loose audio, and no second executable.

## What the original does

Not yet examined — this task was identification and inventory only.

## How it was proved

- The extracted boot ELF is 3,537,456 bytes, equal to its ISO9660 directory record. (D2's test.)
- `./kit doctor`: every check clean except `no PS2 BIOS`.
- The hash was taken after the image finished copying. An earlier read of it caught a partial file at
  1,265,041,408 bytes; if any number in this repo ever looks like it belongs to a smaller disc, that is
  why, and the hash above is the arbiter.

## Not verified / not 1:1

- **No BIOS, so nothing live has run.** PCSX2 has never booted this game here. Everything recorded so far
  is static: the disc's directory, the ELF's headers, Ghidra's output, the archive layouts. See
  `2-the-bios-READY.md`.
- `IOPRP270.IMG` was identified from its magic and version alone; I did not list the modules inside it. If
  something later turns out to depend on an IOP module's behaviour, that is unexplored ground.
- `MX.PSR`'s header words 2, 3 and 5 (`1466`, `75254`, `13176`) are still unexplained; see `d5-archives`.
- The region is US and the version is 1.00 — a PAL or a 1.01 disc would plausibly have different addresses
  throughout. Nothing here is cross-checked against another dump.

## Next

D3 and D4: the boot ELF's segments, and whether the game uses overlays.
