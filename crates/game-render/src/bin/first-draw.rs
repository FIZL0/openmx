//! Draw one frame of a track, with a bike standing on the start line, from the user's own disc.
//!
//! ```sh
//! cargo run --release -p game-render --bin first-draw -- --out context/shots/v1.png
//! ```
//!
//! Every byte it draws is read at runtime: the ISO's `MX.PSR`, the track's `.res`, the scene, the
//! models the scene places, and their textures. Nothing is written except the PNG, and that goes under
//! `context/`, which is not committed.

use game_render::disc::{Disc, Res};
use game_render::mat::{M4, v3};
use game_render::raster::Frame;
use game_render::rider::{FOOT_ANCHORS, Rider, Sex, Skeleton, bolt_onto};
use game_render::scene::{Preset, Scene, Vehicle};
use game_render::terrain::{Terrain, draw_terrain};
use game_render::world::{Assets, Camera, Lighting, draw_scene};
use game_render::{ini, png};

fn main() {
    if let Err(e) = run() {
        eprintln!("first-draw: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut track = "sx01".to_string();
    let mut bike = "250sx".to_string();
    // The stock gear. Every other rider archive on the disc is a repaint of the same body, and
    // nothing in a scene says which one a race uses — that comes from the player's profile.
    let mut rider_name = "thq1".to_string();
    let mut out = std::path::PathBuf::from("context/shots/first-draw.png");
    let mut width = 640usize;
    let mut height = 448usize;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = |what: &str| args.next().ok_or(format!("{a} wants {what}"));
        match a.as_str() {
            "--track" => track = next("a track name, like sx01")?,
            "--bike" => bike = next("a bike name, like 250sx")?,
            "--rider" => rider_name = next("a rider name, like thq1, or none")?,
            "--out" => out = next("a path")?.into(),
            "--size" => {
                let v = next("WIDTHxHEIGHT")?;
                let (w, h) = v.split_once('x').ok_or("--size wants WIDTHxHEIGHT")?;
                width = w.parse().map_err(|_| "--size: not a width")?;
                height = h.parse().map_err(|_| "--size: not a height")?;
            }
            "--help" | "-h" => {
                println!(
                    "first-draw [--track sx01] [--bike 250sx] [--rider thq1|none] \
                     [--size 640x448] [--out PATH]\n\
                     Draws one frame of a track from the disc named in kit.toml."
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }

    let mut disc = Disc::open_configured()?;
    let track_member = member_ending(&disc, &format!("{track}.res"))?;
    let bike_member = member_ending(&disc, &format!("{bike}.res"))?;
    println!("disc: {track_member}, {bike_member}");
    // A track is two archives: the props and the terrain in one, the bike's animation set, its
    // physics and the follow camera in the other, which the game loads alongside it.
    let second = member_ending(&disc, &format!("{track}_2.res")).ok();
    let mut archives: Vec<Res> = vec![disc.res(&track_member)?, disc.res(&bike_member)?];
    if let Some(name) = &second {
        archives.push(disc.res(name)?);
    }
    // The rider is a fourth archive, named after the rider rather than the track or the bike.
    if rider_name != "none" {
        let member = member_ending(&disc, &format!("{rider_name}.res"))?;
        println!("rider: {member}");
        archives.push(disc.res(&member)?);
    }
    let archives = archives;

    let scn = archives[0]
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))
        .ok_or("the track archive has no .scn")?
        .name
        .clone();
    let scene = Scene::parse(&ini::text(&archives[0].read(&scn)?));
    println!(
        "scene: {} — {} props, start {:?}, terrain {}",
        scene.name,
        scene.models.len(),
        scene.start,
        scene.terrain
    );

    // The ground, and the band its height words live in, which the scene's own props calibrate.
    let terrain = match terrain_bytes(&archives[0], &scene.terrain) {
        Ok(bytes) => match Terrain::parse(&bytes) {
            Ok(mut t) => {
                let fit = t.calibrate(&scene);
                println!(
                    "terrain: {}x{} samples {} apart, ground at {:.2}, {:.1} feet of relief",
                    t.cols,
                    t.rows,
                    t.spacing,
                    t.height_at(scene.start.x, scene.start.z),
                    (t.raw.iter().copied().max().unwrap_or(0) as f32
                        - t.raw.iter().copied().min().unwrap_or(0) as f32)
                        * t.scale
                );
                match fit {
                    Some((points, error)) => println!(
                        "height band: a word is 1/{:.0} of a foot, fitted to {points} props that rest \
                         on the ground, half of them within {error:.2} feet",
                        1.0 / t.scale
                    ),
                    None => println!("height band: not enough props to fit, using the default"),
                }
                Some(t)
            }
            Err(e) => {
                println!("terrain: {e}");
                None
            }
        },
        Err(e) => {
            println!("terrain: {e}");
            None
        }
    };

    let mut assets = Assets::default();
    // The bike's own build: which parts bolt onto the shared chassis, and what this one is painted
    // with. Without the repaint half its meshes ask for a material no archive has.
    let vehicle = match archives[1].read_suffix(&format!("{bike}.sub")) {
        Ok(bytes) => Vehicle::parse(&ini::text(&bytes)),
        Err(e) => {
            println!("bike: {e}");
            Vehicle::default()
        }
    };
    assets.rename_materials(&vehicle.materials);

    let mut names: Vec<String> = scene.models.iter().map(|p| p.model.clone()).collect();
    names.sort();
    names.dedup();
    for name in &names {
        assets.load(&archives, name);
    }
    // The actor: the bike's own model, which carries its wheels and forks as nodes of one tree,
    // and the parts its `.sub` bolts on.
    let chassis_name = "bike";
    let has_bike = assets.load(&archives, chassis_name);
    for part in &vehicle.parts {
        assets.load(&archives, &part.model);
    }
    // The rider on it: one body shared by every rider on the disc, painted in this one's gear. The
    // repaint has to be in before the body is loaded, the same as the bike's.
    let rider = match rider_name.as_str() {
        "none" => None,
        name => match Rider::load(&archives, name, Sex::Male) {
            Ok(r) => {
                assets.rename_materials(&r.suit.materials);
                if assets.load(&archives, &r.geometry) {
                    println!(
                        "rider: {} wearing {}, {} -> {} -> {}.psg, {} motions",
                        r.name,
                        r.suit.base_materials.len(),
                        format_args!("{name}.sub"),
                        r.mcf,
                        r.geometry,
                        r.set.motions.len()
                    );
                    Some(r)
                } else {
                    println!("rider: no {}.psg", r.geometry);
                    None
                }
            }
            Err(e) => {
                println!("rider: {e}");
                None
            }
        },
    };
    println!(
        "loaded: {} of {} models, {} textures{}",
        assets.models.len(),
        names.len(),
        assets.textures.len(),
        if has_bike { ", bike" } else { ", no bike" }
    );
    if let Some(first) = assets.missing().first() {
        println!(
            "missing: {} files, the first being {first}",
            assets.missing().len()
        );
    }

    let dir = scene.start_dir.norm();
    // The start line says where the bike stands, the ground says how high: the scene's own y is a
    // little under the dirt on every track tried.
    let mut stand = scene.start;
    if let Some(t) = &terrain {
        stand.y = t.height_at(stand.x, stand.z);
    }
    // The follow camera the game starts a race on, from the track's own presets.
    let preset = archives
        .iter()
        .find_map(|a| a.read_suffix("cpresets.ini").ok())
        .map(|b| Preset::parse(&ini::text(&b)))
        .unwrap_or_default()
        .into_iter()
        .next();
    let camera = match &preset {
        Some(p) => {
            println!(
                "camera: {} — {} feet back, {} degrees up, {} degrees across",
                p.name,
                p.distance,
                90.0 - p.angle_x,
                p.fov
            );
            Camera::chase(stand, dir, p)
        }
        None => Camera::behind(stand, dir),
    };
    let mut frame = Frame::new(width, height, scene.fog_color);
    let mut drawn = 0;
    if let Some(t) = &terrain {
        drawn += draw_terrain(&mut frame, t, &camera, &Lighting::of(&scene));
    }
    let bike_at = M4::placement(stand, dir, v3(0.0, 1.0, 0.0));
    // Each part rides on one node of the chassis, so its transform is that node's place in the
    // chassis followed by the chassis's place in the world.
    let mut actor: Vec<(&str, M4)> = vec![(chassis_name, bike_at)];
    if let Some(chassis) = assets.models.get(chassis_name) {
        for part in &vehicle.parts {
            let node = chassis
                .model
                .nodes
                .iter()
                .position(|n| n.name.eq_ignore_ascii_case(&part.placement));
            let at = match node {
                Some(i) => chassis.model.world(i).mul(bike_at),
                None => bike_at,
            };
            actor.push((part.model.as_str(), at));
        }
        // The rider stands on the chassis's foot pegs: the two models share four node names and
        // nothing else, and those are the anchors. See `rider.rs` on why the bind pose's placement
        // is a choice and the feet are the pair that suits it.
        if let Some(rider) = &rider {
            if let Some(body) = assets.models.get(&rider.geometry) {
                let bolt = bolt_onto(
                    &Skeleton::of(&body.model),
                    &Skeleton::of(&chassis.model),
                    &FOOT_ANCHORS,
                );
                match bolt {
                    Some(bolt) => actor.push((rider.geometry.as_str(), bolt.mul(bike_at))),
                    None => println!("rider: the chassis has no foot pegs to stand on"),
                }
            }
        }
    }
    drawn += draw_scene(&mut frame, &scene, &assets, &actor, &camera);
    println!("drew {drawn} triangles from {:?}", camera.eye);

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    png::write_rgb(&out, width, height, &frame.color)?;
    println!("wrote {}", out.display());
    Ok(())
}

/// The terrain file a scene asks for. Most scenes name it `SX02.trn`, but what the archive holds is
/// `sx02.ps2.trn`: the loader tags the file with the platform it was built for, and a few scenes
/// (Supercross01 among them) already spell the tag out.
fn terrain_bytes(res: &Res, named: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = res.read_suffix(named) {
        return Ok(bytes);
    }
    let tagged = match named.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
        None => return Err(format!("the archive has no member ending in {named}")),
    };
    res.read_suffix(&tagged)
        .map_err(|_| format!("the archive has no member ending in {named} or {tagged}"))
}

/// The member of `MX.PSR` whose name ends that way — archives are named after the track or bike
/// they hold, in a directory this code does not need to know.
fn member_ending(disc: &Disc, suffix: &str) -> Result<String, String> {
    let want = suffix.to_ascii_lowercase();
    // The shortest match, because the menus keep their own copy of a bike one directory deeper and
    // that copy holds the spinning showroom model rather than the one a race uses.
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .ok_or_else(|| format!("MX.PSR has no member ending in {suffix}"))
}
