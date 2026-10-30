//! Shadows of the things that move (V108): the bike, the rider, the machines and, under the port's
//! own shadows, the world's models too. The ground's baked light maps ([`crate::splat::Shadow`])
//! already darken the ground under hills and trees; this is what casts on top of them.
//!
//! **The original's** (`ProjectedShadow.cpp`, `shadows.res`): each racer is a shadow caster drawn
//! from the sun into a 128x128 render target cleared to nothing (`shadowcasterN.psm`), the target
//! blurred once (`filterTexture`, `filterPasses = 1`), and the target projected onto the terrain and
//! onto the props whose `.scn` entry says `AllowShadowReceiving=1` (`terrainshadowreceiverN.psm`,
//! `shadowreceiverN.psm`), blended `(Cs - Cd) * As + Cd`. The colour is the scene's sun's
//! `ShadowRGB`, black when it gives none, and its alpha `ShadowIntensity` x 128 (85 of 128 when the
//! scene gives none). The projection is an orthographic box along the sun fitted round the
//! casters' bounding sphere. The shadow is drawn only while the camera is within 100 feet of the
//! caster, fading out from 60 (65 fading from 55 with two views or more).
//!
//! **Ours** ([`Shadows::Better`]): the sun's shadows of everything near the camera, the world's
//! models among them, in two nested shadow maps along the sun, filtered soft, onto the ground and
//! onto everything else. Darkened to what the ground's light map is in its own shadows, so a moving
//! shadow and a baked one meet at the same darkness and a baked one is not darkened twice.
//!
//! The GPU half is `gpu`'s; this is the arithmetic both halves agree on, which the tests check
//! without a GPU.

use crate::mat::{M4, V3, v3};

/// The Shadows setting (V108): ours, the original's, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shadows {
    /// Sun shadows of everything near, soft, matched to the light maps.
    #[default]
    Better,
    /// The original's projected shadow under each racer.
    Original,
    /// No shadows but the ground's baked light.
    Off,
}

impl Shadows {
    /// Ours first and the original's after, as Draw Distance's and Texture Filtering's.
    pub const ALL: [Shadows; 3] = [Shadows::Better, Shadows::Original, Shadows::Off];

    /// Its row's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Shadows::Better => "Better",
            Shadows::Original => "Original",
            Shadows::Off => "Off",
        }
    }

    /// The settings file's word, which is also `--shadows`'.
    pub fn word(self) -> &'static str {
        match self {
            Shadows::Better => "better",
            Shadows::Original => "original",
            Shadows::Off => "off",
        }
    }

    pub fn from_word(w: &str) -> Option<Shadows> {
        Shadows::ALL.into_iter().find(|s| s.word() == w.trim().to_ascii_lowercase())
    }
}

/// The original's shadow texture's side, in texels.
pub const ORIGINAL_SIZE: u32 = 128;

/// The original's draw distances, from the camera to the caster: drawn to `.0` feet, fading from
/// `.1` (the game's `(dist, fade)` pair is `(100, 40)` alone and `(65, 10)` with two views or more).
pub fn original_range(views: usize) -> (f32, f32) {
    if views < 2 { (100.0, 60.0) } else { (65.0, 55.0) }
}

/// How much of the original's shadow is drawn with the camera `distance` feet from its caster: all
/// of it to the fade's start, none past the end, and linear between.
pub fn original_fade(distance: f32, views: usize) -> f32 {
    let (end, start) = original_range(views);
    if !(distance < end) {
        0.0
    } else if distance <= start {
        1.0
    } else {
        (end - distance) / (end - start)
    }
}

/// How far from the camera the port draws the original's shadow (user 2026-10-29): to `.0` feet,
/// fading from `.1`. The original's own 100 ft ([`original_range`]) let another racer's shadow fade
/// in and out a few lengths away; a 128x128 map per racer costs next to nothing, so the port keeps
/// them to Better's reach ([`CASCADES`]) whatever the number of views, and fades them smoothly.
pub const PORTED_RANGE: (f32, f32) = (300.0, 210.0);

