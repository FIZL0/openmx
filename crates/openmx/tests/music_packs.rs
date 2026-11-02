//! **MD7:** music packs. A pack written here — a generated `.vig` and a `mod.json` playlist in
//! openmx-mods' standard §4 — is read beside the disc's songs, plays, and shows its title with its
//! game; the Audio page's Music From and each pack's row choose which games' music plays; a pack the
//! folder holds badly is one line, not a crash; and with no pack the songs are the disc's alone, their
//! labels as before. The disc's half skips without a disc.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/dress.rs"]
#[allow(dead_code)]
mod dress;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;
#[path = "../src/music_packs.rs"]
#[allow(dead_code)]
mod music_packs;

use audio::{MusicSource, Pick, Track};
use menu::{Controls, Key, Menu, Screen, Settings};
use music_packs::PackMusic;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openmx-md7-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `.vig` as the discs have them: a 0x800-byte header with the per-channel body size at 0x0c and the
/// rate at 0x10 (big-endian), then left and right blocks in turn. Each block is a square wave at full
/// nibble: filter 0, shift 0, so a nibble of 7 is 7 << 12.
fn vig(pairs: usize, rate: u32) -> Vec<u8> {
    let mut b = vec![0u8; 0x800];
    b[0..4].copy_from_slice(b"VAGp");
    b[0x0c..0x10].copy_from_slice(&((pairs * 16) as u32).to_be_bytes());
    b[0x10..0x14].copy_from_slice(&rate.to_be_bytes());
    for _ in 0..pairs * 2 {
        let mut block = [0u8; 16];
        for (i, x) in block[2..].iter_mut().enumerate() {
            *x = if i % 4 < 2 { 0x77 } else { 0x99 };
        }
        b.extend_from_slice(&block);
    }
    b
}

/// A music pack in `mods/music/<id>/`: `mod.json` and each song's stream at `Aud/Music/<stem>.vig`.
fn write_pack(mods: &Path, id: &str, game: &str, songs: &[(&str, &str, &str)]) {
    let dir = mods.join("music").join(id);
    std::fs::create_dir_all(dir.join("Aud/Music")).unwrap();
    let mut list = Vec::new();
    for (n, (stem, title, artist)) in songs.iter().enumerate() {
        std::fs::write(dir.join(format!("Aud/Music/{stem}.vig")), vig(2000, 44100)).unwrap();
        list.push(format!(
            r#"{{"file": "Aud/Music/{stem}.vig", "title": "{title}", "artist": "{artist}", "from": "Aud\\Music\\music.pb [Song {}]"}}"#,
            n + 1
        ));
    }
    let json = format!(
        r#"{{"standard": 1, "kind": "music", "id": "{id}", "name": "{game} soundtrack",
  "source": {{"game": "{game}", "boot": "SLUS_000.00", "members": []}},
  "version": "1.0.0", "author": "openmx tests/music_packs.rs", "permission": "own-disc",
  "songs": [{}], "todo": []}}"#,
        list.join(", ")
    );
    std::fs::write(dir.join("mod.json"), json).unwrap();
}

/// The disc's songs as [`audio::DiscMusic`] lists them, for the half that needs no disc: named
/// `disc`, MX Unleashed's, each a quiet hum.
struct Disc;

struct Hum(usize);

impl audio::Stream for Hum {
    fn rate(&self) -> u32 {
        audio::RATE
    }
    fn read(&mut self, out: &mut [i16]) -> usize {
        let n = (out.len() / 2).min(self.0);
        out[..n * 2].fill(10);
        self.0 -= n;
        n
    }
}

impl MusicSource for Disc {
    fn name(&self) -> &str {
        "disc"
    }
    fn tracks(&self) -> Vec<Track> {
        vec![Track { id: "disc:breakdown".into(), title: "Breakdown".into(), artist: "Disc Band".into() }]
    }
    fn open(&self, _id: &str) -> Result<Box<dyn audio::Stream>, String> {
        Ok(Box::new(Hum(audio::RATE as usize * 60)))
    }
    fn game(&self, _id: &str) -> Option<String> {
        Some(audio::DISC_GAME.into())
    }
}

fn ids(tracks: &[Track]) -> Vec<String> {
    tracks.iter().map(|t| t.id.clone()).collect()
}

