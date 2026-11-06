//! **The app going away and coming back** (P298): iOS resigning the app active, then sending it to the
//! background on its way to being suspended. The shell pauses the ride and, entering the background,
//! gives its GPU models back, then answers, which lets iOS suspend it. Nothing is saved: a relaunch
//! after iOS ended the app goes to the front end as any launch does (P320). Elsewhere the app never
//! goes away.

/// The app has gone away since [`answered`] was last called: the count to answer with.
pub fn away() -> Option<usize> {
    #[cfg(target_os = "ios")]
    return openmx_ios::away();
    #[cfg(not(target_os = "ios"))]
    None
}

/// Going away is answered: the ride is paused, up to [`away`]'s `asked`.
pub fn answered(asked: usize) {
    #[cfg(target_os = "ios")]
    openmx_ios::answered_away(asked);
    #[cfg(not(target_os = "ios"))]
    let _ = asked;
}

/// How many times the app has entered the background (iOS's DidEnterBackground, on its way to being
/// suspended) since it opened: never elsewhere (P298a).
pub fn backgrounds() -> usize {
    #[cfg(target_os = "ios")]
    return openmx_ios::backgrounds();
    #[cfg(not(target_os = "ios"))]
    0
}

#[cfg(test)]
mod tests {
    use crate::menu::{Menu, Press, Screen};

    /// P320: the launch offers nothing — no "Resume …?" popup takes the main menu's first press.
    #[test]
    fn the_main_menu_after_a_launch_takes_its_own_first_press() {
        let mut menu = Menu::new(Default::default(), Default::default());
        menu.main();
        assert_eq!(menu.screen(), Some(Screen::Main));
        menu.step(Press { ok: true, ..Press::default() });
        assert_ne!(menu.screen(), Some(Screen::Main), "Cross on the main menu opened nothing: something sat over it");
        let mut menu = Menu::new(Default::default(), Default::default());
        menu.main();
        menu.step(Press { down: true, ..Press::default() });
        menu.step(Press { ok: true, ..Press::default() });
        assert_ne!(menu.screen(), Some(Screen::Main), "Down then Cross opened nothing");
    }
}
