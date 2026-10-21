//! A window to ride in: the X11 protocol over its own socket, and nothing else.
//!
//! The whole workspace has no dependency outside itself — the archives, the geometry, the textures and
//! the rasteriser are all written out — so the shell opens its window the same way: it speaks the X11
//! wire protocol down the display's unix socket. That is about three hundred lines for what this needs
//! (one window, one graphics context, a key state and an image per frame) and it keeps `cargo run -p
//! openmx` buildable with nothing installed but a Rust toolchain.
//!
//! What it does *not* do, deliberately: no `MIT-SHM` (that needs shared memory, which needs unsafe
//! code), no `BIG-REQUESTS` (the image is split into bands that fit the server's own maximum request
//! instead), no reply-bearing requests at all — so the only thing that ever comes back down the socket
//! is an event or an error, both 32 bytes, and the event reader never has to know a request's shape.
//!
//! Authority for the format: the X Window System Protocol, version 11. Field orders below are that
//! document's, in its own order, which is why the request builders read as a list of pushes.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;

/// Request opcodes, by the names the protocol gives them.
const CREATE_WINDOW: u8 = 1;
const MAP_WINDOW: u8 = 8;
const CHANGE_PROPERTY: u8 = 18;
const CREATE_GC: u8 = 55;
const PUT_IMAGE: u8 = 72;

/// The two predefined atoms the shell needs, so that it never has to ask for one (an `InternAtom` is a
/// request with a *reply*, and this module's whole simplification is that it makes none).
const ATOM_WM_NAME: u32 = 39;
const ATOM_STRING: u32 = 31;

/// How far apart the two halves of an auto-repeat may be stamped, in milliseconds. The server
/// generates both from one operation and gives them the same timestamp, so this is slack and not a
/// window: it costs nothing, because no finger lets go of a key and presses it again inside two
/// milliseconds, and it means a server that stamped the pair a millisecond apart would still be read
/// correctly rather than falling back to the flicker.
const REPEAT_MS: u32 = 2;

/// Event codes. The window selects three masks' worth of these.
const KEY_PRESS: u8 = 2;
const KEY_RELEASE: u8 = 3;
const DESTROY_NOTIFY: u8 = 17;
const UNMAP_NOTIFY: u8 = 18;
const CONFIGURE_NOTIFY: u8 = 22;

/// `CreateWindow`'s value mask: the background pixel, then the event mask. Values go in the request in
/// increasing bit order, which is the order they are listed here.
const CW_BACK_PIXEL: u32 = 0x0000_0002;
const CW_EVENT_MASK: u32 = 0x0000_0800;
/// What the window asks to hear about: keys down, keys up, and its structure — which is being taken
/// away, and being given a new size (`ConfigureNotify`).
const EVENT_MASK: u32 = 0x0000_0001 | 0x0000_0002 | 0x0002_0000;

/// `PutImage`'s format: one byte per pixel channel in the server's own layout.
const ZPIXMAP: u8 = 2;

/// How many X11 keycodes there are. An event's `detail` is one byte, so the held-key table is one byte
/// wide and complete.
pub const KEYCODES: usize = 256;

/// The X11 keycodes the shell binds, by the name of the key as a Linux keyboard reports it. These are
/// the evdev keycodes every X server on Linux has used since XKB's evdev rules became the default
/// (the kernel's own code plus eight), so they do not depend on the user's layout — `Z` is the key
/// where `Z` is on a US keyboard whatever the layout calls it, which is what a game wants.
pub mod key {
    pub const ESCAPE: u8 = 9;
    pub const TAB: u8 = 23;
    pub const Q: u8 = 24;
    pub const E: u8 = 26;
    pub const RETURN: u8 = 36;
    pub const A: u8 = 38;
    pub const S: u8 = 39;
    pub const D: u8 = 40;
    pub const F: u8 = 41;
    pub const H: u8 = 43;
    pub const Z: u8 = 52;
    pub const X: u8 = 53;
    pub const C: u8 = 54;
    pub const V: u8 = 55;
    pub const SPACE: u8 = 65;
    pub const UP: u8 = 111;
    pub const LEFT: u8 = 113;
    pub const RIGHT: u8 = 114;
    pub const DOWN: u8 = 116;
    pub const I: u8 = 31;
    pub const J: u8 = 44;
    pub const K: u8 = 45;
    pub const L: u8 = 46;
    pub const M: u8 = 58;
    // The new layout's keys (P54): WASD the left stick, YUIO the right (P75), HJKL the face buttons.
    pub const W: u8 = 25;
    pub const N: u8 = 57;
    pub const Y: u8 = 29;
    pub const U: u8 = 30;
    pub const O: u8 = 32;
    pub const SHIFT_LEFT: u8 = 50;
    pub const SHIFT_RIGHT: u8 = 62;
    pub const F3: u8 = 69;
}

