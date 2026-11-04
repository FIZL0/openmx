//! The Button Icons setting (P281), opentennis's (B362, its `legend.rs`): which buttons the hints
//! draw. Auto, the default, draws each controller's own — PlayStation by Sony's ids or name, Switch
//! by Nintendo's, Steam Deck by Valve's, else Xbox — and the keyboard's keys while the keyboard
//! drives; or one set for every controller: PlayStation, Xbox, Switch or Generic (the four face
//! buttons as dots, the meant one filled). The keyboard keeps its keys whatever the setting says.
//!
//! Every hint takes its look from here: the menus' hint tab, the splash and the pause
//! ([`super::screens`]), and the ride HUD's prompts and join cue (`ride_hud.rs`). A split's views
//! each draw their own player's ([`Buttons`] per view). `OPENMX_BUTTONS=keys|ps|xbox|switch|deck|generic`
//! draws every device in that style, for `./kit shot`s without the pad (opentennis's `HST_LEGEND`).

use super::Dev;
use game_sim::input::Control;

/// A device's button art.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Style {
    /// The keyboard's keys, as keycaps.
    Keys,
    /// ✕ ○ □ △, L1 / R1, Create / Options. The original's pad, and the look before P281.
    #[default]
    PlayStation,
    /// A B X Y in their colours, LB / RB, View / Menu.
    Xbox,
    /// Nintendo's letters (A right, B bottom, X top, Y left), L / R, − / +.
    Switch,
    /// Valve's A B X Y on dark buttons, L1 / R1, View / Menu.
    Deck,
    /// No maker's: the four face buttons as dots, the meant one filled; L1 / R1, Select / Start.
    Generic,
}

impl Style {
    pub const ALL: [Style; 6] = [Style::Keys, Style::PlayStation, Style::Xbox, Style::Switch, Style::Deck, Style::Generic];

    /// `OPENMX_BUTTONS`'s word for it.
    pub fn word(self) -> &'static str {
        match self {
            Style::Keys => "keys",
            Style::PlayStation => "ps",
            Style::Xbox => "xbox",
            Style::Switch => "switch",
            Style::Deck => "deck",
            Style::Generic => "generic",
        }
    }

    pub fn from_word(w: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|s| s.word() == w)
    }
}

/// The Button Icons setting (Settings → Display, `button_icons` in settings.txt): Auto, or one
/// style for every controller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonIcons {
    #[default]
    Auto,
    PlayStation,
    Xbox,
    Switch,
    Generic,
}

/// The Button Icons row's help, opentennis's words.
pub const HELP: &str = "The controller buttons the hints show. Auto: each controller's own (PlayStation, Switch, Steam Deck, else Xbox); or the same set for every controller.";

impl ButtonIcons {
    /// The row's choices in order, opentennis's.
    pub const ALL: [ButtonIcons; 5] = [ButtonIcons::Auto, ButtonIcons::PlayStation, ButtonIcons::Xbox, ButtonIcons::Switch, ButtonIcons::Generic];

    /// settings.txt's word, opentennis's.
    pub fn word(self) -> &'static str {
        match self {
            ButtonIcons::Auto => "auto",
            ButtonIcons::PlayStation => "ps",
            ButtonIcons::Xbox => "xbox",
            ButtonIcons::Switch => "switch",
            ButtonIcons::Generic => "generic",
        }
    }

    pub fn from_word(w: &str) -> Option<ButtonIcons> {
        ButtonIcons::ALL.into_iter().find(|b| b.word() == w)
    }

    /// The row's name for it.
    pub fn label(self) -> &'static str {
        match self {
            ButtonIcons::Auto => "Auto",
            ButtonIcons::PlayStation => "PlayStation",
            ButtonIcons::Xbox => "Xbox",
            ButtonIcons::Switch => "Switch",
            ButtonIcons::Generic => "Generic",
        }
    }

    /// The style it puts on every controller, `None` for Auto's each its own.
    pub fn style(self) -> Option<Style> {
        match self {
            ButtonIcons::Auto => None,
            ButtonIcons::PlayStation => Some(Style::PlayStation),
            ButtonIcons::Xbox => Some(Style::Xbox),
            ButtonIcons::Switch => Some(Style::Switch),
            ButtonIcons::Generic => Some(Style::Generic),
        }
    }
}

const SONY: u16 = 0x054c;
const NINTENDO: u16 = 0x057e;
const VALVE: u16 = 0x28de;
/// Steam's virtual pad, which stands in for whatever pad Steam Input reads: Valve's id on any
/// maker's pad, so it is a Deck only on a Deck.
const STEAM_VIRTUAL: u16 = 0x11ff;

