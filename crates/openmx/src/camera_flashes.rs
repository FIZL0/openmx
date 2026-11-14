//! V127: a supercross stadium's camera flashes as the shell draws them. The race activity's loader,
//! on a track whose name holds `sx`, takes every `cameraflash` model out of the scene (sx01's 1200 in
//! its stands) and hides them; [`CameraFlashes`] then lights a batch of them at a time, a handful all
//! race long and a burst at each podium finish, and the lit ones are drawn here as actors, each at its
//! fade. A finish's burst is handed to [`Supercross::race_over`] through [`Stadium::podium`].
//!
//! The draws are the shell's own generator's, not the simulation's (the original's come off the
//! generator everything shares: V127's journal).

use game_render::gpu::Actor;
use game_render::ini;
use game_render::mat::M4;
use game_render::scene::{Placement, Scene};
use game_sim::camera_flash::{self, CameraFlashes};
use game_sim::race::{FlashLevels, HallOfFame, LapCard, PodiumFlashes, RaceOver, ResultEvent, Supercross, card_flags};
use game_sim::rng::Rng;
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

/// The stadium's flashes: the object, the models it lights in its numbering, the track's levels and
/// the generator its picks draw on.
pub struct Stadium {
    pub flashes: CameraFlashes,
    placed: Vec<Placement>,
    pub levels: FlashLevels,
    rng: Rng,
    /// Seconds since the race started, for `--race-win`.
    pub clock: f32,
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
        Some(Stadium { flashes, placed, levels, rng: Rng::new(), clock: 0.0 })
    }

    /// How many flashes the stadium has.
    pub fn len(&self) -> usize {
        self.placed.len()
    }

    /// One tick of the flashes.
    pub fn tick(&mut self, dt: F) {
        self.flashes.tick(dt, &mut self.rng);
        self.clock += dt.0;
    }

    /// The burst a podium finish sets going, for [`Supercross::race_over`].
    pub fn podium(&mut self) -> &mut PodiumFlashes {
        &mut self.flashes.burst
    }

    /// The lit flashes' models where the scene placed them, and their fades.
    pub fn shown(&self) -> impl Iterator<Item = (&str, M4, f32)> + '_ {
        self.flashes.shown().filter_map(|(k, fade)| self.placed.get(k as usize).map(|p| (p.model.as_str(), p.transform(), fade.0)))
    }

    /// [`Stadium::shown`] as actors, see-through by their fade (one at 0 is not drawn).
    pub fn actors(&self) -> impl Iterator<Item = Actor<'_>> + '_ {
        self.shown().map(|(model, at, alpha)| Actor { model, at, pose: None, lod_from: None, alpha, casts: false })
    }

    /// `--race-win`: the human's card wins a single race at the line, as the lap line will tell
    /// [`Supercross::race_over`] once the shell counts laps (S301): the flashes burst toward the
    /// player's win, and what it answers (the fireworks) is the caller's to show.
    pub fn player_wins(&mut self) -> RaceOver {
        let card = LapCard { place: 1, flags: card_flags::PLAYER | card_flags::TAKING_PART | card_flags::OUT, ..LapCard::default() };
        let event = ResultEvent { activity: 2, ..ResultEvent::default() };
        let levels = self.levels;
        Supercross::new().race_over(&card, None, &event, -1, &HallOfFame::default(), Some(self.podium()), &levels)
    }

    /// P313: a card of the race's lap line has finished, and [`Supercross::race_over`] hears it as
    /// [`Stadium::player_wins`] does, with the card's own place and flags.
    pub fn card_finishes(&mut self, card: &LapCard, laps: u8) -> RaceOver {
        let event = ResultEvent { activity: 2, laps, ..ResultEvent::default() };
        let levels = self.levels;
        Supercross::new().race_over(card, None, &event, -1, &HallOfFame::default(), Some(self.podium()), &levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let over = s.player_wins();
        assert!(over.fireworks);
        assert_eq!((s.flashes.burst.length.0, s.flashes.burst.half.0), (2.0, 1.0));
        for _ in 0..30 {
            s.tick(F(1.0 / 30.0));
        }
        assert!(s.flashes.brightness().0 > 0.25);
        assert_eq!(s.shown().count(), s.flashes.lit_count());
    }
}
