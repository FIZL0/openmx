//! V73: nat01's rain — the `.toy`'s `[WeatherBoxToy1]` and the `RainBegins`/`RainEnds` triggers
//! whose `Weather` actions turn it up and down — read off the disc the way the original's weather
//! box toy and its trigger action read them, and run by weather.rs.
//!
//! The disc is not in the repo: without it the tests say so and pass.

#[allow(dead_code)]
#[path = "../src/weather.rs"]
mod weather;

use game_render::Disc;
use game_render::ini;
use game_render::mat::{V3, v3};
use weather::{Rain, Ramp};

/// nat01's `.toy` text and its rain's texture's bytes.
fn nat01() -> Option<(String, Vec<u8>)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = names.iter().find(|n| n.eq_ignore_ascii_case("Tracks\\nat01.res")).cloned().unwrap();
    let res = disc.res(&member).unwrap();
    let toy = ini::text(&res.read_suffix("\\nat01.toy").unwrap());
    let art = res.read_suffix("\\rain_gpu.pst").unwrap();
    Some((toy, art))
}

/// The box and its two triggers, as the disc writes them.
#[test]
fn nat01s_weather_box_and_triggers() {
    let Some((toy, art)) = nat01() else { return };
    let boxes = weather::boxes(&toy);
    assert_eq!(boxes.len(), 1);
    let b = &boxes[0];
    assert_eq!(b.name, "Camera0Weather");
    assert_eq!(b.size, v3(90.0, 45.0, 125.0));
    assert!(b.rain && b.follow_camera && !b.adjust_to_speed);
    assert_eq!((b.particles, b.camera), (800, 0));
    assert_eq!((b.min_velocity, b.max_velocity), (-110.0, -180.0));
    assert_eq!((b.min_size, b.max_size, b.min_life, b.max_life), (1.5, 3.0, 0.2, 0.3));
    assert_eq!((b.min_range, b.max_range, b.aspect, b.tilt), (0.0, 100.0, 6.0, 4.0));
    assert_eq!((b.start_density, b.start_step), (0.0, 0.01));
    assert_eq!(b.material, "rain_gpu");
    let texture = game_render::pst::Texture::parse(&art).unwrap();
    assert!(texture.width > 0 && texture.height > 0);

    let triggers = weather::triggers(&toy);
    let found: Vec<(&str, i32, f32, f32)> = triggers.iter().map(|(t, a)| (t.name.as_str(), t.percent, a[0].density, a[0].step)).collect();
    assert_eq!(found, [("RainBegins", 75, 1.0, 0.01), ("RainEnds", 50, 0.0, 0.01)]);
    for (t, actions) in &triggers {
        assert_eq!(actions.len(), 1);
        let a = &actions[0];
        assert_eq!(a.weather_box, "Camera0Weather");
        assert_eq!((a.aspect, a.min_velocity, a.max_velocity), (0.0, 0.0, 0.0));
        assert!(t.enabled && !t.one_time);
        assert!(t.holds(t.at) && !t.holds(t.at.add(v3(0.0, 0.0, 3.0))), "{} is 2.5 ft thick", t.name);
    }
}

/// A frame of the box: its update, then its draw at `eye`.
fn frame(rain: &mut Rain, eye: V3) {
    rain.update(1.0 / 30.0);
    rain.emit(Some(eye));
}

/// Ride in and out of trigger `name` until it fires: each pass in rolls its percent once.
fn ride_through(rain: &mut Rain, name: &str, eye: V3) -> usize {
    let at = rain.triggers.iter().find(|t| t.0.name == name).unwrap().0.at;
    let away = at.add(v3(0.0, 0.0, 50.0));
    let target = rain.target;
    for pass in 1..=50 {
        rain.touch(&[at]);
        frame(rain, eye);
        rain.touch(&[away]);
        frame(rain, eye);
        if rain.target != target {
            return pass;
        }
    }
    panic!("{name} never fired in 50 passes");
}