/// What `pad.rs`'s `TOUCH_PAD` is called (P210): web/touch.js's on-screen pad, whose buttons are
/// ✕ ○ □ △.
const TOUCH_PAD: &str = "Touch controls";
/// What `ios.rs`'s `TOUCH_NAME` is called (P292): the iPhone's on-screen pad, touch.js's in the app.
const IOS_TOUCH_PAD: &str = "Touch pad";

/// Whether pad `id` (a seat's name for it, `vvvv:pppp uniq phys name`) is an on-screen touch pad,
/// the browser's or the iPhone's (P292).
pub fn is_touch(id: &str) -> bool {
    id.contains(TOUCH_PAD) || id.trim_end().strip_suffix(IOS_TOUCH_PAD).is_some_and(|rest| rest.is_empty() || rest.ends_with(' '))
}

/// A pad's family by its USB ids and its name, Auto's pick: PlayStation by Sony's vendor id or a
/// Sony name, Switch by Nintendo's, Steam Deck by Valve's (Steam's virtual pad only on a Deck,
/// `deck`), else Xbox. The name is all a browser's, an iPhone's or Windows' pad gives (vendor 0),
/// and a browser puts the ids in it: Chrome's "… (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)",
/// Firefox's "054c-0ce6-…".
pub fn family(name: &str, vendor: u16, product: u16, deck: bool) -> Style {
    let name = name.to_ascii_lowercase();
    let (vendor, product) = match (vendor, ids_in(&name)) {
        (0, Some(ids)) => ids,
        _ => (vendor, product),
    };
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    match vendor {
        SONY => Style::PlayStation,
        NINTENDO => Style::Switch,
        VALVE if product != STEAM_VIRTUAL || deck => Style::Deck,
        VALVE => Style::Xbox,
        _ if has(&["dualsense", "dualshock", "playstation", "ps3", "ps4", "ps5", "sony"]) || name == "wireless controller" => Style::PlayStation,
        _ if has(&["nintendo", "pro controller", "joy-con", "switch"]) => Style::Switch,
        _ if has(&["steam deck", "steam controller"]) => Style::Deck,
        _ => Style::Xbox,
    }
}

/// The ids a browser writes into a pad's name, if it did.
fn ids_in(name: &str) -> Option<(u16, u16)> {
    let hex = |s: &str| u16::from_str_radix(s.get(..4)?, 16).ok();
    if let (Some(v), Some(p)) = (name.find("vendor: "), name.find("product: ")) {
        return Some((hex(&name[v + 8..])?, hex(&name[p + 9..])?));
    }
    let mut parts = name.splitn(3, '-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(v), Some(p), Some(_)) if v.len() == 4 && p.len() == 4 => Some((hex(v)?, hex(p)?)),
        _ => None,
    }
}

/// A pad's family by the name the seats know it by (`pad.rs`'s `DeviceInfo::identity`,
/// `vvvv:pppp uniq phys name`; a node's path where the kernel said nothing).
pub fn identity_family(id: &str, deck: bool) -> Style {
    let ids = id.split_whitespace().next().and_then(|first| {
        let (v, p) = first.split_once(':')?;
        (v.len() == 4 && p.len() == 4).then_some(())?;
        Some((u16::from_str_radix(v, 16).ok()?, u16::from_str_radix(p, 16).ok()?))
    });
    match ids {
        Some((v, p)) => family(id.splitn(2, ' ').nth(1).unwrap_or(""), v, p, deck),
        None => family(id, 0, 0, deck),
    }
}

/// SteamOS says it is a Deck (`SteamDeck=1`): Steam's virtual pad there is the Deck's own.
pub fn on_steam_deck() -> bool {
    std::env::var("SteamDeck").as_deref() == Ok("1")
}

/// `OPENMX_BUTTONS`'s style, if it names one.
pub fn forced() -> Option<Style> {
    std::env::var("OPENMX_BUTTONS").ok().and_then(|w| Style::from_word(w.trim()))
}

/// The style device `dev` draws under `icons`: the keyboard its keys, the touch pad its ✕ ○ □ △
/// on Auto, any other pad its family on Auto, or the setting's.
pub fn style_of(icons: ButtonIcons, dev: &Dev) -> Style {
    if let Some(s) = forced() {
        return s;
    }
    match dev {
        Dev::Keys => Style::Keys,
        Dev::Pad(id) => icons.style().unwrap_or_else(|| if is_touch(id) { Style::PlayStation } else { identity_family(id, on_steam_deck()) }),
    }
}

/// How a view's hints draw their buttons: a style, the keyboard's key on each control (for
/// [`Style::Keys`]), and whether a Switch pad's A confirms ("Nintendo A/B" By Label).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub style: Style,
    /// The first key bound to each control, in [`Control`]'s order.
    pub keys: [String; 16],
    pub labels: bool,
}

