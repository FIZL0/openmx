# 1 — dressed rider beside the original (FINAL)

Task P91. 2026-10-26.

## What I did

- Set up the original, PCSX2 slot 3, from a first_recording-derived session: a custom rider "Maximilianus",
  number "888", THQ1 gear. Start Event copies the name back from the saved-profile records, so to change
  the name I had to poke every copy: the race profile at 0x506718 (number +13), 0x599b30, 0x599ba0 (+13),
  and the records 0x515f79, 0x51d639, 0x565949, 0x56d7d9, 0x56ff29 and 0x584f09 (number +17). After the
  pokes I quit to the main menu so it repaints. I rode a 125 (CR125_THQ_01) and a CR250_THQ_01.
- In both races I pulled the painted `rider\tex\namedefault0.pst` texture out of RAM (64x128, 8-bit
  indexed, palette CSM1-swizzled: index bits 3 and 4 swap) into context/probe/p91-cr{125,250}-{pix,pal}.bin.
- Took ride screenshots (original and openmx) and set the jersey crops side by side. They match.
- dress.rs: painting now follows the original's text painter. `Face` reads a `.cel` the way the painter
  does (spacing, line height, whole-texel boxes, kerning), `measure`/`draw`/`blit` centre on whole texels,
  use no scale or fitting, write by linear index (wrapping) and use the original's alpha blend. The name,
  number and plate positions are the original's.
- dress.rs: a pro rider with no chosen outfit wears the gear.dat [JerseyPants] uniform that its
  `<rider>.sub` renames of THQ_JerseySleeves_01/THQ_Pants_01 select (`uniform`, `Kit::uniform`).
  `Kit::texture` now takes the disc so it can read that .sub.
- Removed `menu::Font::has`, which only P86's fitting used.
- Tests: 5 unit tests in dress.rs; tests/menu.rs
  `a_long_name_and_a_three_digit_number_are_painted_as_the_original_paints_them` checks the uniform
  table, the plate font per bike, the per-row painted counts against the RAM capture, and sample colours.
- research/oracle.txt: nameplate_paint and its helpers.

## What the original does

- `./kit fn 0x0020baa0` (called from 0x0020b860, fonts loaded by 0x0020b240): for the player,
  profile = rider+0x6b4. It strcpy's 13 bytes of name from profile+0x38 and sprintf's "%s" from
  profile+0x45 for the number. The name and number fonts come from *(profile+0xc)+0x18/+0x1c, the plate
  font from the bike record's +0x1c. AI riders go through rider+0x6b8 instead. It calls
  `./kit fn 0x001af2b0` (font, tex, text, x, y, 1, 1) four times: name at (0x20,0xf), number at
  (0x20,0x27), plates at (0x1f,0x4f) and (0x1f,0x6d).
- 0x001af2b0: measures the string with `./kit fn 0x001aead0`. The measure is the glyph widths plus, for
  each inner gap, the after of the glyph before it, the before of the glyph after it, the spacing and the
  first matching kerning pair. The left edge is x − (int)(w/2+0.5) and the top is
  y − (int)(glyph0.rows/2+0.5). Each glyph's width×rows texels go through `./kit fn 0x0018a5c0` at
  index y*width+x. Nothing clips x, so the text wraps a row. A clear font texel is skipped. For any
  other texel, rgb = (d·da + s·sa)/(da+sa) and alpha = sa. Glyphs past the font draw glyph 0.
- Uniforms: `./kit fn 0x00371a10` / 0x003716c0 read gear.dat's [JerseyPants]. 0x003712c0 picks the first
  entry whose jersey and pants match the rider's. The port's table matches RAM for every rider: Parsons1
  → Skin1 (name3_black), Parsons2 → Skin2, Faisst → Generic, Renner → TLD, Jessman/Boniface → Fly, the
  Thor family → Thor1/Thor2, Ward/Adams/Larocco/Lewis/Styles → Oneal, Wey → Answer, Lusk → Generic2,
  Preston → Shift.
- Plates: every 125 uses plate3 (white digits). CR250_THQ_01 uses plate3_black. All 52 bike plate fonts
  in the port match RAM.
- The career profile reset at 0x0036e1f0 writes the name to +0x89 and the number "001" to +0x9a.

## How it was proved

- A save-state capture: the NameDefault0 texture from RAM for both bikes, compared texel for texel with
  the port's (scratch compare.py). The coverage mask has 0 mismatches. The worst difference is rgb 4 and
  alpha 2. The test pins the per-row counts (const ROWS).
- RAM reads of each rider's font pointers against the port's uniform table, and of all 52 bikes' plate
  fonts.
- Looked right beside a screenshot: ride shots of both bikes with the jersey crops side by side
  (scratchpad, not committed).

## Not verified / not 1:1

- The original quantizes the painted texture back to an 8-bit palette. The port keeps RGBA, so colours
  differ by up to 4 (rgb), within the quantizer's tolerance of 5. That step is not ported.
- Alpha: the PS2 0..0x80 alpha is doubled for the compare, which leaves ±2.
- Kerning: drawing sums every pair for the next code, while the measure takes the first match. No retail
  font repeats a pair, so the difference is untested.
- Characters above 0x7f and the font's multibyte mode are not exercised. Characters past 255 draw glyph 0
  by assumption.
- When the player picks a pro Gear Set in the original's Bike & Rider screen, I did not check whether
  profile+0xc changes. The port uses look.outfit if one is set, else the uniform.
- The AI riders' name and number path (rider+0x6b8) was read but not compared on screen.
- On screen: only the THQ1 jersey and two bikes, and the plates only in the texture and menu shots, not
  in a side-on ride shot.
- Texels that would land outside the texture: the port skips them. The original writes past the buffer,
  into whatever memory follows. A 12-character name at scale 1 stays inside it.

## Next

none
