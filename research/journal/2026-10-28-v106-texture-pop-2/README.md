# V106 — texture pop 2

2026-10-28. Task: `./kit plan V106`.

The user still saw texture pop-in on the free-ride ground: tyre tracks sharp near the camera, then a
band, then blur. It was V67's rut paint on the GPU. A near target (4 texels a foot, 512 ft) was read
up to its own edge and a whole-map far target (3.4 ft a texel) past it, a hard switch that jumped 64 ft
each time the near target followed the rider. Now there is a 1-texel-a-foot mid target between them,
and the ground fades near → mid → far by the eye's own distance (152–216 ft, 600–880 ft), so neither
an edge nor a target redraw shows. Headless rides over generated ruts and over free06's own paint and
ruts measure the frame-to-frame step at the band: 3.3–7.4 before, 0.2–0.5 now (free06: 4.4 → under its own
shimmer). Details in `1-texture-pop-2-FINAL.md`.
