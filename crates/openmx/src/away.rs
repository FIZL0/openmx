//! **The app going away and coming back** (P298): iOS resigning the app active, then sending it to the
//! background on its way to being suspended. The shell pauses the ride and, entering the background,
//! gives its GPU models back, then answers, which lets iOS suspend it. Android's activity pausing,
//! then stopping, is the same pair (P311, [`openmx_android::away`]). Going away writes the settings
//! (P311: a phone ends an app in the background without a word); nothing else is saved, and a
//! relaunch after the phone ended the app goes to the front end as any launch does (P320).
//! Elsewhere the app never goes away.

/// The app has gone away since [`answered`] was last called: the count to answer with.
pub fn away() -> Option<usize> {
    #[cfg(target_os = "ios")]
    return openmx_ios::away();
    #[cfg(target_os = "android")]
    return openmx_android::away::away();
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    None
}

/// Going away is answered: the ride is paused, up to [`away`]'s `asked`.
pub fn answered(asked: usize) {
    #[cfg(target_os = "ios")]
    openmx_ios::answered_away(asked);
    #[cfg(target_os = "android")]
    openmx_android::away::answered(asked);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let _ = asked;
}

/// How many times the app has entered the background (iOS's DidEnterBackground, on its way to being
/// suspended; Android's activity stopped, P311) since it opened: never elsewhere (P298a).
pub fn backgrounds() -> usize {
    #[cfg(target_os = "ios")]
    return openmx_ios::backgrounds();
    #[cfg(target_os = "android")]
    return openmx_android::away::backgrounds();
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    0
}

/// Whether the app is away right now, not yet back (P311: Android's activity paused; its frames
/// still come, slowly, and the Auto preset must not time them). iOS suspends the app instead.
pub fn gone() -> bool {
    #[cfg(target_os = "android")]
    return openmx_android::away::paused();
    #[cfg(not(target_os = "android"))]
    false
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