/// An open window on an X display.
pub struct Window {
    stream: UnixStream,
    window: u32,
    gc: u32,
    /// The window's size in pixels: what it was opened at, and then whatever the last
    /// `ConfigureNotify` said — a window manager that tiles or a person who drags an edge changes it,
    /// and the frame follows ([`Window::resized`]) rather than being stretched over the difference.
    pub width: usize,
    pub height: usize,
    /// How many window pixels one frame pixel becomes, in both directions.
    pub scale: usize,
    /// The screen's own size in pixels, out of the connection setup: what a window opened without a
    /// size of its own is opened at.
    pub screen: (usize, usize),
    /// The window's size has changed since [`Window::resized`] last said so.
    resized: bool,
    depth: u8,
    /// The largest request the server will take, in bytes — the image is banded to fit it.
    max_request: usize,
    /// Which keys are down, by keycode, with auto-repeat filtered out of the stream.
    keys: Keyboard,
    /// The server took the window away, or the connection went.
    closed: bool,
    /// Requests being assembled, likewise.
    out: Vec<u8>,
}

impl Window {
    /// Open a window for frames of `size` pixels, each drawn `scale` times across and down — or, with
    /// no size, a window as big as the screen, whose frame is its own shape or the largest 4:3 one inside
    /// it, as `--aspect` says ([`crate::aspect::Aspect::frame`]).
    ///
    /// `display` is the usual `DISPLAY`, of which only the display number is used: this talks to the
    /// unix socket, never to TCP, and sends no authorisation — which is what a local X server (or the
    /// Xwayland a Wayland session runs) accepts from a client of the same user.
    pub fn open(
        display: &str,
        size: Option<(usize, usize)>,
        scale: usize,
        title: &str,
    ) -> Result<Window, String> {
        let number = display_number(display)?;
        let path = format!("/tmp/.X11-unix/X{number}");
        let stream = UnixStream::connect(&path).map_err(|e| format!("{path}: {e}"))?;
        let mut w = Window {
            stream,
            window: 0,
            gc: 0,
            width: 0,
            height: 0,
            scale,
            screen: (0, 0),
            resized: false,
            depth: 24,
            max_request: 0,
            keys: Keyboard::new(),
            closed: false,
            out: Vec::new(),
        };
        let setup = w.handshake()?;
        w.depth = setup.root_depth;
        w.max_request = setup.max_request_bytes;
        w.screen = (setup.screen_width, setup.screen_height);
        (w.width, w.height) = match size {
            Some((width, height)) => (width * scale, height * scale),
            None => w.screen,
        };
        // One pixel per 32 bits is what every modern server uses for depth 24, and it is the layout
        // the frame is converted into below. Anything else would need a different conversion, so say
        // so rather than draw a smear.
        if setup.bits_per_pixel != 32 {
            return Err(format!(
                "this display wants {} bits per pixel at depth {}; the shell writes 32",
                setup.bits_per_pixel, setup.root_depth
            ));
        }
        if setup.image_byte_order != 0 {
            return Err("this display is big-endian; the shell writes little-endian pixels".into());
        }
        w.window = setup.id_base | 1;
        w.gc = setup.id_base | 2;
        w.create_window(&setup, title)?;
        w.stream
            .set_nonblocking(true)
            .map_err(|e| format!("the display socket will not go non-blocking: {e}"))?;
        Ok(w)
    }

