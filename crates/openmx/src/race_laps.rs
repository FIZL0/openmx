//! S301: a race's laps as the shell starts them and answers what they tell. The race file's paths
//! and finish line, the score manager's words, and every rider's card make the [`Laps`] the race
//! steps after its drop; each tick the shell takes what they told and hands the activity's two
//! answers on: [`Supercross::race_over`] for each card that finished at the line (the podium's
//! flashes, the fireworks, a player's win and Hall of Fame entries) and [`Supercross::race_end`] once
//! the race is over (the results, the tips, the outro).
//!
//! The port's own where the original's code was not traced: the score manager's words are those
//! C31's live sx01 single race holds (S301's journal), less the two flags whose helpers are not
//! ported; the finish line is the race file's, not re-placed on the scene's finish-gate prop; and a
//! player's records are tried on the Hall of Fame's own tables with no track index.

use game_sim::race::{
    Course, EndSeat, FinishNotice, FlashLevels, HallOfFame, LapRules, LineLists, Laps, PodiumFlashes, RaceEnd, RaceOver, RaceRules, ResultEvent, ResultSeat,
    Supercross, Told, TrackFile, TrackPaths, card_flags, race_card_flags, race_flags, race_kind,
};
use ps2_float::{F, Vf};

/// How far ahead of and behind a rider's spot the track's path search reaches (the track's `+0x38`
/// and `+0x3c` in C31's live race).
pub const AHEAD: F = F(125.0);
pub const BEHIND: F = F(75.0);

/// The score manager's words in C31's live sx01 single race (flags `0x5e00`, kind 1, a 30 s finish
/// countdown, lap records from the 10th, one ghost run, below the path by more than 1 counted off,
/// 5 s off the track, an aim reach of 1, no time limit), for a race of `laps`. Its flags keep the
/// ranking, the lap records and the zone list; the off-track and notice helpers' two are left out.
pub fn rules(laps: u32) -> RaceRules {
    RaceRules {
        lap: LapRules {
            flags: race_flags::RANKED | race_flags::KEEP_LAPS | race_flags::ZONES,
            kind: race_kind::RACE,
            laps: laps as i32,
            finish_wait: F(30.0),
            record_limit: 10,
            ghost_runs: 1,
            ..LapRules::default()
        },
        below_limit: F(-1.0),
        time_limit: F::ZERO,
        off_limit: F(5.0),
        aim_reach: F(1.0),
    }
}

/// A human rider's card flags at the join, as C31's player card holds them before it: a player's,
/// held to the off-track limit.
pub const PLAYER: u64 = card_flags::PLAYER | race_card_flags::OFF_TRACK_LIMIT;

/// What the laps run on, from the race file (stood on the ground already): the track's paths with
/// its reach, the lap line off the file's finish line and its spot, the score manager's words and
/// its empty line lists (C31's reach 20). `None` when the file has no line or its paths do not walk.
pub fn course(file: &TrackFile, laps: u32) -> Option<(RaceRules, TrackPaths, Course, LineLists)> {
    let (mut track, spots) = TrackPaths::load(file)?;
    let line = file.line?;
    let line_spot = spots[1]?;
    track.ahead = AHEAD;
    track.behind = BEHIND;
    let course = Course { line, line_spot, lap_length: track.lap_length };
    Some((rules(laps), track, course, LineLists { reach: F(20.0), ..LineLists::default() }))
}

/// The laps at the start, every rider in `riders` (where it stands) a player.
pub fn start(course: &(RaceRules, TrackPaths, Course, LineLists), riders: &[Vf]) -> Laps {
    let (rules, track, line, lists) = course.clone();
    let cards: Vec<(Vf, u64)> = riders.iter().map(|&at| (at, PLAYER)).collect();
    Laps::new(rules, track, line, lists, &cards)
}

/// The event the answers read: a single race of `laps` in its league (0 supercross).
pub fn event(track: &str, laps: u32) -> ResultEvent {
    let league = i32::from(!track.to_ascii_lowercase().starts_with("sx"));
    ResultEvent { league, activity: 2, laps: laps.min(255) as u8, ..ResultEvent::default() }
}

/// What the activity answered one tick's told: [`Supercross::race_over`]'s for each card that
/// finished (with its rider), and [`Supercross::race_end`]'s if the race ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answers {
    pub overs: Vec<(usize, RaceOver)>,
    pub end: Option<RaceEnd>,
}

/// The activity's answers to `told`: each finisher's card as it stood when told, a player's seat its
/// rider's (whose records it keeps, with a profile), the podium's flashes `flashes`; and the end,
/// once, over the list the close found, a player's seat with no preload or clutch counted.
pub fn answer(told: &Told, activity: &mut Supercross, event: &ResultEvent, mut flashes: Option<&mut PodiumFlashes>, levels: &FlashLevels) -> Answers {
    let mut out = Answers::default();
    for (rider, card) in &told.finished {
        let player = card.flags & card_flags::PLAYER != 0;
        let seat = player.then_some(ResultSeat { seat: *rider as i32, keeps_records: true, profile: true });
        let over = activity.race_over(card, seat, event, -1, &HallOfFame::default(), flashes.as_deref_mut(), levels);
        out.overs.push((*rider, over));
    }
    if told.finish.iter().any(|n| matches!(n, FinishNotice::Over { .. })) {
        let cards: Vec<(i32, Option<EndSeat>)> = told.over_list.iter().map(|&(rider, place)| (place, Some(EndSeat { seat: rider as i32, preloads: 0, clutches: 0 }))).collect();
        out.end = Some(activity.race_end(&cards, event, true));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::race::{LapCard, LineNotice, RaceNotice};

    /// A player's finish in first answers the fireworks, a win and the winner's flashes; the end
    /// hands the results on and cues the winner's outro.
    #[test]
    fn a_players_finish_and_the_end_are_answered() {
        let card = LapCard { place: 1, flags: PLAYER | card_flags::OUT, ..LapCard::default() };
        let told = Told {
            said: vec![(0, RaceNotice::Line(LineNotice::Finished))],
            finish: vec![FinishNotice::Over { card: 0 }],
            finished: vec![(0, card)],
            over_list: vec![(0, 1)],
        };
        let mut activity = Supercross::new();
        let mut flashes = PodiumFlashes::default();
        let levels = FlashLevels { shortest: F(2.0), player_win: F(0.3), podium: F(0.1) };
        let a = answer(&told, &mut activity, &event("sx01", 3), Some(&mut flashes), &levels);
        assert_eq!(a.overs.len(), 1);
        assert!(a.overs[0].1.fireworks);
        assert_eq!(a.overs[0].1.win, Some(0));
        assert_eq!((flashes.length.0, flashes.half.0), (2.0, 1.0));
        let end = a.end.unwrap();
        assert!(end.results && end.tips.is_empty());
        assert_eq!(end.cue, Some(2));
    }

    /// Nothing told, nothing answered.
    #[test]
    fn a_quiet_tick_answers_nothing() {
        let mut activity = Supercross::new();
        let a = answer(&Told::default(), &mut activity, &event("nat01", 5), None, &FlashLevels::default());
        assert_eq!(a, Answers::default());
        assert_eq!(event("nat01", 5).league, 1);
    }

    /// The race's words keep the ranking, the lap records and the zones.
    #[test]
    fn the_rules_are_c31s_less_the_helpers() {
        let r = rules(3);
        assert_eq!((r.lap.flags, r.lap.kind, r.lap.laps), (0x4600, 1, 3));
    }
}