/// How much of the original's shadow the port draws with the camera `distance` feet from its
/// caster ([`PORTED_RANGE`]): all of it to the fade's start, none past the end, smoothly between.
pub fn ported_fade(distance: f32) -> f32 {
    let (end, start) = PORTED_RANGE;
    if !(distance < end) { 0.0 } else { 1.0 - smoothstep(start, end, distance) }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far under its caster the original's shadow is drawn whole, and where it is gone, in feet
/// straight down from the caster's centre to the ground. Ours, not read (V108's journal): V108 cut
/// the box 40 ft along the sun past the caster's sphere and faded it over the last 40% of that, so a
/// bike 10 ft up under a 15° sun (57 ft along the sun to the dirt) or 25 ft up under a 40° one lost
/// its shadow mid-jump (user 2026-10-29: "when my shadow gets long enough it just disappears").
pub const ORIGINAL_WHOLE: f32 = 120.0;
pub const ORIGINAL_DROP: f32 = 200.0;

/// The lowest sun the original's box is stretched for, as the sine of its height over the horizon
/// (about 6°): below it a high caster's shadow is cut rather than the box made miles deep.
pub const ORIGINAL_LOW_SUN: f32 = 0.1;

/// The original's box for one racer of radius `r`, along `sun` (the way the light goes): across, its
/// sphere, as the original fits it (across the light the shadow is the caster's own outline, however
/// long it lies on the ground); along the light, from just short of the sphere on the sun's side to
/// past where the ground [`ORIGINAL_DROP`] feet under the caster meets the sun's rays through the
/// sphere — the whole of the shadow's footprint, the far tip of a long one too. `.3` is the share of
/// the depth, from the far end, the shadow fades over: whole down to [`ORIGINAL_WHOLE`] feet.
pub fn original_box(sun: V3, r: f32) -> (f32, f32, f32, f32) {
    let dir = sun.norm();
    let s = (-dir.y).max(ORIGINAL_LOW_SUN);
    let c = (1.0 - s * s).max(0.0).sqrt();
    // A ground point under the caster at drop `y`, inside the sphere's outline across the light, lies
    // (y ± r cos) / sin along the light from the centre.
    let along = |y: f32| (y + r * c) / s;
    let (back, ahead) = (r + 10.0, along(ORIGINAL_DROP) + r);
    let fade = (ahead - along(ORIGINAL_WHOLE)) / (back + ahead);
    (r, back, ahead, fade)
}

/// The original's blend of the shadow: the sun's `ShadowRGB` (0..255), and its share of the colour
/// under it (`ShadowIntensity`, or 85 of 128 where the scene gives none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    pub rgb: [f32; 3],
    pub alpha: f32,
}

impl Default for Tint {
    fn default() -> Tint {
        Tint { rgb: [0.0; 3], alpha: 85.0 / 128.0 }
    }
}

impl Tint {
    /// From the sun light's `ShadowRGB` and `ShadowIntensity`, as the scene's light reader takes
    /// them: an intensity only when above 0, a colour only when given.
    pub fn of(rgb: Option<[f32; 3]>, intensity: Option<f32>) -> Tint {
        let d = Tint::default();
        Tint {
            rgb: rgb.unwrap_or(d.rgb),
            alpha: intensity.filter(|i| *i > 0.0).map_or(d.alpha, |i| (i * 128.0).min(128.0) / 128.0),
        }
    }
}

/// The two halves of the sun's light space: across, up the picture, and the way the light goes.
pub fn light_axes(sun: V3) -> (V3, V3, V3) {
    let dir = sun.norm();
    // Any up that is not the light's own way; straight down is the usual sun's nearest, so +Z.
    let helper = if dir.z.abs() < 0.9 { v3(0.0, 0.0, 1.0) } else { v3(1.0, 0.0, 0.0) };
    let right = helper.cross(dir).norm();
    let up = dir.cross(right).norm();
    (right, up, dir)
}

/// An orthographic box along the sun: `half` feet each way across about `centre`, from `back` feet
/// toward the sun to `ahead` feet away from it. World points are given relative to `origin` (the
/// view's, MD15), and come out with x and y in -1..1 and the depth reversed: 1 nearest the sun, 0
/// farthest, as the GPU's depth tests run.
pub fn light_box(sun: V3, centre: V3, half: f32, back: f32, ahead: f32, origin: V3) -> M4 {
    let (right, up, dir) = light_axes(sun);
    let span = (back + ahead).max(1e-3);
    let o = origin.sub(centre);
    // depth = (ahead - d) / span, where d = (p - centre) . dir.
    let col = |a: V3, s: f32| [a.x * s, a.y * s, a.z * s];
    let (r, u, d) = (col(right, 1.0 / half), col(up, 1.0 / half), col(dir, -1.0 / span));
    M4([
        [r[0], u[0], d[0], 0.0],
        [r[1], u[1], d[1], 0.0],
        [r[2], u[2], d[2], 0.0],
        [o.dot(right) / half, o.dot(up) / half, (ahead - o.dot(dir)) / span, 1.0],
    ])
}