    /// The connection setup: the only exchange in this module that has a reply.
    fn handshake(&mut self) -> Result<Setup, String> {
        let mut request = Vec::new();
        request.push(b'l'); // little-endian
        request.push(0);
        request.extend_from_slice(&11u16.to_le_bytes()); // protocol major
        request.extend_from_slice(&0u16.to_le_bytes()); // protocol minor
        request.extend_from_slice(&0u16.to_le_bytes()); // no authorisation name
        request.extend_from_slice(&0u16.to_le_bytes()); // no authorisation data
        request.extend_from_slice(&0u16.to_le_bytes()); // pad
        self.stream
            .write_all(&request)
            .map_err(|e| format!("the display will not take a setup: {e}"))?;

        let mut head = [0u8; 8];
        self.stream
            .read_exact(&mut head)
            .map_err(|e| format!("the display sent no setup reply: {e}"))?;
        let extra = u16::from_le_bytes([head[6], head[7]]) as usize * 4;
        let mut body = vec![0u8; extra];
        self.stream
            .read_exact(&mut body)
            .map_err(|e| format!("the display's setup reply is short: {e}"))?;
        match head[0] {
            1 => {}
            0 => {
                let reason = head[1] as usize;
                let text = String::from_utf8_lossy(&body[..reason.min(body.len())]).into_owned();
                return Err(format!("the display refused the connection: {text}"));
            }
            2 => return Err("the display wants authorisation this shell cannot give".into()),
            other => return Err(format!("the display answered a setup with {other}")),
        }
        parse_setup(&body)
    }

    /// Create the window, name it, give it a graphics context and put it on screen.
    fn create_window(&mut self, setup: &Setup, title: &str) -> Result<(), String> {
        let mut r = Vec::new();
        r.push(CREATE_WINDOW);
        r.push(0); // depth: copy the parent's
        r.extend_from_slice(&(8u16 + 2).to_le_bytes()); // request length, in words
        r.extend_from_slice(&self.window.to_le_bytes());
        r.extend_from_slice(&setup.root.to_le_bytes());
        r.extend_from_slice(&0i16.to_le_bytes()); // x, left to the window manager
        r.extend_from_slice(&0i16.to_le_bytes()); // y
        r.extend_from_slice(&(self.width as u16).to_le_bytes());
        r.extend_from_slice(&(self.height as u16).to_le_bytes());
        r.extend_from_slice(&0u16.to_le_bytes()); // border width
        r.extend_from_slice(&1u16.to_le_bytes()); // class: InputOutput
        r.extend_from_slice(&setup.root_visual.to_le_bytes());
        r.extend_from_slice(&(CW_BACK_PIXEL | CW_EVENT_MASK).to_le_bytes());
        r.extend_from_slice(&setup.black_pixel.to_le_bytes());
        r.extend_from_slice(&EVENT_MASK.to_le_bytes());

        // The title, as a plain string property on the window.
        let name = title.as_bytes();
        let pad = (4 - name.len() % 4) % 4;
        r.push(CHANGE_PROPERTY);
        r.push(0); // Replace
        r.extend_from_slice(&(6u16 + ((name.len() + pad) / 4) as u16).to_le_bytes());
        r.extend_from_slice(&self.window.to_le_bytes());
        r.extend_from_slice(&ATOM_WM_NAME.to_le_bytes());
        r.extend_from_slice(&ATOM_STRING.to_le_bytes());
        r.push(8); // one byte per character
        r.extend_from_slice(&[0, 0, 0]);
        r.extend_from_slice(&(name.len() as u32).to_le_bytes());
        r.extend_from_slice(name);
        r.extend_from_slice(&vec![0u8; pad]);

        r.push(CREATE_GC);
        r.push(0);
        r.extend_from_slice(&4u16.to_le_bytes());
        r.extend_from_slice(&self.gc.to_le_bytes());
        r.extend_from_slice(&self.window.to_le_bytes());
        r.extend_from_slice(&0u32.to_le_bytes()); // no values: the defaults draw an image fine

        r.push(MAP_WINDOW);
        r.push(0);
        r.extend_from_slice(&2u16.to_le_bytes());
        r.extend_from_slice(&self.window.to_le_bytes());

        self.stream
            .write_all(&r)
            .map_err(|e| format!("the display will not take the window: {e}"))
    }

    /// Whether the window has gone — closed by its manager, or the connection dropped.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Whether a key is down now.
    pub fn held(&self, code: u8) -> bool {
        self.keys.held(code)
    }

    /// Whether a key went down during the last poll, whether or not it is still down (P53).
    pub fn tapped(&self, code: u8) -> bool {
        self.keys.tapped[code as usize]
    }

    /// Read everything the server has said since the last call: keys, and whether the window is gone.
    ///
    /// Never blocks, so a frame that nobody touches costs one `read` that returns `WouldBlock`.
    ///
    /// The key state settles when the stream runs dry rather than on each event, because a release
    /// is only believed once the event after it has been seen ([`Keyboard`]).
    pub fn poll(&mut self) {
        self.keys.tapped = [false; KEYCODES];
        let mut event = [0u8; 32];
        loop {
            match read_exact_nb(&mut self.stream, &mut event) {
                Ok(true) => self.handle(&event),
                Ok(false) => {
                    self.keys.settle();
                    return;
                }
                Err(_) => {
                    self.keys.settle();
                    self.closed = true;
                    return;
                }
            }
        }
    }

