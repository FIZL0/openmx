//! V127: a supercross stadium's camera flashes as the shell draws them. The race activity's loader,
//! on a track whose name holds `sx`, takes every `cameraflash` model out of the scene (sx01's 1200 in
//! its stands) and hides them; [`CameraFlashes`] then lights a batch of them at a time, a handful all
//! race long and a burst at each podium finish, and the lit ones are drawn here as actors, each at its
//! fade. A finish's burst is handed to [`Supercross::race_over`] ([`Stadium::card_finishes`]).
//!
//! The object itself is the race's (S321): handed to the first seat's [`Race`] as it is made
//! ([`Stadium::hand_to`]), the simulation ticks it at the end of the race's tick and its picks draw on
//! the race's generator, as the original's draw on the one `rand()` everything shares. The stadium
//! keeps where each flash stands and the track's levels, and draws what the race's object shows.

use game_render::gpu::Actor;
use game_render::ini;
use game_render::mat::M4;
use game_render::scene::{Placement, Scene};
use game_sim::camera_flash::{self, CameraFlashes};
use game_sim::race::{FlashLevels, HallOfFame, LapCard, PodiumFlashes, Race, RaceOver, ResultEvent, ResultSeat, Supercross};
use ps2_float::F;

/// The track's flash words off its `.toy`'s first `[CameraFlash]` (sx01 has two; the original keeps
/// the first's): the burst's shortest length (`BurstDuration`), a player's win (`PlayerWinBurstPercent`)
/// and anyone else on the podium (`AIFinishBurstPercent`), and the floor (`ConstantFlashPercent`), each
/// read as the original's track reader does (`strtod`, then truncated to a float: `0.3` is 0x3e999999).
/// A missing word is 0.
pub fn levels(toy: &str) -> (FlashLevels, F) {
    let sections = ini::parse(toy);
    let s = sections.iter().find(|s| s.name.eq_ignore_ascii_case("CameraFlash"));
    let word = |key: &str| s.and_then(|s| s.get(key)).map(game_sim::phy::number).unwrap_or(F::ZERO);
    let levels = FlashLevels { shortest: word("BurstDuration"), player_win: word("PlayerWinBurstPercent"), podium: word("AIFinishBurstPercent") };
    (levels, word("ConstantFlashPercent"))
}

/// Whether the race activity makes the flash object on this track: its name holds `sx`.
pub fn has_flashes(track: &str) -> bool {
    track.contains("sx")
}

/// The stadium's flashes: the object until the race takes it, the models it lights in its numbering
/// and the track's levels.
pub struct Stadium {
    /// The flash object, until [`Stadium::hand_to`] gives it to the race that ticks it.
    pub flashes: Option<CameraFlashes>,
    placed: Vec<Placement>,
    pub levels: FlashLevels,
}

impl Stadium {
    /// The `cameraflash` models taken out of `scene` (so the props drawn from it never show them),
    /// in its order, and the object over them; `None`, and the scene left alone, off an sx track or
    /// with none.
    pub fn take(scene: &mut Scene, track: &str, toy: &str) -> Option<Stadium> {
        if !has_flashes(track) {
            return None;
        }
        let (placed, kept): (Vec<Placement>, Vec<Placement>) = std::mem::take(&mut scene.models).into_iter().partition(|p| p.model.eq_ignore_ascii_case(camera_flash::MODEL));
        scene.models = kept;
        let (levels, floor) = levels(toy);
        let flashes = CameraFlashes::new(placed.len() as u32, floor)?;
        Some(Stadium { flashes: Some(flashes), placed, levels })
    }

    /// How many flashes the stadium has.
    pub fn len(&self) -> usize {
        self.placed.len()
    }

    /// The flash object goes to `race`, whose tick ticks it from now on (S321).
    pub fn hand_to(&mut self, race: &mut Race) {
        race.flashes = self.flashes.take().map(Box::new);
    }

    /// `flashes`' lit flashes' models where the scene placed them, and their fades.
    pub fn shown<'a>(&'a self, flashes: &'a CameraFlashes) -> impl Iterator<Item = (&'a str, M4, f32)> + 'a {
        flashes.shown().filter_map(|(k, fade)| self.placed.get(k as usize).map(|p| (p.model.as_str(), p.transform(), fade.0)))
    }

    /// [`Stadium::shown`] as actors, see-through by their fade (one at 0 is not drawn).
    pub fn actors<'a>(&'a self, flashes: &'a CameraFlashes) -> impl Iterator<Item = Actor<'a>> + 'a {
        self.shown(flashes).map(|(model, at, alpha)| Actor { model, at, pose: None, lod_from: None, alpha, casts: false })
    }

    /// P313: a card of the race's lap line has finished, and [`Supercross::race_over`] hears it
    /// (`finish`): the flashes burst toward its finish (`burst`, the race's flashes'), and what it
    /// answers (the fireworks, a player's win and Hall of Fame entries: P354) is the caller's.
    pub fn card_finishes(&self, finish: &Finish, burst: &mut PodiumFlashes) -> RaceOver {
        finish.heard(Some(burst), &self.levels)
    }
}

/// P354: a card's finish as [`Supercross::race_over`] hears it: the card, its seat if a player's,
/// the event, its track in the league's list (−1 for none) and the profile's Hall of Fame.
pub struct Finish<'a> {
    pub card: LapCard,
    pub seat: Option<ResultSeat>,
    pub event: ResultEvent,
    pub track: i32,
    pub fame: &'a HallOfFame,
}