/// Whether a sphere at `c` of radius `r` reaches into [`light_box`]'s box (`half` across about
/// `centre`, `back` toward the sun and `ahead` away from it): whether it casts into that map.
pub fn in_box(sun: V3, centre: V3, half: f32, back: f32, ahead: f32, c: V3, r: f32) -> bool {
    let (right, up, dir) = light_axes(sun);
    let o = c.sub(centre);
    o.dot(right).abs() < half + r && o.dot(up).abs() < half + r && o.dot(dir) > -(back + r) && o.dot(dir) < ahead + r
}

/// `centre` moved across the light to the nearest texel of a map `size` texels over `2 * half`
/// feet, so that a box which follows the camera does not make its shadows' edges crawl.
pub fn snapped(sun: V3, centre: V3, half: f32, size: u32) -> V3 {
    let (right, up, _) = light_axes(sun);
    let texel = 2.0 * half / size.max(1) as f32;
    let snap = |a: V3| {
        let d = centre.dot(a);
        ((d / texel).round() * texel - d, a)
    };
    let ((dr, r), (du, u)) = (snap(right), snap(up));
    centre.add(r.scale(dr)).add(u.scale(du))
}

/// The sphere round the stretch of a view from `near` to `far` feet along its look, for a vertical
/// field of view `fov` and a width over height `aspect`: its centre on the look, and its radius.
pub fn slice_sphere(eye: V3, forward: V3, fov: f32, aspect: f32, near: f32, far: f32) -> (V3, f32) {
    let t = (fov * 0.5).tan();
    let t2 = t * t * (1.0 + aspect * aspect);
    // The centre on the axis at z, as near both the near corners and the far ones as it can be.
    let z = ((far * far - near * near) * (1.0 + t2) / (2.0 * (far - near).max(1e-3))).clamp(near, far);
    let corner = |d: f32| ((d - z).powi(2) + d * d * t2).sqrt();
    (eye.add(forward.norm().scale(z)), corner(near).max(corner(far)))
}

/// Our shadows' two maps: everything to `CASCADES[0]` feet from the eye in the sharp one, to
/// `CASCADES[1]` in the wide one, fading out over the last [`REACH_FADE`] of it.
pub const CASCADES: [f32; 2] = [45.0, 300.0];

/// The share of a map, out to its edge across the light, over which it hands over to the next one:
/// V108 switched maps at the sharp one's edge, and a shadow crossing it popped from one map's
/// texels, bias and filter to the other's (user 2026-10-29, "pop in"). The shader has the same rule.
pub const CASCADE_BLEND: f32 = 0.2;

/// The share of our shadows' reach over which they fade out, smoothly (V108 faded the last fifth,
/// linearly).
pub const REACH_FADE: f32 = 0.3;

/// How much of the sharp map a point `edge` across it takes (the larger of its |x| and |y| in the
/// map, 0 mid-map, 1 at the edge), the next map taking the rest; the last map takes all of it.
pub fn cascade_share(edge: f32, last: bool) -> f32 {
    if last { 1.0 } else { 1.0 - ((edge - (1.0 - CASCADE_BLEND)) / CASCADE_BLEND).clamp(0.0, 1.0) }
}

/// How much of our shadow is drawn `distance` feet from the eye, our maps reaching `reach`.
pub fn reach_fade(distance: f32, reach: f32) -> f32 {
    1.0 - smoothstep(reach * (1.0 - REACH_FADE), reach, distance)
}

/// How far toward the sun a map takes casters in from beyond its own sphere: a hill or a tall tree
/// standing between the sun and what is seen.
pub const REACH_BACK: f32 = 600.0;