    /// One 32-byte event or error.
    fn handle(&mut self, event: &[u8; 32]) {
        match event[0] & 0x7f {
            0 => {
                // An error. Nothing the shell sends should produce one, so it is worth a line on
                // stderr — but not worth dying for, because the window is usually still usable.
                eprintln!(
                    "openmx: the display reported error {} for request {}.{}",
                    event[1], event[10], event[9]
                );
            }
            KEY_PRESS | KEY_RELEASE => self.keys.saw(event),
            DESTROY_NOTIFY | UNMAP_NOTIFY => self.closed = true,
            // The structure mask also brings the window's moves and restacks, which carry the same
            // size again, so only a size that differs counts as a resize. Width and height are the
            // event's fifth and sixth fields, after the two windows, the sibling and the position.
            CONFIGURE_NOTIFY => {
                let (width, height) = (le16(event, 20) as usize, le16(event, 22) as usize);
                if (width, height) != (self.width, self.height) && width > 0 && height > 0 {
                    (self.width, self.height) = (width, height);
                    self.resized = true;
                }
            }
            _ => {}
        }
    }

    /// Whether the window has changed size since the last time this was asked — once per change, so
    /// the shell re-sizes its frame on the frame after a resize and not on every frame.
    pub fn resized(&mut self) -> bool {
        std::mem::take(&mut self.resized)
    }

    /// Put one frame on the window: RGB triples, row by row from the top, scaled up by
    /// [`Window::scale`], centred, and sent as however many bands the server's maximum request allows.
    ///
    /// Only the picture's own rectangle is written. A window shaped differently from the picture —
    /// a 16:9 screen round `--aspect 4:3`'s frame — leaves the rest to the window's background,
    /// which is black and which the server paints itself: the window has the
    /// default bit gravity, so a resize clears it, and nothing of an older, larger picture is left
    /// in the bars.
    pub fn present(&mut self, rgb: &[u8], width: usize, height: usize) -> Result<(), String> {
        if rgb.len() < width * height * 3 {
            return Err("the frame is smaller than its own size".into());
        }
        let (across, down) = (width * self.scale, height * self.scale);
        let (left, top) = centred((across, down), (self.width, self.height));
        let row_bytes = across * 4;
        // The 24-byte header has to fit too, and a band of no rows would never finish.
        let rows_per_band = ((self.max_request - 24) / row_bytes).max(1);
        let mut band: Vec<u8> = Vec::with_capacity(row_bytes * rows_per_band);

        let mut y = 0;
        while y < down {
            let rows = rows_per_band.min(down - y);
            band.clear();
            for line in y..y + rows {
                let source = line / self.scale;
                for x in 0..width {
                    let p = (source * width + x) * 3;
                    // The server's little-endian depth-24 pixel is blue, green, red, unused.
                    let pixel = [rgb[p + 2], rgb[p + 1], rgb[p], 0];
                    for _ in 0..self.scale {
                        band.extend_from_slice(&pixel);
                    }
                }
            }
            self.out.clear();
            self.out.push(PUT_IMAGE);
            self.out.push(ZPIXMAP);
            self.out
                .extend_from_slice(&(6u16 + (band.len() / 4) as u16).to_le_bytes());
            self.out.extend_from_slice(&self.window.to_le_bytes());
            self.out.extend_from_slice(&self.gc.to_le_bytes());
            self.out.extend_from_slice(&(across as u16).to_le_bytes());
            self.out.extend_from_slice(&(rows as u16).to_le_bytes());
            self.out.extend_from_slice(&(left as i16).to_le_bytes()); // destination x
            self.out
                .extend_from_slice(&((top + y as isize) as i16).to_le_bytes());
            self.out.push(0); // left pad
            self.out.push(self.depth);
            self.out.extend_from_slice(&0u16.to_le_bytes());
            self.out.extend_from_slice(&band);
            let bytes = std::mem::take(&mut self.out);
            let sent = self.write_all_nb(&bytes);
            self.out = bytes;
            sent?;
            y += rows;
        }
        Ok(())
    }