#[test]
fn a_written_pack_plays_beside_the_discs_songs_and_shows_its_title_with_its_game() {
    let mods = scratch("plays");
    write_pack(&mods, "mxvatv_music", "MX vs. ATV Unleashed", &[("PAPA_ROACH_Murder", "Scars", "Papa Roach"), ("PILLAR_Bring", "Bring Me Down", "Pillar")]);
    write_pack(&mods, "untamed_music", "MX vs. ATV Untamed", &[("SONG_A", "Song A", "Band A")]);
    let (packs, log) = PackMusic::read(Some(&mods));
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(packs.len(), 3);
    assert_eq!(
        ids(&packs.tracks()),
        ["mod:mxvatv_music/papa_roach_murder", "mod:mxvatv_music/pillar_bring", "mod:untamed_music/song_a"],
        "packs in id order, each in its playlist's order, ids as the standard's §4 says"
    );

    let mut music = audio::Music::new(vec![Arc::new(Disc), Arc::new(packs)], 3);
    music.wait = true;
    assert_eq!(music.tracks().len(), 4, "the disc's song first, then the packs'");
    assert_eq!(music.tracks()[0].id, "disc:breakdown");
    let labels = music.labels();
    assert_eq!(labels[0].1, "Breakdown - Disc Band (MX Unleashed)");
    assert_eq!(labels[1], ("mod:mxvatv_music/papa_roach_murder".into(), "Scars - Papa Roach (MX vs. ATV Unleashed)".into()));
    assert_eq!(labels[3].1, "Song A - Band A (MX vs. ATV Untamed)");

    // The pack's song plays: the speakers' mix carries its square wave, and the menu names it.
    let speakers = audio::Speakers::start(music, false);
    speakers.apply("shuffle", levels());
    speakers.play_now("mod:mxvatv_music/papa_roach_murder");
    let mut out = vec![0i16; audio::RATE as usize / 10 * 2];
    speakers.mixer().lock().unwrap().render(&mut out);
    assert_eq!(speakers.now().as_deref(), Some("mod:mxvatv_music/papa_roach_murder"));
    let loud = out.iter().map(|s| (*s as i32).abs()).max().unwrap();
    assert!(loud > 1000, "the pack's song is heard: peak {loud}");

    let mut m = Menu::new(Settings::default(), Controls::default());
    m.music_choices = speakers.choices();
    m.music_now = speakers.now();
    m.open_tab(Screen::Settings, audio_tab());
    let skip = m.page().into_iter().find(|r| r.key == Key::SkipSong).unwrap();
    assert_eq!(skip.value, "Scars - Papa Roach (MX vs. ATV Unleashed)", "the song playing shows with its game");

    // Every song in order plays through the packs' too, and their slider is User Soundtracks'.
    speakers.apply("order", levels());
    assert_eq!(*speakers.mixer().lock().unwrap().music.pick(), Pick::Order);
    let gain = speakers.mixer().lock().unwrap().music_gain();
    assert_eq!(gain, audio::volume(levels().folder));
    std::fs::remove_dir_all(&mods).unwrap();
}

fn levels() -> audio::Levels {
    audio::Levels { music: 70, folder: 50, effects: 80, crowd: 80, engine: 80, menu: 80 }
}

fn audio_tab() -> usize {
    menu::SETTINGS_TABS.iter().position(|t| *t == "Audio").unwrap()
}