impl Default for Buttons {
    /// PlayStation's, the look before P281, on the default keyboard.
    fn default() -> Buttons {
        Buttons { style: Style::PlayStation, keys: default_keys(), labels: false }
    }
}

/// The default keyboard's keys (`pad.rs`'s modern layout), in [`Control`]'s order.
pub fn default_keys() -> [String; 16] {
    let mut keys: [String; 16] = Default::default();
    for (c, k) in [
        (Control::Triangle, "G"),
        (Control::Circle, "F"),
        (Control::Cross, "Space"),
        (Control::Square, "V"),
        (Control::L1, "Q"),
        (Control::R1, "E"),
        (Control::L2, "S"),
        (Control::R2, "W"),
        (Control::Select, "C"),
        (Control::Start, "Enter"),
        (Control::R3, "B"),
        (Control::L3, "Z"),
        (Control::Up, "Up"),
        (Control::Right, "Right"),
        (Control::Down, "Down"),
        (Control::Left, "Left"),
    ] {
        keys[c as usize] = k.to_string();
    }
    keys
}

/// One button as a hint draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A face button, by where it sits: 0 bottom, 1 right, 2 left, 3 top.
    Face(Style, u8),
    /// A key, by its name.
    Cap(String),
    /// A shoulder button's word.
    Shoulder(&'static str),
    /// Select's or Start's word.
    Small(&'static str),
}

impl Buttons {
    /// `style` on the default keyboard.
    pub fn of(style: Style) -> Buttons {
        Buttons { style, ..Buttons::default() }
    }

    /// The glyph control `c` is drawn as; `None` for the d-pad, which every style draws as its
    /// arrows. `menus`: the menus' Circle, which on the keyboard is Escape (and the bound key).
    pub fn glyph(&self, c: Control, menus: bool) -> Option<Glyph> {
        let s = self.style;
        if s == Style::Keys {
            return match c {
                Control::Up | Control::Down | Control::Left | Control::Right => None,
                Control::Circle if menus => Some(Glyph::Cap("Esc".into())),
                // A control no key is bound to says its own name.
                c => Some(Glyph::Cap(Some(&self.keys[c as usize]).filter(|k| !k.is_empty()).map_or_else(|| c.name().to_string(), String::clone))),
            };
        }
        let (ps, sw, xb) = (s == Style::PlayStation, s == Style::Switch, s == Style::Xbox);
        let pick = |p: &'static str, w: &'static str, x: &'static str| if sw { w } else if xb { x } else { p };
        // By label a Switch's A, on the right, is Cross and its B Circle.
        let swap = sw && self.labels;
        Some(match c {
            Control::Cross => Glyph::Face(s, if swap { 1 } else { 0 }),
            Control::Circle => Glyph::Face(s, if swap { 0 } else { 1 }),
            Control::Square => Glyph::Face(s, 2),
            Control::Triangle => Glyph::Face(s, 3),
            Control::L1 => Glyph::Shoulder(pick("L1", "L", "LB")),
            Control::R1 => Glyph::Shoulder(pick("R1", "R", "RB")),
            Control::L2 => Glyph::Shoulder(pick("L2", "ZL", "LT")),
            Control::R2 => Glyph::Shoulder(pick("R2", "ZR", "RT")),
            Control::L3 => Glyph::Shoulder(pick("L3", "LS", "LS")),
            Control::R3 => Glyph::Shoulder(pick("R3", "RS", "RS")),
            // The PlayStation's are the DualShock 2's, the pad the original was played on.
            Control::Select => Glyph::Small(if ps || s == Style::Generic { "SELECT" } else if sw { "-" } else { "VIEW" }),
            Control::Start => Glyph::Small(if ps || s == Style::Generic { "START" } else if sw { "+" } else { "MENU" }),
            Control::Up | Control::Down | Control::Left | Control::Right => return None,
        })
    }

    /// The word a hint spells a button out as ("Cross", "A", "Space"), where it says it in words.
    pub fn word(&self, c: Control) -> String {
        match self.glyph(c, false) {
            Some(Glyph::Face(Style::PlayStation, i)) => ["Cross", "Circle", "Square", "Triangle"][i as usize].into(),
            Some(Glyph::Face(Style::Generic, i)) => ["Bottom", "Right", "Left", "Top"][i as usize].into(),
            Some(Glyph::Face(s, i)) => face_letter(s, i).into(),
            Some(Glyph::Cap(k)) => k,
            Some(Glyph::Shoulder(w) | Glyph::Small(w)) => w.into(),
            None => c.name().into(),
        }
    }
}

/// The letter on face button `i` (0 bottom, 1 right, 2 left, 3 top) of a lettered style.
pub fn face_letter(style: Style, i: u8) -> &'static str {
    let letters = if style == Style::Switch { ["B", "A", "Y", "X"] } else { ["A", "B", "X", "Y"] };
    letters[i as usize & 3]
}