impl<'a> Finish<'a> {
    /// An AI rider's card (or a player's off any Hall of Fame track) in a single race of `laps` laps:
    /// no seat, so no win and no records.
    pub fn single(card: &LapCard, laps: u8, fame: &'a HallOfFame) -> Self {
        let event = ResultEvent { activity: 2, laps, ..ResultEvent::default() };
        Finish { card: card.clone(), seat: None, event, track: -1, fame }
    }

    /// The first seat's human card in a single race of `laps` laps on `track` of `league`, bike class
    /// `class`: a player's card, keeping records, on a seat with a profile.
    pub fn player(card: &LapCard, seat: i32, league: i32, track: i32, class: i32, laps: u8, fame: &'a HallOfFame) -> Self {
        let event = ResultEvent { league, activity: 2, class, laps, ..ResultEvent::default() };
        let seat = Some(ResultSeat { seat, keeps_records: true, profile: true });
        let card = LapCard { flags: card.flags | game_sim::race::card_flags::PLAYER, ..card.clone() };
        Finish { card, seat, event, track, fame }
    }

    /// What [`Supercross::race_over`] answers, setting `burst` going where there is one.
    pub fn heard(&self, burst: Option<&mut PodiumFlashes>, levels: &FlashLevels) -> RaceOver {
        Supercross::new().race_over(&self.card, self.seat, &self.event, self.track, self.fame, burst, levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::race::card_flags;

    const SX01: &str = "[CameraFlash]\nConstantFlashPercent=0.01\nBurstDuration=2.0\nPlayerWinBurstPercent=0.3\nPlayerStuntBurstPercent=0.4\n\
        AIFinishBurstPercent=0.3\nAIStuntWinBurstPercent0.3\n\n[CameraFlash]\nConstantFlashPercent=0.025\nBurstDuration=2.0\n";

    /// sx01's first `[CameraFlash]` as the original's track words hold it (S13d's capture: +0x538 2.0,
    /// +0x53c and +0x544 0x3e999999, +0x54c 0x3c23d70a).
    #[test]
    fn the_first_camera_flash_section_gives_the_levels() {
        let (l, floor) = levels(SX01);
        let bits = [l.shortest, l.player_win, l.podium, floor].map(|x| x.0.to_bits());
        assert_eq!(bits, [0x4000_0000, 0x3e99_9999, 0x3e99_9999, 0x3c23_d70a]);
    }

    /// Only an sx track's flashes come out of its scene, and a player's win sends them up.
    #[test]
    fn an_sx_scene_gives_up_its_flashes() {
        let text = "[Model1]\nSLT=Barn\n[Model2]\nSLT=cameraflash\nPosition=1,2,3\n[Model3]\nSLT=cameraflash\n";
        let mut free = Scene::parse(text);
        assert!(Stadium::take(&mut free, "free06", SX01).is_none());
        assert_eq!(free.models.len(), 3);
        let mut sx = Scene::parse(text);
        let mut s = Stadium::take(&mut sx, "sx01", SX01).unwrap();
        assert_eq!((s.len(), sx.models.len()), (2, 1));
        let mut race = Race::new(game_sim::race::Gate::new(), &[], F(3.0), game_sim::rng::Rng::new());
        s.hand_to(&mut race);
        assert!(s.flashes.is_none());
        let flashes = race.flashes.as_deref_mut().unwrap();
        let card = LapCard { place: 1, flags: card_flags::PLAYER | card_flags::TAKING_PART | card_flags::OUT, ..LapCard::default() };
        let none = HallOfFame::default();
        let over = s.card_finishes(&Finish::single(&card, 3, &none), &mut flashes.burst);
        assert!(over.fireworks);
        assert_eq!((flashes.burst.length.0, flashes.burst.half.0), (2.0, 1.0));
        for _ in 0..30 {
            race.tick(F(1.0 / 30.0), &[]);
        }
        let flashes = race.flashes.as_deref().unwrap();
        assert!(flashes.brightness().0 > 0.25);
        assert_eq!(s.shown(flashes).count(), flashes.lit_count());
    }

    /// P354: the human's finish through the stadium counts the win and the Hall of Fame records its
    /// times beat in the profile's tables; the same card as an AI rider's counts neither.
    #[test]
    fn a_players_finish_keeps_its_win_and_records() {
        use game_sim::race::{FameEntry, fame_table};
        let mut sx = Scene::parse("[Model1]\nSLT=cameraflash\n");
        let s = Stadium::take(&mut sx, "sx01", SX01).unwrap();
        let mut fame = HallOfFame::new();
        for table in [fame_table::SX_BEST_LAP, fame_table::SX_3_LAPS] {
            let at = HallOfFame::offset(table, 2, 1).unwrap() / 4;
            fame.words[at..at + 5].fill(100f32.to_bits());
        }
        let card = LapCard { place: 1, best_lap: F(30.0), race_time: F(90.5), ..LapCard::default() };
        let mut burst = PodiumFlashes::default();
        let over = s.card_finishes(&Finish::player(&card, 0, 0, 2, 1, 3, &fame), &mut burst);
        assert_eq!(over.win, Some(0));
        let entry = |table| FameEntry { seat: 0, track: 2, table };
        assert_eq!(over.fame, vec![entry(fame_table::SX_BEST_LAP), entry(fame_table::SX_3_LAPS)]);
        assert!(over.fireworks && burst.length.0 == 2.0);
        let over = s.card_finishes(&Finish::single(&card, 3, &fame), &mut PodiumFlashes::default());
        assert_eq!((over.win, over.fame.len(), over.fireworks), (None, 0, true));
    }
}