#[test]
fn the_settings_choose_which_games_music_plays() {
    let mods = scratch("filter");
    write_pack(&mods, "mxvatv_music", "MX vs. ATV Unleashed", &[("A", "A", "")]);
    write_pack(&mods, "untamed_music", "MX vs. ATV Untamed", &[("B", "B", "")]);
    let (packs, _) = PackMusic::read(Some(&mods));
    let mut music = audio::Music::new(vec![Arc::new(Disc), Arc::new(packs)], 3);
    let all = ["disc:breakdown", "mod:mxvatv_music/a", "mod:untamed_music/b"];
    assert_eq!(ids(&music.tracks()), all);
    // One pack off: the other plays beside the disc's.
    assert!(music.set_packs(true, &["untamed_music".into(), "mxvatv".into()]));
    assert_eq!(ids(&music.tracks()), ["disc:breakdown", "mod:mxvatv_music/a"]);
    assert!(!music.set_packs(true, &["untamed_music".into(), "mxvatv".into()]), "the same again changes nothing");
    // The mods folder off: the disc's alone, and its labels are the titles as before.
    assert!(music.set_packs(false, &[]));
    assert_eq!(ids(&music.tracks()), ["disc:breakdown"]);
    assert_eq!(music.labels()[0].1, "Breakdown - Disc Band");
    music.set_pick(Pick::Shuffle);
    music.wait = true;
    for _ in 0..8 {
        music.tend();
        assert_eq!(music.now(), Some("disc:breakdown"), "a pack switched off never plays");
        music.next();
    }
    assert!(music.set_packs(true, &[]));
    assert_eq!(ids(&music.tracks()), all);

    // The Audio page: Music From, then each pack's switch, which is the Mods page's.
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.set_music_packs(vec![("mxvatv_music".into(), "MX vs. ATV Unleashed soundtrack".into(), 1), ("untamed_music".into(), "MX vs. ATV Untamed soundtrack".into(), 1)]);
    m.open_tab(Screen::Settings, audio_tab());
    let row = |m: &Menu, key: Key| m.page().into_iter().find(|r| r.key == key).unwrap();
    assert_eq!(row(&m, Key::MusicFrom).value, "All Games");
    assert_eq!(row(&m, Key::MusicPack(1)).label, "MX vs. ATV Untamed soundtrack");
    let keys: Vec<Key> = m.page().into_iter().map(|r| r.key).collect();
    let from = keys.iter().position(|k| *k == Key::MusicFrom).unwrap();
    assert_eq!(&keys[from..from + 4], [Key::MusicFrom, Key::MusicPack(0), Key::MusicPack(1), Key::MusicPlays]);

    // MX Unleashed only, by a press on Music From.
    press(&mut m, Key::MusicFrom);
    assert_eq!(row(&m, Key::MusicFrom).value, "MX Unleashed Only");
    assert_eq!(m.settings.mods_off, ["mxvatv_music", "untamed_music"]);
    assert!(music.set_packs(m.settings.mods, &m.settings.mods_off));
    assert_eq!(ids(&music.tracks()), ["disc:breakdown"]);
    // A mix: one pack back on.
    press(&mut m, Key::MusicPack(1));
    assert_eq!(row(&m, Key::MusicFrom).value, "A Mix");
    assert_eq!(row(&m, Key::MusicPack(1)).value, "On");
    music.set_packs(m.settings.mods, &m.settings.mods_off);
    assert_eq!(ids(&music.tracks()), ["disc:breakdown", "mod:untamed_music/b"]);
    // From a mix, Music From is every pack off; once more, every pack on.
    press(&mut m, Key::MusicFrom);
    assert_eq!(row(&m, Key::MusicFrom).value, "MX Unleashed Only");
    press(&mut m, Key::MusicFrom);
    assert_eq!(row(&m, Key::MusicFrom).value, "All Games");
    assert!(m.settings.mods_off.is_empty());
    // Kept in settings.txt as the Mods page keeps them.
    press(&mut m, Key::MusicPack(0));
    assert_eq!(Settings::parse(&m.settings.text()).mods_off, ["mxvatv_music"]);
    // The folder off dims them.
    m.settings.mods = false;
    assert_eq!(row(&m, Key::MusicFrom).off.as_deref(), Some("The mods folder is off"));

    // No pack: the page is as it was, no Music From.
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.open_tab(Screen::Settings, audio_tab());
    assert!(m.page().iter().all(|r| r.key != Key::MusicFrom && !matches!(r.key, Key::MusicPack(_))));
    std::fs::remove_dir_all(&mods).unwrap();
}

/// Walk the open page's cursor to `key` and press Cross on it.
fn press(m: &mut Menu, key: Key) {
    use game_sim::input::{Control, Report};
    let mut r = menu::Reader::new();
    m.step(r.read(&Report::idle()));
    let mut tap = |m: &mut Menu, c: Control| {
        for report in [Report::from_buttons(c.mask()), Report::idle(), Report::idle(), Report::idle()] {
            m.step(r.read(&report));
        }
    };
    for _ in 0..64 {
        if m.page()[m.cursor(Screen::Settings)].key == key {
            break;
        }
        tap(m, Control::Down);
    }
    assert_eq!(m.page()[m.cursor(Screen::Settings)].key, key);
    tap(m, Control::Cross);
}

