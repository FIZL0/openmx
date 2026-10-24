//! Which materials a track paints with, and which of them come out untextured and why.
//!
//! ```sh
//! cargo run --release -p game-render --bin materials -- free09 sx01
//! cargo run --release -p game-render --bin materials -- --all
//! ```
//!
//! For every model a track's scene places, every material its meshes name is followed the way
//! [`game_render::world::Assets`] follows it — `.psm`, then its texture, then the `.pst` — and the
//! report says where the chain broke. It also prints the `.psm` keys the renderer does not read,
//! which is where multi-pass and blended materials show themselves. Reads the disc named in
//! `kit.toml`; writes nothing.

use game_render::disc::{Disc, Res};
use game_render::psg::Model;
use game_render::rider::{Rider, Sex};
use game_render::scene::{Material, Scene, Vehicle};
use game_render::ini;
use game_render::world::{Assets, decode_texture, texture_files};
use std::collections::BTreeMap;

fn main() {
    if let Err(e) = run() {
        eprintln!("materials: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut tracks: Vec<String> = std::env::args().skip(1).collect();
    let mut disc = Disc::open_configured()?;
    if let Some(at) = tracks.iter().position(|t| t == "--bike") {
        let bike = tracks.get(at + 1).cloned().ok_or("--bike wants a bike, like 250sx")?;
        let rider = tracks.get(at + 2).cloned();
        return actor(&mut disc, &bike, rider.as_deref());
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
    let mut keys: BTreeMap<String, usize> = BTreeMap::new();
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
            println!("{track}: no scene");
            continue;
        };
        let scene = Scene::parse(&ini::text(&archives[0].read(&scn)?));
        let mut names: Vec<String> = scene.models.iter().map(|p| p.model.clone()).collect();
        names.sort();
        names.dedup();
        let mut materials: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut assets = Assets::default();
        for name in &names {
            assets.load(&archives, name);
            let Some(bytes) = archives.iter().find_map(|a| a.read_suffix(&format!("{name}.psg")).ok())
            else {
                continue;
            };
            if let Ok(m) = Model::parse(&bytes) {
                for mat in m.materials {
                    materials.entry(mat).or_default().push(name.clone());
                }
            }
        }
        let mut broken = 0;
        for (mat, users) in &materials {
            let verdict = follow(&archives, mat, &mut keys);
            if verdict.starts_with("ok") {
                continue;
            }
            broken += 1;
            println!("{track}: {mat} ({} models, e.g. {}): {verdict}", users.len(), users[0]);
        }
        println!(
            "{track}: {} models, {} materials, {broken} untextured, {} models missing",
            names.len(),
            materials.len(),
            assets.missing().len()
        );
    }
    println!("psm keys seen: {keys:?}");
    Ok(())
}

/// The bike and rider the way the shell loads them, renames and all: every material slot of every
/// model that ends up with no texture.
fn actor(disc: &mut Disc, bike: &str, rider: Option<&str>) -> Result<(), String> {
    let mut archives = vec![disc.res(&member_ending(disc, &format!("{bike}.res"))?)?];
    if let Some(r) = rider {
        archives.push(disc.res(&member_ending(disc, &format!("{r}.res"))?)?);
    }
    let vehicle = Vehicle::parse(&ini::text(&archives[0].read_suffix(&format!("{bike}.sub"))?));
    let mut assets = Assets::default();
    assets.rename_materials(&vehicle.materials);
    assets.load(&archives, &game_render::vehicle::chassis_model(&archives, &vehicle));
    for part in &vehicle.parts {
        assets.load(&archives, &part.model);
    }
    if let Some(r) = rider {
        let body = Rider::load(&archives, r, Sex::Male)?;
        assets.rename_materials(&body.suit.materials);
        assets.load(&archives, &body.geometry);
    }
    let mut keys = BTreeMap::new();
    for (name, loaded) in &assets.models {
        for (i, mat) in loaded.model.materials.iter().enumerate() {
            let used = loaded.tris.iter().filter(|t| t.material == i).count();
            let verdict = match loaded.textures[i] {
                Some(t) => format!("ok {}x{}", assets.textures[t].width, assets.textures[t].height),
                None => follow(&archives, mat, &mut keys),
            };
            println!("{name}: {mat} ({used} triangles): {verdict}");
        }
    }
    println!("missing: {:?}", assets.missing());
    Ok(())
}

/// Follow a material to its pixels, saying where it stops.
fn follow(archives: &[Res], material: &str, keys: &mut BTreeMap<String, usize>) -> String {
    let Some(psm) = archives.iter().find_map(|a| a.read_suffix(&format!("{material}.psm")).ok()) else {
        return "no .psm".to_string();
    };
    let text = ini::text(&psm);
    for s in ini::parse(&text) {
        for (k, _) in &s.keys {
            *keys.entry(format!("{}.{k}", s.name.trim_end_matches(char::is_numeric))).or_default() += 1;
        }
    }
    let m = Material::parse(&text);
    let files = texture_files(material, &m);
    if files.is_empty() {
        return format!("the .psm names no texture: {}", text.replace(['\r', '\n'], " | "));
    }
    for file in &files {
        let Some(bytes) = archives.iter().find_map(|a| a.read_suffix(file).ok()) else {
            continue;
        };
        return match decode_texture(file, &bytes) {
            Some(t) => format!("ok {}x{} from {file}", t.width, t.height),
            None => format!("{file} does not decode"),
        };
    }
    format!("none of {files:?}")
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
