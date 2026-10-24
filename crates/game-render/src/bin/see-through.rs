//! Which materials a track draws see-through, and by which of the `.psm`'s rules (V60).
//!
//! ```sh
//! cargo run --release -p game-render --bin see-through -- free06 sx01
//! cargo run --release -p game-render --bin see-through -- --all
//! ```
//!
//! For every material a track's placed models, its ground and its paths' cars name, it prints the
//! `.psm`'s alpha keys (the blend's four inputs and its fixed alpha, the texture's colour component,
//! the test, the depth keys) next to the texture's alpha, and the [`Surface`] the port draws it with.
//! A line starts with `SEE` where the port lets a share of the backdrop through that the GS would
//! not: a blend whose C input is the fixed alpha (so the texel's alpha does not weigh it), a texture
//! whose alpha the GS is told to ignore, or no `.psm` at all. Reads the disc named in `kit.toml`;
//! writes nothing.

use game_render::disc::{Disc, Res};
use game_render::ini;
use game_render::psg::Model;
use game_render::scene::{Material, Scene};
use game_render::world::{Surface, decode_texture, texture_files};
use std::collections::BTreeMap;

fn main() {
    if let Err(e) = run() {
        eprintln!("see-through: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut tracks: Vec<String> = std::env::args().skip(1).collect();
    let mut disc = Disc::open_configured()?;
    if tracks.first().map(String::as_str) == Some("--show") {
        let [_, track, material] = &tracks[..] else { return Err("--show wants a track and a material".into()) };
        return show(&mut disc, track, material);
    }
    if tracks.first().map(String::as_str) == Some("--where") {
        // Where a track's scene places the models whose names contain a word, to shoot them.
        let [_, track, word] = &tracks[..] else { return Err("--where wants a track and a word".into()) };
        let res = disc.res(&member_ending(&disc, &format!("{track}.res"))?)?;
        let scn = res.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn")).ok_or("no scene")?;
        let scene = Scene::parse(&ini::text(&res.read(&scn.name.clone())?));
        for p in scene.models.iter().filter(|p| p.model.to_ascii_lowercase().contains(&word.to_ascii_lowercase())) {
            println!("{} at {:.0},{:.0},{:.0}", p.model, p.pos.x, p.pos.y, p.pos.z);
        }
        println!("start {:.0},{:.0},{:.0}", scene.start.x, scene.start.y, scene.start.z);
        return Ok(());
    }
    if tracks.first().map(String::as_str) == Some("--list") {
        for e in disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")) {
            println!("{}", e.name);
        }
        return Ok(());
    }
    if tracks.first().map(String::as_str) == Some("--res") {
        // Every material in an archive, placed or not: the bikes' and the riders'.
        for name in &tracks[1..] {
            let res = disc.res(&member_ending(&disc, &format!("{name}.res"))?)?;
            let mut mats: Vec<String> = res
                .entries()
                .iter()
                .filter_map(|e| {
                    let n = e.name.replace('\\', "/");
                    n.rsplit('/').next()?.strip_suffix(".psm").map(str::to_string)
                })
                .collect();
            mats.sort();
            mats.dedup();
            for m in mats {
                println!("{name} {}", report(std::slice::from_ref(&res), &m, "-", 128, &[]));
            }
        }
        return Ok(());
    }
    if tracks.iter().any(|t| t == "--all") {
        tracks = disc
            .entries()
            .iter()
            .filter_map(|e| {
                let n = e.name.to_ascii_lowercase().replace('\\', "/");
                let base = n.rsplit('/').next()?.strip_suffix(".res")?.to_string();
                n.contains("tracks").then_some(base)
            })
            .filter(|b| !b.ends_with("_2"))
            .collect();
        tracks.sort();
        tracks.dedup();
    }
    for track in &tracks {
        let Ok(member) = member_ending(&disc, &format!("{track}.res")) else {
            println!("{track}: not on the disc");
            continue;
        };
        let mut archives = vec![disc.res(&member)?];
        if let Ok(second) = member_ending(&disc, &format!("{track}_2.res")) {
            archives.push(disc.res(&second)?);
        }
        let Some(scn) = archives[0]
            .entries()
            .iter()
            .find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))
            .map(|e| e.name.clone())
        else {
            continue;
        };
        let scene = Scene::parse(&ini::text(&archives[0].read(&scn)?));
        let mut names: Vec<String> = scene.models.iter().map(|p| p.model.clone()).collect();
        names.sort();
        names.dedup();
        // Material -> (a model using it, the least vertex alpha its meshes carry).
        let mut materials: BTreeMap<String, (String, u8, Vec<[[f32; 2]; 3]>)> = BTreeMap::new();
        for name in &names {
            let Some(bytes) = archives.iter().find_map(|a| a.read_suffix(&format!("{name}.psg")).ok()) else {
                continue;
            };
            let Ok(m) = Model::parse(&bytes) else { continue };
            for mesh in &m.meshes {
                let least = mesh.strips.iter().flat_map(|s| s.verts.iter()).map(|v| v.color[3]).min().unwrap_or(128);
                let e = materials.entry(m.materials[mesh.material].clone()).or_insert((name.clone(), 255, Vec::new()));
                e.1 = e.1.min(least);
            }
            for t in m.triangles(0) {
                if let Some(e) = materials.get_mut(&m.materials[t.material]) {
                    e.2.push([t.v[0].uv, t.v[1].uv, t.v[2].uv]);
                }
            }
        }
        for (mat, (user, vertex_alpha, uvs)) in &materials {
            println!("{track} {}", report(&archives, mat, user, *vertex_alpha, uvs));
        }
    }
    Ok(())
}