#[test]
fn a_bad_pack_or_song_is_a_line_and_the_rest_plays() {
    let mods = scratch("bad");
    write_pack(&mods, "mxvatv_music", "MX vs. ATV Unleashed", &[("A", "A", ""), ("B", "B", "")]);
    std::fs::remove_file(mods.join("music/mxvatv_music/Aud/Music/B.vig")).unwrap();
    // Its id is not its folder's; not JSON; a newer standard; another kind, which is no error.
    for (folder, json) in [
        ("wrong_music", r#"{"standard": 1, "kind": "music", "id": "other_music", "songs": []}"#.to_string()),
        ("broken_music", "{ not json".to_string()),
        ("future_music", r#"{"standard": 9, "kind": "music", "id": "future_music", "songs": []}"#.to_string()),
        ("mxvatv_nat13", r#"{"standard": 1, "kind": "track", "id": "mxvatv_nat13"}"#.to_string()),
    ] {
        std::fs::create_dir_all(mods.join("music").join(folder)).unwrap();
        std::fs::write(mods.join("music").join(folder).join("mod.json"), json).unwrap();
    }
    // A song whose file is outside its pack.
    let escape = mods.join("music/escape_music");
    std::fs::create_dir_all(&escape).unwrap();
    std::fs::write(
        escape.join("mod.json"),
        r#"{"standard": 1, "kind": "music", "id": "escape_music", "source": {"game": "X"}, "songs": [{"file": "../mxvatv_music/Aud/Music/A.vig", "title": "Out"}]}"#,
    )
    .unwrap();
    let (packs, log) = PackMusic::read(Some(&mods));
    assert_eq!(ids(&packs.tracks()), ["mod:mxvatv_music/a"]);
    assert_eq!(log.len(), 5, "{log:#?}");
    for (needle, why) in [("broken_music", "json"), ("future_music", "standard"), ("wrong_music", "id"), ("escape_music", "not a path"), ("mxvatv_music song 2", "no Aud/Music/B.vig")] {
        assert!(log.iter().any(|l| l.contains(needle) && l.contains(why)), "{needle}: {log:#?}");
    }
    std::fs::remove_dir_all(&mods).unwrap();
}

#[test]
fn with_no_pack_the_songs_are_the_discs() {
    // No folder, an empty one, and `--mods off` (none) all read no song.
    let empty = scratch("none");
    for mods in [Some(empty.as_path()), Some(empty.join("missing").as_path()), None] {
        let (packs, log) = PackMusic::read(mods);
        assert_eq!((packs.len(), log.len()), (0, 0));
    }
    let (packs, _) = PackMusic::read(Some(&empty));
    let with = audio::Music::new(vec![Arc::new(Disc), Arc::new(packs)], 3);
    let without = audio::Music::new(vec![Arc::new(Disc)], 3);
    assert_eq!(with.tracks(), without.tracks());
    assert_eq!(with.labels(), without.labels());
    std::fs::remove_dir_all(&empty).unwrap();

    // The disc's own: its list and labels unchanged by an empty pack source.
    let image = match game_render::disc::Disc::configured_path() {
        Ok(p) => p,
        Err(e) => return println!("SKIPPED the disc's half: {e}"),
    };
    let Ok(mut d) = game_render::disc::Disc::open(&image) else { return println!("SKIPPED the disc's half: it does not open") };
    let disc = Arc::new(audio::DiscMusic::read(&mut d, image).expect("the disc's songs"));
    let plain: Vec<(String, String)> = disc.tracks().iter().map(|t| (t.id.clone(), t.label())).collect();
    assert!(plain.len() >= 10, "{plain:?}");
    let music = audio::Music::new(vec![disc, Arc::new(PackMusic::read(None).0)], 3);
    assert_eq!(music.labels(), plain, "with no pack the playlist and its labels are the disc's");
}

/// The packs openmx-mods' `tools/import-music` built, where they are installed (`OPENMX_MUSIC_MODS`,
/// a mods folder), each song read and its first second decoded. Skips without one.
#[test]
fn the_installed_packs_read_and_play() {
    let Some(mods) = std::env::var_os("OPENMX_MUSIC_MODS").map(PathBuf::from) else {
        return println!("SKIPPED: OPENMX_MUSIC_MODS names no mods folder");
    };
    let (packs, log) = PackMusic::read(Some(&mods));
    assert!(log.is_empty(), "{log:#?}");
    assert!(packs.len() > 0, "no song in {}", mods.display());
    for t in packs.tracks() {
        let mut s = packs.open(&t.id).unwrap();
        let mut out = vec![0i16; s.rate() as usize * 2];
        assert_eq!(s.read(&mut out), out.len() / 2, "{}: under a second", t.id);
        assert!(out.iter().any(|x| *x != 0), "{}: silent", t.id);
        println!("{} | {} | {}", t.id, t.label(), packs.game(&t.id).unwrap_or_default());
    }
}
