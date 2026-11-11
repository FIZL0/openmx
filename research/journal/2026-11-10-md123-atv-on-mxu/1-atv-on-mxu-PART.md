# 1 — atv on mxu (PART)

Task MD123. 2026-11-10.

## What I did

- Checked the dependency before starting. MD123 steps "quad::frame on a quad from MD122", and its gate
  test is "a quad made from its files rides free09 headless". MD122 (quad::Load, the quad from an
  atv_*.phy) is committed on `task/MD122` (fd555f409) but is not on main and is still open in the plan,
  so `crates/game-sim/src/quad.rs` here has no loader. Every quad on main is seeded from a C28 RAM image.
- No code. I didn't start the World-backed ground query and Sweep against an image-seeded quad: the
  test the task asks for can't be written without the loader, and doing it against an image would be
  reworked once MD122's types land in the same file (merge churn in quad.rs, which MD122 also edits).
- Blocked with `--after MD122`, so the run reopens this task once MD122 merges.

## What the original does

Not read this session (blocked before the port).

## How it was proved

Nothing was proved. There is no product change.

## Not verified / not 1:1

- All of MD123 is still to do: the World-backed ground query and Sweep, the Calls left to rest, the
  atv_ride type, the headless free09 ride and the Oasis replay through the same type. Reason: it waits
  on MD122's loader landing on main.
- MD122's own note says the half box and wheel mounts still come from the image (MD124). A quad made
  purely from files may need MD124 too. I haven't checked this, because MD122 isn't on main.

## Next

Once MD122 is on main: read `quad::Load` in quad.rs and tests/atv_quad/load.rs. Check whether a quad can
be built with no image at all (the MD124 half box and wheel mounts). If it can't, block on MD124 as well.
Then port the World adapter for quad::Steps' ground query and Sweep.