fn report(archives: &[Res], material: &str, user: &str, vertex_alpha: u8, uvs: &[[[f32; 2]; 3]]) -> String {
    let Some(psm) = archives.iter().find_map(|a| a.read_suffix(&format!("{material}.psm")).ok()) else {
        return format!("SEE? {material} ({user}): no .psm, drawn with Surface::default (cut under 32)");
    };
    let text = ini::text(&psm);
    let sections = ini::parse(&text);
    let passes = sections
        .iter()
        .filter(|s| s.name.to_ascii_lowercase().starts_with("shaderpass") && !s.name.to_ascii_lowercase().contains("animator"))
        .count();
    let pass = sections.iter().find(|s| s.name.to_ascii_lowercase().starts_with("shaderpass"));
    // A pass of two texture maps reads its first texture's keys suffixed `_1`, as Material::parse.
    let two = pass.and_then(|s| s.number("totalTextureMaps")) == Some(2.0);
    let key = |k: &str| {
        let k = match two && !k.ends_with("Name") && k != "textureColorComponent" {
            true => format!("{k}_1"),
            false => k.to_string(),
        };
        pass.and_then(|s| s.get(&k)).map(|v| v.trim().to_string()).unwrap_or_else(|| "-".into())
    };
    let m = Material::parse(&text);
    let surface = Surface::of(&m);
    let texture = texture_files(material, &m).into_iter().find_map(|f| {
        let bytes = archives.iter().find_map(|a| a.read_suffix(&f).ok())?;
        decode_texture(&f, &bytes)
    });
    let (least, part, holes) = match &texture {
        Some(t) => {
            let n = (t.rgba.len() / 4).max(1) as f32;
            let alphas = || t.rgba.chunks(4).map(|p| p[3]);
            (
                alphas().min().unwrap_or(255),
                alphas().filter(|&a| a < 255).count() as f32 / n,
                alphas().filter(|&a| a < surface.cutoff).count() as f32 / n,
            )
        }
        None => (255, 0.0, 0.0),
    };
    let c = key("inputAlphaC");
    let fix = key("fixedAlphaValue");
    let tcc = match two {
        true => pass.and_then(|s| s.get("textureColorComponent_1")).map(|v| v.trim().to_string()).unwrap_or_else(|| "-".into()),
        false => key("textureColorComponent"),
    };
    let mut why = Vec::new();
    if surface.blend && c == "2" && part > 0.0 {
        why.push(format!("blend C is FIX {fix}, the port weighs by the texel's alpha"));
    }
    if tcc == "0" && part > 0.0 {
        why.push("texture colour component RGB: the GS ignores the texel's alpha".to_string());
    }
    if !surface.blend && holes > 0.0 {
        let mapped = texture.as_ref().map(|t| mapped_cut(t, uvs, surface.cutoff)).unwrap_or((0, 0));
        why.push(format!(
            "{:.1}% of texels cut, {:.1}% of the {} texels the model maps",
            holes * 100.0,
            100.0 * mapped.1 as f32 / mapped.0.max(1) as f32,
            mapped.0
        ));
    }
    format!(
        "{}{material} ({user}) {} passes {passes} | blend {} A{} B{} C{c} D{} fix {fix} | tcc {tcc} tfx {} | test {} m{} r{} f{} | zw-off {} ztest {} | tex min {least} part {:.1}% | vtx a {vertex_alpha} | port cut {} dcut {} blend {} add {} dw {}{}",
        if why.is_empty() { "     " } else { "SEE  " },
        key("microProgramName"),
        key("alphaBlendingOnOff"),
        key("inputColorA"),
        key("inputColorB"),
        key("inputColorD"),
        key("textureFunction"),
        key("alphaTestOnOff"),
        key("alphaTestMethod"),
        key("alphaReferenceValue"),
        key("processingMethodWhenFailedInAlphaTest"),
        key("zWriteDisabled"),
        key("depthTestMethod"),
        part * 100.0,
        surface.cutoff,
        surface.depth_cutoff,
        surface.blend,
        surface.additive,
        surface.depth_write,
        if why.is_empty() { String::new() } else { format!(" <- {}", why.join("; ")) },
    )
}