/// A lettered face button's fill and its letter's ink: Xbox's green, red, blue and yellow, a
/// Switch's and a Deck's dark buttons with white letters.
pub fn face_colours(style: Style, i: u8) -> ([u8; 4], [u8; 4]) {
    const DARK: [u8; 4] = [0x2e, 0x28, 0x26, 255];
    const WHITE: [u8; 4] = [0xf5, 0xf0, 0xe8, 255];
    match style {
        Style::Xbox => ([[0x46, 0xaa, 0x3c, 255], [0xd7, 0x3c, 0x3c, 255], [0x32, 0x73, 0xdc, 255], [0xe6, 0xb4, 0x1e, 255]][i as usize & 3], WHITE),
        _ => (DARK, WHITE),
    }
}

/// The dots of a Generic face button, as offsets of its radius: bottom, right, left, top.
pub const GENERIC_DOTS: [[f32; 2]; 4] = [[0.0, 0.44], [0.44, 0.0], [-0.44, 0.0], [0.0, -0.44]];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_picks_each_pads_family() {
        assert_eq!(identity_family("054c:0ce6  usb-1 Sony Interactive Entertainment DualSense Wireless Controller", false), Style::PlayStation);
        assert_eq!(identity_family("057e:2009 aa:bb  Nintendo Switch Pro Controller", false), Style::Switch);
        assert_eq!(identity_family("28de:1205  usb-3 Steam Deck", false), Style::Deck);
        assert_eq!(identity_family("045e:028e  usb-2 Microsoft X-Box 360 pad", false), Style::Xbox);
        // Steam's virtual pad is a Deck only on a Deck.
        assert_eq!(identity_family("28de:11ff   Microsoft X-Box 360 pad 0", false), Style::Xbox);
        assert_eq!(identity_family("28de:11ff   Microsoft X-Box 360 pad 0", true), Style::Deck);
        // Names alone, as a browser, an iPhone and Windows give them.
        assert_eq!(family("DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)", 0, 0, false), Style::PlayStation);
        assert_eq!(family("057e-2009-Pro Controller", 0, 0, false), Style::Switch);
        assert_eq!(family("DUALSHOCK 4 Wireless Controller", 0, 0, false), Style::PlayStation);
        assert_eq!(family("Xbox Wireless Controller", 0, 0, false), Style::Xbox);
        assert_eq!(family("8BitDo SN30 Pro", 0, 0, false), Style::Xbox);
    }

    #[test]
    fn a_fixed_setting_is_every_pads_but_not_the_keyboards() {
        let ds = Dev::Pad("054c:0ce6   DualSense Wireless Controller".into());
        if forced().is_some() {
            return;
        }
        assert_eq!(style_of(ButtonIcons::Auto, &ds), Style::PlayStation);
        assert_eq!(style_of(ButtonIcons::Xbox, &ds), Style::Xbox);
        assert_eq!(style_of(ButtonIcons::Xbox, &Dev::Keys), Style::Keys);
        assert_eq!(style_of(ButtonIcons::Auto, &Dev::Pad("/dev/input/js0 Touch controls".into())), Style::PlayStation);
        // The iPhone's touch pad, as `pad.rs` names it (P292), and no other pad of the name's words.
        assert_eq!(style_of(ButtonIcons::Auto, &Dev::Pad("0000:0000   Touch pad".into())), Style::PlayStation);
        assert!(!is_touch("045e:028e   Xbox Touch padded"));
    }

    #[test]
    fn glyphs_name_each_styles_buttons() {
        let b = |s| Buttons::of(s);
        assert_eq!(b(Style::Xbox).word(Control::Cross), "A");
        assert_eq!(b(Style::Switch).word(Control::Cross), "B");
        assert_eq!(Buttons { labels: true, ..b(Style::Switch) }.word(Control::Cross), "A");
        assert_eq!(b(Style::PlayStation).word(Control::Circle), "Circle");
        assert_eq!(b(Style::Keys).word(Control::Cross), "Space");
        assert_eq!(b(Style::Keys).glyph(Control::Circle, true), Some(Glyph::Cap("Esc".into())));
        assert_eq!(b(Style::Xbox).glyph(Control::L1, true), Some(Glyph::Shoulder("LB")));
        assert_eq!(b(Style::Switch).glyph(Control::Start, true), Some(Glyph::Small("+")));
    }
}