/// Spheres that touch made one (the original's casters: a bike and the rider on it are a racer's
/// one shadow, each racer its own): `(centre, radius)` in, the merged ones out, each with the
/// indices of the spheres it holds.
pub fn groups(spheres: &[(V3, f32)]) -> Vec<(V3, f32, Vec<usize>)> {
    let mut out: Vec<(V3, f32, Vec<usize>)> = spheres.iter().enumerate().map(|(i, &(c, r))| (c, r, vec![i])).collect();
    loop {
        let mut joined = false;
        'scan: for a in 0..out.len() {
            for b in a + 1..out.len() {
                let (ca, ra) = (out[a].0, out[a].1);
                let (cb, rb) = (out[b].0, out[b].1);
                let d = cb.sub(ca).len();
                if d < ra + rb {
                    // The smallest sphere round both.
                    let (c, r) = if d + rb <= ra {
                        (ca, ra)
                    } else if d + ra <= rb {
                        (cb, rb)
                    } else {
                        let r = (d + ra + rb) / 2.0;
                        (ca.add(cb.sub(ca).scale((r - ra) / d)), r)
                    };
                    let mut held = std::mem::take(&mut out[b].2);
                    out[a].2.append(&mut held);
                    out[a].0 = c;
                    out[a].1 = r;
                    out.remove(b);
                    joined = true;
                    break 'scan;
                }
            }
        }
        if !joined {
            return out;
        }
    }
}

/// The level a light map stands at in its own shadows, and that level over its lit one: the 5th
/// and 50th percentiles of its red channel, in the light map's own units (128 is 1). Our shadows
/// darken the ground to the first, and what has no light map by the second.
pub fn light_map_levels(tiles: &[crate::pst::Texture]) -> Option<(f32, f32)> {
    let mut h = [0u64; 256];
    for t in tiles {
        for p in t.rgba.chunks_exact(4) {
            h[p[0] as usize] += 1;
        }
    }
    let n: u64 = h.iter().sum();
    if n == 0 {
        return None;
    }
    let at = |q: f64| {
        let mut a = 0;
        for (i, c) in h.iter().enumerate() {
            a += c;
            if a as f64 >= q * n as f64 {
                return i as f32;
            }
        }
        255.0
    };
    let (dark, mid) = (at(0.05), at(0.5));
    (mid > 0.0).then(|| (dark / 128.0, (dark / mid).min(1.0)))
}