/// Of the texels a model's triangles cover (wrapped into the texture), how many there are and how
/// many fall under `cutoff`.
fn mapped_cut(t: &game_render::Texture, uvs: &[[[f32; 2]; 3]], cutoff: u8) -> (usize, usize) {
    let (w, h) = (t.width, t.height);
    let mut covered = vec![false; w * h];
    for tri in uvs {
        let p: Vec<[f32; 2]> = tri.iter().map(|uv| [uv[0] * w as f32, uv[1] * h as f32]).collect();
        let (x0, x1) = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min), p.iter().map(|q| q[0]).fold(f32::MIN, f32::max));
        let (y0, y1) = (p.iter().map(|q| q[1]).fold(f32::MAX, f32::min), p.iter().map(|q| q[1]).fold(f32::MIN, f32::max));
        if (x1 - x0) * (y1 - y0) > 16.0 * (w * h) as f32 {
            continue;
        }
        let edge = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let area = edge(p[0], p[1], p[2]);
        for y in y0.floor() as i64..=y1.ceil() as i64 {
            for x in x0.floor() as i64..=x1.ceil() as i64 {
                let c = [x as f32 + 0.5, y as f32 + 0.5];
                let (a, b, d) = (edge(p[1], p[2], c), edge(p[2], p[0], c), edge(p[0], p[1], c));
                let inside = if area >= 0.0 { a >= 0.0 && b >= 0.0 && d >= 0.0 } else { a <= 0.0 && b <= 0.0 && d <= 0.0 };
                if inside {
                    covered[y.rem_euclid(h as i64) as usize * w + x.rem_euclid(w as i64) as usize] = true;
                }
            }
        }
    }
    let n = covered.iter().filter(|&&c| c).count();
    let cut = covered.iter().enumerate().filter(|&(i, &c)| c && t.rgba[4 * i + 3] < cutoff).count();
    (n, cut)
}

/// One material's `.psm` whole, and its texture's alpha by value, in the decoder's 0..=255.
fn show(disc: &mut Disc, track: &str, material: &str) -> Result<(), String> {
    let mut archives = vec![disc.res(&member_ending(disc, &format!("{track}.res"))?)?];
    if let Ok(second) = member_ending(disc, &format!("{track}_2.res")) {
        archives.push(disc.res(&second)?);
    }
    let psm = archives
        .iter()
        .find_map(|a| a.read_suffix(&format!("{material}.psm")).ok())
        .ok_or("no .psm")?;
    let text = ini::text(&psm);
    println!("{text}");
    let m = Material::parse(&text);
    for f in texture_files(material, &m) {
        let Some(t) = archives.iter().find_map(|a| a.read_suffix(&f).ok()).and_then(|b| decode_texture(&f, &b)) else {
            continue;
        };
        let mut by: BTreeMap<u8, usize> = BTreeMap::new();
        for p in t.rgba.chunks(4) {
            *by.entry(p[3]).or_default() += 1;
        }
        println!("{f} {}x{}: alpha {by:?}", t.width, t.height);
        break;
    }
    Ok(())
}

/// The member of `MX.PSR` whose name ends that way, the shortest if several do.
fn member_ending(disc: &Disc, suffix: &str) -> Result<String, String> {
    let want = suffix.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .ok_or_else(|| format!("MX.PSR has no member ending in {suffix}"))
}