/// Dry at the start; RainBegins brings it up a hundredth a frame to all 267 packets live, every drop
/// falling round the camera; RainEnds takes it back down until nothing is live.
#[test]
fn rain_begins_and_ends() {
    let Some((toy, _)) = nat01() else { return };
    let mut rain = Rain::new(weather::boxes(&toy).remove(0), weather::triggers(&toy));
    assert_eq!(rain.triggers.len(), 2);
    assert_eq!(rain.packets.len(), 267);
    let eye = v3(2800.0, 200.0, 2900.0);
    for _ in 0..60 {
        frame(&mut rain, eye);
    }
    assert!(rain.drawn(0.0).is_empty(), "dry before the trigger");

    // A rider standing in it rolls once, on the frame they come in, and not again.
    let begins = rain.triggers[0].0.at;
    rain.touch(&[begins]);
    let rolled = rain.rng;
    for _ in 0..30 {
        rain.touch(&[begins]);
    }
    assert_eq!(rain.rng, rolled);
    rain.touch(&[begins.add(v3(0.0, 0.0, 50.0))]);
    rain.reset();

    ride_through(&mut rain, "RainBegins", eye);
    assert_eq!(rain.target, 1.0);
    let mut frames = 0;
    let mut halved = false;
    while rain.density < 1.0 {
        frame(&mut rain, eye);
        halved |= rain.ramp == Ramp::Rising && rain.life == 0.1;
        frames += 1;
        assert!(frames < 200);
    }
    assert!(halved, "the packets live half as long while it rises");
    assert!((99..=103).contains(&frames), "{frames} frames to full rain at 0.01 a frame");
    for _ in 0..30 {
        frame(&mut rain, eye);
    }
    assert_eq!(rain.ramp, Ramp::Steady);
    assert_eq!(rain.life, 0.2);
    let live = rain.packets.iter().filter(|p| rain.clock < p.until).count();
    assert_eq!(live, 267);
    let drops = rain.drawn(0.0);
    assert!(drops.len() > 700, "{} drops", drops.len());
    for (at, d) in &drops {
        let off = at.sub(eye);
        assert!(off.x.abs() <= 46.0 && off.z.abs() <= 63.0, "{off:?} out of the box");
        assert!(off.y <= 22.5 && off.y > 22.5 - 180.0 * 0.31, "{off:?} out of its fall");
        assert!(d.velocity.y <= -110.0 && d.velocity.y >= -180.0);
        assert_eq!(d.length, d.size * 6.0);
    }
    // A still camera stands the streaks up; a moving one leans them into its way.
    assert_eq!(rain.axis, v3(0.0, 1.0, 0.0));
    frame(&mut rain, eye.add(v3(4.0, 0.0, 0.0)));
    assert!((rain.axis.x - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    let glow = rain.glows(3, eye, 0.0, [1.0; 4]);
    assert!(glow.additive && glow.texture == 3);
    assert_eq!(glow.tris.len(), 2 * rain.drawn(0.0).len());

    ride_through(&mut rain, "RainEnds", eye);
    assert_eq!(rain.target, 0.0);
    for _ in 0..120 {
        frame(&mut rain, eye);
    }
    assert_eq!(rain.density, 0.0);
    assert!(rain.drawn(0.0).is_empty(), "dry after it ends");
}

/// The ramp loses a frame at each turn, as the original's update does.
#[test]
fn the_ramp_turns_a_frame_late() {
    let b = weather::WeatherBox {
        name: "w".into(),
        position: v3(0.0, 0.0, 0.0),
        size: v3(10.0, 10.0, 10.0),
        rain: true,
        particles: 30,
        follow_camera: true,
        camera: 0,
        min_velocity: -110.0,
        max_velocity: -180.0,
        min_size: 1.0,
        max_size: 2.0,
        min_life: 0.2,
        max_life: 0.3,
        min_range: 0.0,
        max_range: 100.0,
        adjust_to_speed: false,
        aspect: 6.0,
        tilt: 4.0,
        start_density: 0.0,
        start_step: 0.25,
        material: "rain_gpu".into(),
    };
    let mut rain = Rain::new(b, Vec::new());
    rain.target = 1.0;
    let mut seen = Vec::new();
    for _ in 0..7 {
        rain.update(1.0 / 30.0);
        seen.push((rain.ramp, rain.density));
    }
    use Ramp::*;
    assert_eq!(seen, [(Rising, 0.0), (Rising, 0.25), (Rising, 0.5), (Rising, 0.75), (Rising, 1.0), (Steady, 1.0), (Steady, 1.0)]);
    rain.act(&weather::Action { weather_box: "w".into(), density: 0.5, step: -1.0, aspect: 0.0, min_velocity: 0.0, max_velocity: 0.0 });
    let mut seen = Vec::new();
    for _ in 0..4 {
        rain.update(1.0 / 30.0);
        seen.push((rain.ramp, rain.density));
    }
    assert_eq!(seen, [(Falling, 1.0), (Falling, 0.75), (Falling, 0.5), (Steady, 0.5)]);
    // New speeds above 0 rebuild the table, first the most, last the least.
    rain.act(&weather::Action { weather_box: "w".into(), density: -1.0, step: -1.0, aspect: 0.0, min_velocity: 30.0, max_velocity: 60.0 });
    assert_eq!((rain.speeds[0], rain.speeds[15]), (60.0, 30.0));
}