    /// Write a whole request to a non-blocking socket.
    ///
    /// A frame is about a megabyte and the socket's buffer is not, so this waits — and while it waits
    /// it keeps reading events, because a server blocked on sending us events while we are blocked on
    /// sending it an image is a deadlock, and draining is all it takes to avoid one.
    fn write_all_nb(&mut self, mut bytes: &[u8]) -> Result<(), String> {
        while !bytes.is_empty() {
            match self.stream.write(bytes) {
                Ok(0) => return Err("the display stopped reading".into()),
                Ok(n) => bytes = &bytes[n..],
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    self.poll();
                    if self.closed {
                        return Ok(());
                    }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("the display will not take a frame: {e}")),
            }
        }
        Ok(())
    }
}

/// Which keys are down, as the key events say — with X11's auto-repeat taken back out of them.
///
/// While a key is held, the server does not simply leave it down: about thirty times a second it
/// delivers a `KeyRelease` immediately followed by a `KeyPress` of the same keycode, both bearing the
/// same timestamp. Believed literally that is a key going up and down thirty times a second, which is
/// what a held key used to look like here — [`Window::held`] blinked false for a frame on every
/// repeat, and the simulation's press and release edges saw dozens of taps where a person held one
/// button down.
///
/// So a release is not applied when it arrives. It waits for the event after it, and if that is the
/// press of the same key at the same timestamp (give or take [`REPEAT_MS`]), the pair was a repeat
/// and the key never came up. A
/// release with anything else behind it — or nothing, which is what [`Keyboard::settle`] says — is a
/// person letting go.
///
/// The other way out would be `ChangeKeyboardControl` (opcode 102, no reply, which would suit this
/// module), but `auto-repeat-mode` is a property of the server's keyboard rather than of one window:
/// turning it off would stop the whole session repeating, including in the terminal the shell was
/// started from, and would leave it off if the shell died before putting it back. Reading the stream
/// correctly costs one `Option` and changes nothing outside this struct.
struct Keyboard {
    /// Which keys are down, by keycode.
    down: [bool; KEYCODES],
    /// A release that has arrived but is not believed yet: its keycode and its timestamp.
    pending: Option<(u8, u32)>,
    /// Keys that went down since the last poll — not a repeat's second half — down now or not.
    tapped: [bool; KEYCODES],
}

impl Keyboard {
    const fn new() -> Keyboard {
        Keyboard {
            down: [false; KEYCODES],
            pending: None,
            tapped: [false; KEYCODES],
        }
    }

    /// Whether a key is down now.
    fn held(&self, code: u8) -> bool {
        self.down[code as usize]
    }

    /// One `KeyPress` or `KeyRelease` event, as its 32 bytes: the keycode is the event's `detail` and
    /// the timestamp its first four-byte field.
    fn saw(&mut self, event: &[u8; 32]) {
        let press = event[0] & 0x7f == KEY_PRESS;
        let code = event[1];
        let time = le32(event, 4);
        match self.pending.take() {
            // The second half of a repeat: the key was never up, so neither event happened. The
            // subtraction wraps, which is also what rules out a press stamped *before* the release.
            Some((held, at)) if press && held == code && time.wrapping_sub(at) <= REPEAT_MS => {
                self.down[code as usize] = true;
                return;
            }
            // Something else followed the release, so the release was real. Apply it first: the
            // event in hand is later than it, even when it is a press of the same key.
            Some((held, _)) => self.down[held as usize] = false,
            None => {}
        }
        if press {
            self.down[code as usize] = true;
            self.tapped[code as usize] = true;
        } else {
            self.pending = Some((code, time));
        }
    }

    /// The server has said everything it had to say, so a release still being held back is a real
    /// one. The two halves of a repeat are written to the socket together, so the stream does not run
    /// dry between them.
    fn settle(&mut self) {
        if let Some((code, _)) = self.pending.take() {
            self.down[code as usize] = false;
        }
    }
}

/// Where a picture of `picture` pixels goes in a window of `window` pixels to sit in its middle: the
/// left and top offsets, negative where the picture is the larger (and the server clips it).
fn centred(picture: (usize, usize), window: (usize, usize)) -> (isize, isize) {
    (
        (window.0 as isize - picture.0 as isize) / 2,
        (window.1 as isize - picture.1 as isize) / 2,
    )
}