/// What our shadow darkens to where there is no light map to match: the share of the light the
/// light maps keep in their shadows, measured on free06, free09 and free10 (80 against 144).
pub const SHADE_DEFAULT: f32 = 80.0 / 144.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn the_settings_words_go_round() {
        for s in Shadows::ALL {
            assert_eq!(Shadows::from_word(s.word()), Some(s));
        }
        assert_eq!(Shadows::default(), Shadows::Better);
        assert_eq!(Shadows::from_word("soft"), None);
    }

    #[test]
    fn the_originals_shadow_fades_from_sixty_to_a_hundred_feet() {
        assert_eq!(original_fade(10.0, 1), 1.0);
        assert_eq!(original_fade(60.0, 1), 1.0);
        assert!(near(original_fade(80.0, 1), 0.5));
        assert_eq!(original_fade(100.0, 1), 0.0);
        assert_eq!(original_fade(f32::NAN, 1), 0.0);
        assert!(near(original_fade(60.0, 2), 0.5));
    }

    #[test]
    fn the_tint_is_the_scenes_or_the_games_default() {
        let t = Tint::of(Some([25.0, 25.0, 25.0]), Some(0.7));
        assert_eq!(t.rgb, [25.0; 3]);
        assert!(near(t.alpha, 0.7));
        assert_eq!(Tint::of(None, Some(-1.0)), Tint::default());
        assert!(near(Tint::default().alpha, 85.0 / 128.0));
    }

    #[test]
    fn a_light_box_puts_its_centre_mid_map_and_the_sun_side_nearer() {
        let sun = v3(-0.4, -0.8, 0.3).norm();
        let centre = v3(1000.0, 50.0, -300.0);
        let origin = v3(990.0, 45.0, -290.0);
        let m = light_box(sun, centre, 10.0, 30.0, 70.0, origin);
        let c = m.clip(centre.sub(origin));
        assert!(near(c[0], 0.0) && near(c[1], 0.0) && near(c[2], 0.7) && near(c[3], 1.0), "{c:?}");
        // Toward the sun is nearer: deeper in the reversed depth.
        let toward = m.clip(centre.sub(sun.scale(20.0)).sub(origin));
        assert!(toward[2] > c[2]);
        assert!(near(m.clip(centre.sub(sun.scale(30.0)).sub(origin))[2], 1.0));
        assert!(near(m.clip(centre.add(sun.scale(70.0)).sub(origin))[2], 0.0));
        // Along the light, x and y do not move; across it they reach the edge at `half`.
        assert!(near(toward[0], 0.0) && near(toward[1], 0.0));
        let (right, up, _) = light_axes(sun);
        assert!(near(m.clip(centre.add(right.scale(10.0)).sub(origin))[0], 1.0));
        assert!(near(m.clip(centre.add(up.scale(-10.0)).sub(origin))[1], -1.0));
    }

    #[test]
    fn a_box_takes_what_reaches_into_it_from_the_sun_side() {
        let sun = v3(0.3, -1.0, 0.2).norm();
        let centre = v3(0.0, 0.0, 0.0);
        let (right, _, _) = light_axes(sun);
        assert!(in_box(sun, centre, 10.0, 100.0, 20.0, centre, 1.0));
        // A tree 90 feet up the sun's way still casts in; one 90 feet down it does not.
        assert!(in_box(sun, centre, 10.0, 100.0, 20.0, centre.sub(sun.scale(90.0)), 1.0));
        assert!(!in_box(sun, centre, 10.0, 100.0, 20.0, centre.add(sun.scale(90.0)), 1.0));
        // Across, its own radius counts.
        assert!(in_box(sun, centre, 10.0, 100.0, 20.0, right.scale(12.0), 3.0));
        assert!(!in_box(sun, centre, 10.0, 100.0, 20.0, right.scale(14.0), 3.0));
    }

    #[test]
    fn a_snapped_centre_moves_less_than_a_texel_and_only_across_the_light() {
        let sun = v3(0.2, -1.0, 0.1).norm();
        let c = v3(123.456, 7.0, -89.01);
        let s = snapped(sun, c, 40.0, 2048);
        let moved = s.sub(c);
        assert!(moved.len() <= 2.0 * 40.0 / 2048.0, "{moved:?}");
        assert!(moved.dot(sun).abs() < 1e-3);
        // And twice is once.
        let again = snapped(sun, s, 40.0, 2048);
        assert!(again.sub(s).len() < 1e-3);
    }

    #[test]
    fn a_slice_sphere_holds_its_slices_corners() {
        let (eye, fwd) = (v3(5.0, 2.0, 1.0), v3(0.0, 0.0, 1.0));
        let (fov, aspect) = (1.2f32, 16.0 / 9.0);
        for (n, f) in [(0.0, 45.0), (0.0, 300.0), (20.0, 300.0)] {
            let (c, r) = slice_sphere(eye, fwd, fov, aspect, n, f);
            let t = (fov / 2.0).tan();
            for d in [n, f] {
                for (sx, sy) in [(-1.0, -1.0), (1.0, 1.0), (1.0, -1.0)] {
                    let p = eye.add(v3(sx * d * t * aspect, sy * d * t, d));
                    assert!(p.sub(c).len() <= r + 1e-3, "{n}..{f}: {p:?} outside {c:?} {r}");
                }
            }
        }
    }

    /// The sun `elevation` degrees over the horizon, its light going `heading` degrees round.
    fn sun_at(elevation: f32, heading: f32) -> V3 {
        let (e, h) = (elevation.to_radians(), heading.to_radians());
        v3(e.cos() * h.sin(), -e.sin(), e.cos() * h.cos())
    }

    #[test]
    fn a_long_shadows_footprint_lies_inside_the_originals_box() {
        // A racer's sphere `height` feet over flat ground, the sun low or high: every ground point
        // whose ray to the sun passes through the sphere is inside the box, and whole (past the fade)
        // while the drop is no more than ORIGINAL_WHOLE.
        let r = 5.0;
        for elevation in [6.0, 10.0, 15.0, 30.0, 45.0, 70.0, 89.0] {
            for height in [3.0, 10.0, 30.0, 60.0, 100.0, 115.0] {
                let sun = sun_at(elevation, 40.0);
                let centre = v3(100.0, height, -50.0);
                let (half, back, ahead, fade) = original_box(sun, r);
                let m = light_box(sun, centre, half, back, ahead, v3(0.0, 0.0, 0.0));
                let (right, up, dir) = light_axes(sun);
                for k in 0..64 {
                    // A point of the sphere's outline across the light, cast along it to y = 0.
                    let a = k as f32 / 64.0 * std::f32::consts::TAU;
                    let q = centre.add(right.scale(0.98 * r * a.cos())).add(up.scale(0.98 * r * a.sin()));
                    // Ground on the sun's side of a sphere that dips under it is lit: nothing of the
                    // caster stands between it and the sun.
                    if q.y <= 0.0 {
                        continue;
                    }
                    let p = q.add(dir.scale(-q.y / dir.y));
                    assert!(p.y.abs() < 1e-2);
                    let c = m.clip(p);
                    assert!(c[0].abs() < 1.0 && c[1].abs() < 1.0, "{elevation}° {height} ft: {c:?} off the map");
                    assert!(c[2] > fade && c[2] < 1.0, "{elevation}° {height} ft: depth {} not past the fade {fade}", c[2]);
                }
            }
        }
        // V108's box, 40 ft past the sphere: the far tip of a bike's shadow 10 ft up under a 15° sun
        // lay outside it.
        let sun = sun_at(15.0, 0.0);
        let e = 15.0f32.to_radians();
        assert!((10.0 + r * e.cos()) / e.sin() > r + 40.0);
        // Gone by ORIGINAL_DROP, and no deeper than the lowest sun allows.
        let (_, back, ahead, _) = original_box(sun_at(1.0, 0.0), r);
        assert!(back + ahead < (ORIGINAL_DROP + r) / ORIGINAL_LOW_SUN + 3.0 * r + 10.0);
        let _ = sun;
    }

    #[test]
    fn a_caster_high_over_the_ground_still_casts_into_our_maps() {
        // The camera over the dirt looking along it; a bike 60 ft up over a point 30 ft ahead, under
        // a 12° sun: its shadow lands some 280 ft further along, and the map whose box holds that
        // point holds the bike too.
        let (eye, fwd) = (v3(0.0, 6.0, 0.0), v3(0.0, -0.1, 1.0).norm());
        let sun = sun_at(12.0, 0.0);
        let bike = v3(0.0, 50.0, 30.0);
        let ground = bike.add(sun.scale(-bike.y / sun.y));
        assert!(ground.z > 250.0 && ground.z < CASCADES[1], "{ground:?}");
        let mut held = 0;
        for (near, far) in [(0.0, CASCADES[0]), (CASCADES[0] * 0.8, CASCADES[1])] {
            let (c, r) = slice_sphere(eye, fwd, 1.2, 16.0 / 9.0, near, far);
            let (back, ahead) = (r + REACH_BACK, r);
            if in_box(sun, c, r, back, ahead, ground, 0.0) {
                held += 1;
                assert!(in_box(sun, c, r, back, ahead, bike, 3.0), "{near}..{far} holds the shadow but not the bike");
            }
        }
        assert!(held > 0, "no map holds the shadow at {ground:?}");
    }

    #[test]
    fn the_maps_hand_over_and_fade_without_a_step() {
        // Across the sharp map's last fifth its share falls from 1 to 0 with no jump; the last map
        // takes all; the reach fades smoothly to nothing.
        assert_eq!(cascade_share(0.0, false), 1.0);
        assert!(cascade_share(1.0, false) < 1e-6);
        assert_eq!(cascade_share(0.99, true), 1.0);
        let steps: Vec<f32> = (0..=100).map(|i| cascade_share(i as f32 / 100.0, false)).collect();
        assert!(steps.windows(2).all(|w| w[1] <= w[0] && w[0] - w[1] < 0.06), "{steps:?}");
        assert_eq!(reach_fade(0.0, 300.0), 1.0);
        assert!(reach_fade(300.0, 300.0) < 1e-6);
        let fades: Vec<f32> = (0..=300).map(|d| reach_fade(d as f32, 300.0)).collect();
        assert!(fades.windows(2).all(|w| w[1] <= w[0] && w[0] - w[1] < 0.02));
        // The port draws the original's shadow to 300 ft, smoothly, not the original's 100.
        assert_eq!(ported_fade(100.0), 1.0);
        assert!(ported_fade(255.0) > 0.0 && ported_fade(255.0) < 1.0);
        assert_eq!(ported_fade(300.0), 0.0);
        assert_eq!(ported_fade(f32::NAN), 0.0);
    }

    #[test]
    fn touching_spheres_make_one_caster() {
        let g = groups(&[(v3(0.0, 0.0, 0.0), 4.0), (v3(1.0, 1.0, 0.0), 3.0), (v3(50.0, 0.0, 0.0), 4.0)]);
        assert_eq!(g.len(), 2);
        let joined = g.iter().find(|x| x.2.len() == 2).expect("the bike and its rider");
        for (c, r) in [(v3(0.0, 0.0, 0.0), 4.0), (v3(1.0, 1.0, 0.0), 3.0)] {
            assert!(c.sub(joined.0).len() + r <= joined.1 + 1e-3);
        }
    }
}