/// Read exactly `buf.len()` bytes if that many are there: `Ok(false)` means nothing was waiting.
///
/// Once the first byte of an event has arrived the rest is on its way, so a partial read spins rather
/// than giving up — an event is 32 bytes and a unix socket does not split them for long.
fn read_exact_nb(stream: &mut UnixStream, buf: &mut [u8]) -> Result<bool, std::io::Error> {
    let mut got = 0;
    while got < buf.len() {
        match stream.read(&mut buf[got..]) {
            Ok(0) => return Err(std::io::Error::from(ErrorKind::UnexpectedEof)),
            Ok(n) => got += n,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if got == 0 {
                    return Ok(false);
                }
                std::thread::sleep(std::time::Duration::from_micros(50));
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

/// What the shell keeps out of the connection setup.
struct Setup {
    id_base: u32,
    max_request_bytes: usize,
    image_byte_order: u8,
    root: u32,
    root_visual: u32,
    root_depth: u8,
    black_pixel: u32,
    bits_per_pixel: u8,
    /// The first screen's size in pixels.
    screen_width: usize,
    screen_height: usize,
}

/// Pick the fields the shell needs out of the setup reply's body.
///
/// The body is fixed fields, then the vendor's name, then one eight-byte format per pixel format, then
/// the screens — each of which is a fixed part followed by its depths and their visuals. Only the first
/// screen is read, and of the formats only the one matching the root's depth.
fn parse_setup(body: &[u8]) -> Result<Setup, String> {
    let short = || "the display's setup reply is too short".to_string();
    if body.len() < 32 {
        return Err(short());
    }
    let id_base = le32(body, 4);
    let vendor_len = le16(body, 16) as usize;
    let max_request_words = le16(body, 18) as usize;
    let formats = body[21] as usize;
    let image_byte_order = body[22];

    let mut at = 32 + vendor_len + (4 - vendor_len % 4) % 4;
    let format_table = at;
    at += formats * 8;
    if body.len() < at + 40 {
        return Err(short());
    }
    let root = le32(body, at);
    let black_pixel = le32(body, at + 12);
    // After the root, its colormap, the two pixels and the current input masks: the screen's width
    // and height in pixels, then the same in millimetres.
    let screen_width = le16(body, at + 20) as usize;
    let screen_height = le16(body, at + 22) as usize;
    let root_visual = le32(body, at + 32);
    let root_depth = body[at + 38];

    let bits_per_pixel = (0..formats)
        .map(|i| format_table + i * 8)
        .find(|&f| body[f] == root_depth)
        .map(|f| body[f + 1])
        .ok_or_else(|| format!("the display has no pixel format for depth {root_depth}"))?;

    Ok(Setup {
        id_base,
        // Zero would mean the server said nothing useful; 256 KiB is the protocol's own floor.
        max_request_bytes: if max_request_words == 0 {
            0x10000 * 4
        } else {
            max_request_words * 4
        },
        image_byte_order,
        root,
        root_visual,
        root_depth,
        black_pixel,
        bits_per_pixel,
        screen_width,
        screen_height,
    })
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The display number out of a `DISPLAY` like `:0`, `:0.1` or `unix:0`.
fn display_number(display: &str) -> Result<u32, String> {
    let after_colon = display
        .rsplit_once(':')
        .map(|(_, n)| n)
        .ok_or_else(|| format!("DISPLAY={display} has no display number"))?;
    let number = after_colon.split('.').next().unwrap_or("");
    number
        .parse()
        .map_err(|_| format!("DISPLAY={display}: {number} is not a display number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A setup reply's body as a server writes it: the fixed part, a vendor name that needs padding,
    /// two pixel formats, and one screen of `width x height` pixels with no depths listed after it.
    fn setup_body(width: u16, height: u16) -> Vec<u8> {
        let vendor = b"Example X Server 11"; // 19 bytes, so one byte of pad
        let mut b = vec![0u8; 32];
        b[4..8].copy_from_slice(&0x0060_0000u32.to_le_bytes()); // resource id base
        b[16..18].copy_from_slice(&(vendor.len() as u16).to_le_bytes());
        b[18..20].copy_from_slice(&0xffffu16.to_le_bytes()); // maximum request, in words
        b[20] = 1; // one screen
        b[21] = 2; // two formats
        b[22] = 0; // image byte order: little-endian
        b.extend_from_slice(vendor);
        b.push(0);
        b.extend_from_slice(&[1, 1, 32, 0, 0, 0, 0, 0]); // depth 1, 1 bit per pixel
        b.extend_from_slice(&[24, 32, 32, 0, 0, 0, 0, 0]); // depth 24, 32 bits per pixel
        let mut screen = [0u8; 40];
        screen[0..4].copy_from_slice(&0x0000_0123u32.to_le_bytes()); // root
        screen[12..16].copy_from_slice(&0u32.to_le_bytes()); // black pixel
        screen[20..22].copy_from_slice(&width.to_le_bytes());
        screen[22..24].copy_from_slice(&height.to_le_bytes());
        screen[24..26].copy_from_slice(&508u16.to_le_bytes()); // millimetres, which nothing reads
        screen[26..28].copy_from_slice(&286u16.to_le_bytes());
        screen[32..36].copy_from_slice(&0x0000_0021u32.to_le_bytes()); // root visual
        screen[38] = 24; // root depth
        b.extend_from_slice(&screen);
        b
    }

    #[test]
    fn the_setup_says_how_big_the_screen_is() {
        let setup = parse_setup(&setup_body(1920, 1080)).unwrap();
        assert_eq!((setup.screen_width, setup.screen_height), (1920, 1080));
        // And the fields around it are where they were, so the extents did not come from a neighbour.
        assert_eq!(setup.root, 0x123);
        assert_eq!(setup.root_visual, 0x21);
        assert_eq!(setup.root_depth, 24);
        assert_eq!(setup.bits_per_pixel, 32);
        assert_eq!(setup.max_request_bytes, 0xffff * 4);
        let setup = parse_setup(&setup_body(2560, 1440)).unwrap();
        assert_eq!((setup.screen_width, setup.screen_height), (2560, 1440));
    }

    /// A window with the far end of a socket pair standing in for the server: what the shell sends
    /// can be read back, and events can be handed to it as the server's would be.
    fn window_on_a_pair(width: usize, height: usize, scale: usize) -> (Window, UnixStream) {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let w = Window {
            stream: ours,
            window: 0x0060_0001,
            gc: 0x0060_0002,
            width,
            height,
            scale,
            screen: (1920, 1080),
            resized: false,
            depth: 24,
            max_request: 0xffff * 4,
            keys: Keyboard::new(),
            closed: false,
            out: Vec::new(),
        };
        (w, theirs)
    }

    /// A `ConfigureNotify` for a window now `width x height`.
    fn configure(width: u16, height: u16) -> [u8; 32] {
        let mut e = [0u8; 32];
        e[0] = CONFIGURE_NOTIFY;
        e[4..8].copy_from_slice(&0x0060_0001u32.to_le_bytes()); // event window
        e[8..12].copy_from_slice(&0x0060_0001u32.to_le_bytes()); // window
        e[16..18].copy_from_slice(&40i16.to_le_bytes()); // x
        e[18..20].copy_from_slice(&30i16.to_le_bytes()); // y
        e[20..22].copy_from_slice(&width.to_le_bytes());
        e[22..24].copy_from_slice(&height.to_le_bytes());
        e
    }

    #[test]
    fn a_resize_is_read_and_said_once() {
        let (mut w, _server) = window_on_a_pair(1920, 1080, 1);
        assert!(!w.resized());
        // The window manager tiles it to half the screen.
        w.handle(&configure(958, 1046));
        assert_eq!((w.width, w.height), (958, 1046));
        assert!(w.resized());
        assert!(!w.resized(), "a resize is said once, not every frame after it");
        // A move carries the same size again, and is not a resize.
        w.handle(&configure(958, 1046));
        assert!(!w.resized());
        // A sent event (the top bit of the code) is read the same.
        let mut sent = configure(1280, 720);
        sent[0] |= 0x80;
        w.handle(&sent);
        assert_eq!((w.width, w.height), (1280, 720));
        assert!(w.resized());
        // The frame drawn for it is `--aspect`'s (`crate::aspect`, tests/aspect.rs).
    }

    #[test]
    fn a_frame_goes_in_the_middle_of_a_wider_window() {
        // A 4x3 frame at scale 2 is 8x6 window pixels; in a 16x6 window it starts 4 pixels in.
        let (mut w, mut server) = window_on_a_pair(16, 6, 2);
        let rgb: Vec<u8> = (0..4 * 3).flat_map(|i| [i as u8, 0x10, 0x20]).collect();
        w.present(&rgb, 4, 3).unwrap();
        let mut request = vec![0u8; 24 + 8 * 6 * 4];
        server.read_exact(&mut request).unwrap();
        assert_eq!(request[0], PUT_IMAGE);
        assert_eq!(le16(&request, 2) as usize, request.len() / 4);
        assert_eq!(le16(&request, 12), 8, "width: the picture's, not the window's");
        assert_eq!(le16(&request, 14), 6);
        assert_eq!(le16(&request, 16) as i16, 4, "destination x");
        assert_eq!(le16(&request, 18) as i16, 0, "destination y");
        // Each frame pixel twice across and twice down, blue first.
        let pixel = |x: usize, y: usize| &request[24 + (y * 8 + x) * 4..][..4];
        assert_eq!(pixel(0, 0), [0x20, 0x10, 0, 0]);
        assert_eq!(pixel(1, 1), [0x20, 0x10, 0, 0]);
        assert_eq!(pixel(2, 0), [0x20, 0x10, 1, 0]);
        assert_eq!(pixel(7, 5), [0x20, 0x10, 11, 0]);
    }

    #[test]
    fn a_display_name_gives_its_socket_number() {
        assert_eq!(display_number(":0"), Ok(0));
        assert_eq!(display_number(":1.0"), Ok(1));
        assert_eq!(display_number("unix:3"), Ok(3));
        assert!(display_number("0").is_err());
    }

    /// A key event as the server writes it: the code, the keycode in `detail`, and the timestamp in
    /// the event's first four-byte field. The rest is windows and coordinates the shell never reads.
    fn event(kind: u8, code: u8, time: u32) -> [u8; 32] {
        let mut e = [0u8; 32];
        e[0] = kind;
        e[1] = code;
        e[4..8].copy_from_slice(&time.to_le_bytes());
        e
    }

    /// Feed a whole poll's worth of events, as `poll` does: every event, then the settle at the end.
    fn stream(keys: &mut Keyboard, events: &[[u8; 32]]) {
        for e in events {
            keys.saw(e);
        }
        keys.settle();
    }

    #[test]
    fn an_auto_repeat_pair_leaves_the_key_held() {
        let mut keys = Keyboard::new();
        // Down, then five frames' worth of repeats at thirty a second — the stream a held key makes.
        stream(&mut keys, &[event(KEY_PRESS, key::X, 1000)]);
        assert!(keys.held(key::X));
        for frame in 0..5 {
            let at = 1033 + frame * 33;
            stream(
                &mut keys,
                &[event(KEY_RELEASE, key::X, at), event(KEY_PRESS, key::X, at)],
            );
            assert!(keys.held(key::X), "the repeat at {at} dropped the key");
        }
        // A pair a millisecond apart is a repeat too, which is the slack in `REPEAT_MS`.
        stream(
            &mut keys,
            &[
                event(KEY_RELEASE, key::X, 1200),
                event(KEY_PRESS, key::X, 1201),
            ],
        );
        assert!(keys.held(key::X));
        // And it still comes up when the person lets go.
        stream(&mut keys, &[event(KEY_RELEASE, key::X, 1233)]);
        assert!(!keys.held(key::X));
    }

    #[test]
    fn a_repeat_does_not_blink_even_part_way_through_the_stream() {
        let mut keys = Keyboard::new();
        keys.saw(&event(KEY_PRESS, key::LEFT, 1));
        // The release half on its own, with the press half still in the socket: nothing has settled,
        // so the key must not have moved. This is the frame the old reader lost.
        keys.saw(&event(KEY_RELEASE, key::LEFT, 34));
        assert!(keys.held(key::LEFT));
        keys.saw(&event(KEY_PRESS, key::LEFT, 34));
        keys.settle();
        assert!(keys.held(key::LEFT));
    }

    #[test]
    fn a_release_is_believed_whatever_follows_it() {
        // Another key's press behind it, which is two fingers and not a repeat.
        let mut keys = Keyboard::new();
        stream(
            &mut keys,
            &[event(KEY_PRESS, key::A, 10), event(KEY_PRESS, key::S, 11)],
        );
        stream(
            &mut keys,
            &[event(KEY_RELEASE, key::A, 20), event(KEY_PRESS, key::D, 20)],
        );
        assert!(!keys.held(key::A), "a release is not a repeat of another key");
        assert!(keys.held(key::S));
        assert!(keys.held(key::D));

        // And a real tap of the same key, which the server stamps with a later time: up, then down.
        let mut keys = Keyboard::new();
        stream(&mut keys, &[event(KEY_PRESS, key::Z, 10)]);
        stream(&mut keys, &[event(KEY_RELEASE, key::Z, 90)]);
        assert!(!keys.held(key::Z));
        stream(&mut keys, &[event(KEY_PRESS, key::Z, 150)]);
        assert!(keys.held(key::Z));
    }
}
