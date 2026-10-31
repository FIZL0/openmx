//! The calls themselves: Objective-C messages through `objc_msgSend` cast to each message's own
//! signature (arm64 has one entry for every return type, structs of four doubles included), and the
//! C frameworks' functions declared as their headers do.

use std::ffi::{CStr, c_void};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

type Id = *mut c_void;

#[link(name = "Foundation", kind = "framework")]
#[link(name = "UIKit", kind = "framework")]
#[link(name = "GameController", kind = "framework")]
#[link(name = "CoreMotion", kind = "framework")]
#[link(name = "AudioToolbox", kind = "framework")]
#[link(name = "CoreHaptics", kind = "framework")]
#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {
    fn objc_msgSend();
    fn objc_getClass(name: *const std::ffi::c_char) -> Id;
    fn sel_registerName(name: *const std::ffi::c_char) -> Id;
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
    fn NSSetUncaughtExceptionHandler(handler: extern "C" fn(Id));
    fn pthread_main_np() -> i32;
}

fn sel(name: &CStr) -> Id {
    // SAFETY: registering a selector name is always allowed
    unsafe { sel_registerName(name.as_ptr()) }
}

fn class(name: &CStr) -> Id {
    // SAFETY: a lookup; nil when the class is not there
    unsafe { objc_getClass(name.as_ptr()) }
}

/// `[o name]` for an object-returning message; nil in, nil out.
fn get(o: Id, name: &CStr) -> Id {
    if o.is_null() {
        return o;
    }
    // SAFETY: objc_msgSend cast to `id (id, SEL)`, the signature of every getter it is used for
    unsafe {
        let f: extern "C" fn(Id, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
        f(o, sel(name))
    }
}

/// `[o name]` for an `NSInteger` / `NSUInteger` getter; 0 for nil.
fn int(o: Id, name: &CStr) -> isize {
    if o.is_null() {
        return 0;
    }
    // SAFETY: objc_msgSend cast to `NSInteger (id, SEL)`
    unsafe {
        let f: extern "C" fn(Id, Id) -> isize = std::mem::transmute(objc_msgSend as *const ());
        f(o, sel(name))
    }
}

/// A `GCControllerButtonInput` / `GCControllerAxisInput`'s `value` (a `float`); 0 for nil.
fn float(o: Id, name: &CStr) -> f32 {
    if o.is_null() {
        return 0.0;
    }
    // SAFETY: objc_msgSend cast to `float (id, SEL)`
    unsafe {
        let f: extern "C" fn(Id, Id) -> f32 = std::mem::transmute(objc_msgSend as *const ());
        f(o, sel(name))
    }
}

/// `[o name]` for a `BOOL` getter; false for nil.
fn flag(o: Id, name: &CStr) -> bool {
    if o.is_null() {
        return false;
    }
    // SAFETY: objc_msgSend cast to `BOOL (id, SEL)`
    unsafe {
        let f: extern "C" fn(Id, Id) -> bool = std::mem::transmute(objc_msgSend as *const ());
        f(o, sel(name))
    }
}

/// An `NSString`'s text; empty for nil.
fn text(s: Id) -> String {
    let utf8 = get(s, c"UTF8String") as *const std::ffi::c_char;
    if utf8.is_null() {
        return String::new();
    }
    // SAFETY: UTF8String is a NUL-terminated string that lives as long as the autorelease pool
    unsafe { CStr::from_ptr(utf8) }.to_string_lossy().into_owned()
}

/// Run `f` inside an autorelease pool, as any thread but the main one must for messages that hand
/// back autoreleased objects.
fn pool<T>(f: impl FnOnce() -> T) -> T {
    // SAFETY: push and pop pair up around `f`
    let p = unsafe { objc_autoreleasePoolPush() };
    let out = f();
    unsafe { objc_autoreleasePoolPop(p) };
    out
}

fn main_thread() -> bool {
    // SAFETY: a plain query
    unsafe { pthread_main_np() != 0 }
}

// ---- files and the log ---------------------------------------------------------------------------

/// The app's Documents folder: what the Files app shows as On My iPhone → openMX
/// (UIFileSharingEnabled + LSSupportsOpeningDocumentsInPlace in Info.plist). The disc image,
/// settings.txt, controls.txt and mods/ go in it.
pub fn documents() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/")).join("Documents")
}

/// stdout and stderr into `path`, fresh each launch (opentennis M105): the shell's own lines,
/// panics, and the crash reports below. iOS shows nothing else without a Mac, and writing a file
/// also makes iOS list the app's folder in Files, which it only does once there is one in it.
///
/// The launch before's log is kept beside it as [`crate::previous_log`] (P212): a player who saw something
/// wrong and opened the app again to fetch the log would otherwise find only the new launch's.
pub fn log_to(path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    crate::keep_previous_log(path);
    if let Ok(f) = std::fs::File::create(path) {
        // SAFETY: dup2 of an open file over fds 1 and 2; `f` closes its own fd after
        unsafe {
            libc::dup2(f.as_raw_fd(), 1);
            libc::dup2(f.as_raw_fd(), 2);
        }
    }
    // A close with no line and no crash report says nothing: these say which kind of end it was.
    // SAFETY: the handlers are `extern "C"` functions that live for the process; on_signal makes
    // only async-signal-safe calls
    unsafe {
        libc::atexit(on_exit);
        for sig in [libc::SIGABRT, libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGTRAP, libc::SIGFPE] {
            libc::signal(sig, on_signal as *const () as libc::sighandler_t);
        }
        NSSetUncaughtExceptionHandler(on_exception);
    }
}

extern "C" fn on_exit() {
    eprintln!("exit() called");
}

extern "C" fn on_signal(sig: libc::c_int) {
    let msg: &[u8] = match sig {
        libc::SIGABRT => b"fatal signal: SIGABRT (abort)\n",
        libc::SIGSEGV => b"fatal signal: SIGSEGV (bad memory access)\n",
        libc::SIGBUS => b"fatal signal: SIGBUS (bad memory access)\n",
        libc::SIGILL => b"fatal signal: SIGILL\n",
        libc::SIGTRAP => b"fatal signal: SIGTRAP\n",
        _ => b"fatal signal: SIGFPE\n",
    };
    // SAFETY: write, signal and raise are async-signal-safe; `msg` is a static byte string
    unsafe {
        libc::write(2, msg.as_ptr().cast(), msg.len());
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

/// UIKit's failures are Objective-C exceptions: their name and reason, before the abort.
extern "C" fn on_exception(e: Id) {
    eprintln!("Objective-C exception {}: {}", text(get(e, c"name")), text(get(e, c"reason")));
}

// ---- focus (P233) ---------------------------------------------------------------------------------

/// A focus notice not yet taken by [`focus_notice`]: the controllers connected then, plus one (0: none).
static NOTICE: AtomicUsize = AtomicUsize::new(0);

/// A block literal as clang lays one out: a global one, which nothing copies or frees.
#[repr(C)]
struct Block {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: extern "C" fn(*const Block, Id),
    descriptor: *const BlockDescriptor,
}

#[repr(C)]
struct BlockDescriptor {
    reserved: usize,
    size: usize,
}

/// `BLOCK_IS_GLOBAL`.
const BLOCK_IS_GLOBAL: i32 = 1 << 28;

unsafe extern "C" {
    static _NSConcreteGlobalBlock: c_void;
}

/// The screenshot notice, the only one that raises "Tap screen to resume controller": the user's
/// iPhone run (2026-10-30) showed a trip out of the app and back leaves the controllers working.
const SCREENSHOT: &str = "UIApplicationUserDidTakeScreenshotNotification";

/// The focus block's body, on the main queue: every notice into the log, and a screenshot's kept
/// with the controllers in for [`focus_notice`]. WillResignActive and DidBecomeActive only log.
extern "C" fn on_focus(_: *const Block, note: Id) {
    pool(|| {
        let pads = int(get(class(c"GCController"), c"controllers"), c"count").max(0) as usize;
        let name = text(get(note, c"name"));
        eprintln!("iOS {name} ({pads} controllers)");
        if name.starts_with("NSProcessInfo") {
            eprintln!("iOS {}", power());
        }
        if name == SCREENSHOT {
            NOTICE.fetch_max(pads + 1, Ordering::Relaxed);
        }
    });
}

/// As opentennis's `watch_focus`: a screenshot can leave the controllers silent until a touch (the
/// shell's "Tap screen to resume controller"). Observes UIApplicationUserDidTakeScreenshotNotification,
/// WillResignActive and DidBecomeActive on the main queue, for the life of the process; only the
/// screenshot one reaches [`focus_notice`], the other two are logged. Low Power Mode and thermal
/// changes are logged with [`power`] too (P238). Call once.
pub fn watch_focus() {
    let descriptor = Box::leak(Box::new(BlockDescriptor { reserved: 0, size: std::mem::size_of::<Block>() }));
    let block: &'static Block = Box::leak(Box::new(Block {
        // only the address of the runtime's global block class
        isa: &raw const _NSConcreteGlobalBlock,
        flags: BLOCK_IS_GLOBAL,
        reserved: 0,
        invoke: on_focus,
        descriptor,
    }));
    pool(|| {
        let center = get(class(c"NSNotificationCenter"), c"defaultCenter");
        let main = get(class(c"NSOperationQueue"), c"mainQueue");
        for name in [
            c"UIApplicationUserDidTakeScreenshotNotification",
            c"UIApplicationWillResignActiveNotification",
            c"UIApplicationDidBecomeActiveNotification",
            c"NSProcessInfoPowerStateDidChangeNotification",
            c"NSProcessInfoThermalStateDidChangeNotification",
        ] {
            // SAFETY: objc_msgSend cast to `id (id, SEL, const char *)` and to
            // `id (id, SEL, NSString *, id, NSOperationQueue *, block)`, as NSString and
            // NSNotificationCenter declare them; the block lives for the process
            let observer = unsafe {
                let string: extern "C" fn(Id, Id, *const std::ffi::c_char) -> Id = std::mem::transmute(objc_msgSend as *const ());
                let ns = string(class(c"NSString"), sel(c"stringWithUTF8String:"), name.as_ptr());
                let add: extern "C" fn(Id, Id, Id, Id, Id, *const Block) -> Id = std::mem::transmute(objc_msgSend as *const ());
                add(center, sel(c"addObserverForName:object:queue:usingBlock:"), ns, std::ptr::null_mut(), main, block)
            };
            retain(observer);
        }
    });
}

/// The screenshot notice since the last call, if one came: how many controllers were connected then.
/// Any thread.
pub fn focus_notice() -> Option<usize> {
    NOTICE.swap(0, Ordering::Relaxed).checked_sub(1)
}

/// Low Power Mode and the thermal state, for the log (P238): either can hold the picture down.
pub fn power() -> String {
    pool(|| {
        let info = get(class(c"NSProcessInfo"), c"processInfo");
        let thermal = match int(info, c"thermalState") {
            0 => "nominal",
            1 => "fair",
            2 => "serious",
            3 => "critical",
            _ => "unknown",
        };
        format!("low power mode {}, thermal state {thermal}", if flag(info, c"isLowPowerModeEnabled") { "on" } else { "off" })
    })
}

/// QuartzCore's `CAFrameRateRange`.
#[repr(C)]
struct FrameRates {
    minimum: f32,
    maximum: f32,
    preferred: f32,
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    static NSRunLoopCommonModes: Id;
}

/// Ask the screen for its top rate (P238): a ProMotion iPhone otherwise picks its own refresh for a
/// Metal layer, and held the ride at a dead 30 fps. A CADisplayLink on the main run loop, for the
/// life of the process, with `preferredFrameRateRange` up to `maximumFramesPerSecond`, is how iOS
/// is told; it calls `[NSObject self]`, which does nothing. A frame limit that divides the top rate
/// asks for itself instead ([`set_frame_rate`]).
/// The screen's top rate, for the log. Call once, on the main thread.
pub fn ask_frame_rate() -> f32 {
    pool(|| {
        let most = int(get(class(c"UIScreen"), c"mainScreen"), c"maximumFramesPerSecond").max(60) as f32;
        let target = get(get(class(c"NSObject"), c"alloc"), c"init");
        // SAFETY: objc_msgSend cast to `id (id, SEL, id, SEL)`, `void (id, SEL, CAFrameRateRange)` and
        // `void (id, SEL, NSRunLoop *, NSRunLoopMode)`, as CADisplayLink declares them (iOS 15+);
        // the link is retained for the process and retains its target
        unsafe {
            let make: extern "C" fn(Id, Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let link = make(class(c"CADisplayLink"), sel(c"displayLinkWithTarget:selector:"), target, sel(c"self"));
            if link.is_null() {
                return most;
            }
            let range: extern "C" fn(Id, Id, FrameRates) = std::mem::transmute(objc_msgSend as *const ());
            range(link, sel(c"setPreferredFrameRateRange:"), FrameRates { minimum: most.min(60.0), maximum: most, preferred: most });
            let add: extern "C" fn(Id, Id, Id, Id) = std::mem::transmute(objc_msgSend as *const ());
            add(link, sel(c"addToRunLoop:forMode:"), get(class(c"NSRunLoop"), c"mainRunLoop"), NSRunLoopCommonModes);
            retain(link);
            LINK.store(link as usize, Ordering::Relaxed);
            TOP.store(most as usize, Ordering::Relaxed);
        }
        most
    })
}

/// [`ask_frame_rate`]'s display link, retained for the process, and the screen's top rate.
static LINK: AtomicUsize = AtomicUsize::new(0);
static TOP: AtomicUsize = AtomicUsize::new(0);

/// Ask the screen for `fps` exactly (P248): a frame limit of 60 on a 120 Hz iPhone, kept by a sleep,
/// put its pictures on every one, two or three refreshes and juddered at a flat 60 a second; with the
/// screen itself at 60, each present waits one refresh. None asks for the top rate again. Main
/// thread only, after [`ask_frame_rate`].
pub fn set_frame_rate(fps: Option<f32>) {
    let link = LINK.load(Ordering::Relaxed) as Id;
    if link.is_null() || !main_thread() {
        return;
    }
    let most = TOP.load(Ordering::Relaxed) as f32;
    let rates = match fps {
        Some(fps) => FrameRates { minimum: fps, maximum: fps, preferred: fps },
        None => FrameRates { minimum: most.min(60.0), maximum: most, preferred: most },
    };
    // SAFETY: objc_msgSend cast to `void (id, SEL, CAFrameRateRange)`, as CADisplayLink declares it;
    // the link is retained for the process
    unsafe {
        let range: extern "C" fn(Id, Id, FrameRates) = std::mem::transmute(objc_msgSend as *const ());
        range(link, sel(c"setPreferredFrameRateRange:"), rates);
    }
}

unsafe extern "C" {
    static mach_task_self_: u32;
    fn task_info(task: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn os_proc_available_memory() -> usize;
}

/// The memory iOS counts against the app (its phys footprint) and what is left before iOS closes
/// it (opentennis B538), for the log.
pub fn memory() -> String {
    // task_vm_info_data_t as u64s: phys_footprint is the 19th
    let mut info = [0u64; 48];
    let mut count = (std::mem::size_of_val(&info) / 4) as u32;
    const TASK_VM_INFO: i32 = 22;
    // SAFETY: `info` holds `count` natural_t's; the kernel writes at most that many
    let ok = unsafe { task_info(mach_task_self_, TASK_VM_INFO, info.as_mut_ptr().cast(), &mut count) } == 0;
    // SAFETY: a plain query (iOS 13+)
    let left = unsafe { os_proc_available_memory() } as u64;
    format!("memory {} MB, {} MB left before iOS closes the app", if ok { info[18] >> 20 } else { 0 }, left >> 20)
}

/// The device's model identifier (`iPhone16,1`), which the Auto video preset picks by (P223,
/// opentennis's P53).
pub fn model() -> Option<String> {
    let mut buf = [0u8; 64];
    let mut len = buf.len();
    // SAFETY: `buf` holds `len` bytes; sysctlbyname writes at most that and sets `len`
    let r = unsafe { libc::sysctlbyname(c"hw.machine".as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) };
    (r == 0).then(|| String::from_utf8_lossy(&buf[..len.min(buf.len())]).trim_end_matches('\0').to_string())
}

/// The device's memory in MB, 0 when it will not say.
pub fn memory_mb() -> u32 {
    let mut v = 0u64;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: as `model`, into a u64
    let r = unsafe { libc::sysctlbyname(c"hw.memsize".as_ptr(), (&mut v as *mut u64).cast(), &mut len, std::ptr::null_mut(), 0) };
    if r == 0 { (v >> 20) as u32 } else { 0 }
}

// ---- the window ----------------------------------------------------------------------------------

/// UIKit's `UIEdgeInsets`.
#[repr(C)]
#[derive(Clone, Copy)]
struct Insets {
    top: f64,
    left: f64,
    bottom: f64,
    right: f64,
}

fn view_of(window: &winit::window::Window) -> Option<(Id, Option<Id>)> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::UiKit(h) => Some((h.ui_view.as_ptr(), h.ui_view_controller.map(|v| v.as_ptr()))),
        _ => None,
    }
}

/// The window's safe area, in its pixels: the left, top, right and bottom insets the notch or
/// Dynamic Island and the home indicator take. Main thread only (UIKit); zeros elsewhere.
pub fn safe_area(window: &winit::window::Window) -> [u32; 4] {
    let Some((view, _)) = view_of(window).filter(|_| main_thread()) else {
        return [0; 4];
    };
    // SAFETY: on the main thread; `view` is the window's live UIView; safeAreaInsets returns
    // UIEdgeInsets (four doubles, returned in registers on arm64) by value
    let i = unsafe {
        let f: extern "C" fn(Id, Id) -> Insets = std::mem::transmute(objc_msgSend as *const ());
        f(view, sel(c"safeAreaInsets"))
    };
    let scale = window.scale_factor();
    [i.left, i.top, i.right, i.bottom].map(|v| (v * scale).ceil().max(0.0) as u32)
}

/// Hold the window to `hold` (M144, B694): its view controller's orientations, and on iOS 16 and
/// later the call that makes UIKit turn to them now. Main thread only.
pub fn hold(window: &winit::window::Window, hold: crate::Hold) {
    use winit::platform::ios::{ValidOrientations, WindowExtIOS};
    window.set_valid_orientations(match hold {
        crate::Hold::Landscape => ValidOrientations::Landscape,
        crate::Hold::Portrait => ValidOrientations::Portrait,
        crate::Hold::Locked => ValidOrientations::LandscapeRight,
    });
    if let Some((_, Some(vc))) = view_of(window).filter(|_| main_thread()) {
        let update = sel(c"setNeedsUpdateOfSupportedInterfaceOrientations");
        // SAFETY: on the main thread; the view controller lives as long as the window;
        // respondsToSelector: guards iOS 15, which lacks the call
        unsafe {
            let responds: extern "C" fn(Id, Id, Id) -> bool = std::mem::transmute(objc_msgSend as *const ());
            if responds(vc, sel(c"respondsToSelector:"), update) {
                let f: extern "C" fn(Id, Id) = std::mem::transmute(objc_msgSend as *const ());
                f(vc, update);
            }
        }
    }
}

/// CoreMotion's `CMAcceleration`, in g along the device's axes (x right, y up the screen held upright).
#[repr(C)]
#[derive(Clone, Copy)]
struct Acceleration {
    x: f64,
    y: f64,
    _z: f64,
}

/// The motion manager, made on first use and kept for the process.
static MOTION: AtomicUsize = AtomicUsize::new(0);

/// What [`crate::Holder::step`] reads: UIDevice's orientation, and whether the accelerometer has the
/// phone sideways (`None` while it lies flat). Main thread only.
pub fn device_turn() -> (isize, Option<bool>) {
    if !main_thread() {
        return (0, None);
    }
    let device = get(class(c"UIDevice"), c"currentDevice");
    if MOTION.load(Ordering::Relaxed) == 0 {
        get(device, c"beginGeneratingDeviceOrientationNotifications");
        let m = get(get(class(c"CMMotionManager"), c"alloc"), c"init");
        get(m, c"startAccelerometerUpdates");
        MOTION.store(m as usize, Ordering::Relaxed);
    }
    let data = get(MOTION.load(Ordering::Relaxed) as Id, c"accelerometerData");
    let gravity = (!data.is_null())
        .then(|| {
            // SAFETY: objc_msgSend cast to `CMAcceleration (id, SEL)`, three doubles in registers
            unsafe {
                let f: extern "C" fn(Id, Id) -> Acceleration = std::mem::transmute(objc_msgSend as *const ());
                f(data, sel(c"acceleration"))
            }
        })
        .filter(|a| a.x.abs().max(a.y.abs()) > 0.5)
        .map(|a| a.x.abs() > a.y.abs());
    (int(device, c"orientation"), gravity)
}

// ---- pads ----------------------------------------------------------------------------------------

/// The extended gamepad's buttons, in the Gamepad API's standard mapping (the order the shell's web
/// pads come in, `web_host.rs`): A B X Y (Apple's bottom, right, left, top), the shoulders, the
/// triggers, options (select), menu (start), the stick clicks, the d-pad, home.
const BUTTONS: [&[&CStr]; 17] = [
    &[c"buttonA"],
    &[c"buttonB"],
    &[c"buttonX"],
    &[c"buttonY"],
    &[c"leftShoulder"],
    &[c"rightShoulder"],
    &[c"leftTrigger"],
    &[c"rightTrigger"],
    &[c"buttonOptions"],
    &[c"buttonMenu"],
    &[c"leftThumbstickButton"],
    &[c"rightThumbstickButton"],
    &[c"dpad", c"up"],
    &[c"dpad", c"down"],
    &[c"dpad", c"left"],
    &[c"dpad", c"right"],
    &[c"buttonHome"],
];
/// The sticks, standard mapping: left x, left y, right x, right y, y down.
const AXES: [&[&CStr]; 4] = [
    &[c"leftThumbstick", c"xAxis"],
    &[c"leftThumbstick", c"yAxis"],
    &[c"rightThumbstick", c"xAxis"],
    &[c"rightThumbstick", c"yAxis"],
];

/// One controller as it is now, in the Gamepad API's standard mapping.
#[derive(Clone, Debug)]
pub struct PadState {
    /// The controller object's address: the same for as long as it stays connected.
    pub id: usize,
    pub name: String,
    pub buttons: [f32; 17],
    pub axes: [f32; 4],
}

/// Every connected GameController with an extended gamepad (any MFi, Xbox, PlayStation or Switch
/// pad). Any thread.
pub fn pads() -> Vec<PadState> {
    if class(c"GCController").is_null() {
        return Vec::new();
    }
    pool(|| {
        let list = get(class(c"GCController"), c"controllers");
        let n = int(list, c"count").max(0) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            // SAFETY: objc_msgSend cast to `id (id, SEL, NSUInteger)`, `i` below the array's count
            let ctrl = unsafe {
                let f: extern "C" fn(Id, Id, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
                f(list, sel(c"objectAtIndex:"), i)
            };
            let pad = get(ctrl, c"extendedGamepad");
            if pad.is_null() {
                continue;
            }
            let element = |path: &[&CStr]| path.iter().fold(pad, |o, name| get(o, name));
            let mut state = PadState { id: ctrl as usize, name: text(get(ctrl, c"vendorName")), buttons: [0.0; 17], axes: [0.0; 4] };
            for (b, path) in state.buttons.iter_mut().zip(BUTTONS) {
                *b = float(element(path), c"value");
            }
            for (a, path) in state.axes.iter_mut().zip(AXES) {
                *a = float(element(path), c"value");
            }
            // GameController's y is up; the standard mapping's is down.
            state.axes[1] = -state.axes[1];
            state.axes[3] = -state.axes[3];
            if state.name.is_empty() {
                state.name = "Controller".into();
            }
            out.push(state);
        }
        out
    })
}

// ---- rumble -------------------------------------------------------------------------------------

unsafe extern "C" {
    static GCHapticsLocalityDefault: Id;
    static CHHapticEventTypeHapticContinuous: Id;
    static CHHapticEventParameterIDHapticIntensity: Id;
    fn AudioServicesPlaySystemSound(sound: u32);
}

/// AudioToolbox's `kSystemSoundID_Vibrate`: the phone's one buzz, for a phone CoreHaptics cannot drive.
const SYSTEM_SOUND_VIBRATE: u32 = 4095;

/// What a rumble shakes (P183 on the phone): a controller's own haptics, by its [`PadState::id`], or
/// the phone's (the touch pad's seat, which has no controller to shake).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Haptics {
    Controller(usize),
    Phone,
}

/// One ask: a continuous haptic event at `intensity` (0..=1) for `seconds`.
struct Ask {
    to: Haptics,
    intensity: f32,
    seconds: f64,
}

static ASKS: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Sender<Ask>>> = std::sync::OnceLock::new();

/// Rumble `to` at `intensity` (0..=1) for `seconds`. Any thread, and never waits: the ask goes to a
/// thread of its own, which makes, starts and keeps one CHHapticEngine per controller and one for
/// the phone — an engine's start can take a while, and a frame must not wait for it. A controller
/// with no haptics (or gone) shakes nothing; a phone CoreHaptics cannot drive buzzes the system's
/// vibrate, which has no level and no length.
pub fn rumble(to: Haptics, intensity: f32, seconds: f32) {
    if seconds <= 0.0 || intensity <= 0.0 {
        return;
    }
    let asks = ASKS.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Ask>();
        let _ = std::thread::Builder::new().name("haptics".into()).spawn(move || {
            let mut engines = Engines::default();
            for ask in rx {
                pool(|| engines.play(ask));
            }
        });
        std::sync::Mutex::new(tx)
    });
    let ask = Ask { to, intensity: intensity.min(1.0), seconds: seconds as f64 };
    let _ = asks.lock().unwrap_or_else(|e| e.into_inner()).send(ask);
}

/// `[o retain]` / `[o release]`, nil-safe.
fn retain(o: Id) -> Id {
    get(o, c"retain")
}

fn release(o: Id) {
    get(o, c"release");
}

/// A BOOL message whose one argument is an `NSError **` (`startAndReturnError:`): nil for the
/// error, which nothing reads.
fn ok(o: Id, name: &CStr) -> bool {
    if o.is_null() {
        return false;
    }
    // SAFETY: objc_msgSend cast to `BOOL (id, SEL, NSError **)`
    unsafe {
        let f: extern "C" fn(Id, Id, *mut Id) -> bool = std::mem::transmute(objc_msgSend as *const ());
        f(o, sel(name), std::ptr::null_mut())
    }
}

/// `[list objectAtIndex:i]` for each of an NSArray's objects.
fn objects(list: Id) -> Vec<Id> {
    let n = int(list, c"count").max(0) as usize;
    (0..n)
        .map(|i| {
            // SAFETY: objc_msgSend cast to `id (id, SEL, NSUInteger)`, `i` below the array's count
            unsafe {
                let f: extern "C" fn(Id, Id, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
                f(list, sel(c"objectAtIndex:"), i)
            }
        })
        .collect()
}

/// One CHHapticEngine and the player it last started, both retained.
struct Engine {
    engine: Id,
    player: Id,
}

impl Engine {
    /// Start the engine — again: one stopped or reset (the app sent to the background, the haptic
    /// server restarted; its `stoppedHandler` / `resetHandler` would say so) plays nothing until it is
    /// started, and a start of a running one costs little — then one continuous event on it, at
    /// CHHapticTimeImmediate. The last ask's player is stopped first, so a new ask starts the count
    /// again as the original's does. Whether it played.
    fn play(&mut self, intensity: f32, seconds: f64) -> bool {
        if !ok(self.engine, c"startAndReturnError:") {
            return false;
        }
        // SAFETY: objc_msgSend cast to each message's signature as CoreHaptics' headers have it (the
        // parameter's value a float, the times NSTimeIntervals); each alloc'd object is released once
        // the next holds it, the arrays are autoreleased into the caller's pool, and the player is
        // kept until the next ask
        unsafe {
            let at: extern "C" fn(Id, Id, f64, *mut Id) -> bool = std::mem::transmute(objc_msgSend as *const ());
            if !self.player.is_null() {
                at(self.player, sel(c"stopAtTime:error:"), 0.0, std::ptr::null_mut());
                release(self.player);
                self.player = std::ptr::null_mut();
            }
            let array: extern "C" fn(Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let new_param: extern "C" fn(Id, Id, Id, f32) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let param = new_param(
                get(class(c"CHHapticEventParameter"), c"alloc"),
                sel(c"initWithParameterID:value:"),
                CHHapticEventParameterIDHapticIntensity,
                intensity,
            );
            let new_event: extern "C" fn(Id, Id, Id, Id, f64, f64) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let event = new_event(
                get(class(c"CHHapticEvent"), c"alloc"),
                sel(c"initWithEventType:parameters:relativeTime:duration:"),
                CHHapticEventTypeHapticContinuous,
                array(class(c"NSArray"), sel(c"arrayWithObject:"), param),
                0.0,
                seconds,
            );
            release(param);
            let new_pattern: extern "C" fn(Id, Id, Id, Id, *mut Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let pattern = new_pattern(
                get(class(c"CHHapticPattern"), c"alloc"),
                sel(c"initWithEvents:parameters:error:"),
                array(class(c"NSArray"), sel(c"arrayWithObject:"), event),
                get(class(c"NSArray"), c"array"),
                std::ptr::null_mut(),
            );
            release(event);
            if pattern.is_null() {
                return false;
            }
            let create: extern "C" fn(Id, Id, Id, *mut Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
            self.player = retain(create(self.engine, sel(c"createPlayerWithPattern:error:"), pattern, std::ptr::null_mut()));
            release(pattern);
            !self.player.is_null() && at(self.player, sel(c"startAtTime:error:"), 0.0, std::ptr::null_mut())
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        release(self.player);
        release(self.engine);
    }
}

/// The haptics thread's engines: the controllers' by id, and the phone's, which the phone may not
/// have (asked once).
#[derive(Default)]
struct Engines {
    controllers: Vec<(usize, Engine)>,
    phone: Option<Engine>,
    phone_cannot: bool,
}

impl Engines {
    /// One ask, on the engine kept for it; an engine that will not start or play is made again and
    /// tried once more, the next ask's restart for one the system stopped.
    fn play(&mut self, ask: Ask) {
        match ask.to {
            Haptics::Controller(id) => {
                // The controller object itself while it is connected; the engines of those that
                // are not are let go.
                let connected = objects(get(class(c"GCController"), c"controllers"));
                self.controllers.retain(|(c, _)| connected.iter().any(|o| *o as usize == *c));
                let Some(ctrl) = connected.into_iter().find(|o| *o as usize == id) else { return };
                for again in [false, true] {
                    if again {
                        self.controllers.retain(|(c, _)| *c != id);
                    }
                    if !self.controllers.iter().any(|(c, _)| *c == id) {
                        // GCDeviceHaptics (iOS 14): nil for a controller with no motors.
                        let haptics = get(ctrl, c"haptics");
                        if haptics.is_null() {
                            return;
                        }
                        // SAFETY: objc_msgSend cast to `id (id, SEL, GCHapticsLocality)`, the locality
                        // GameController's NSString constant; the engine comes back autoreleased
                        let engine = unsafe {
                            let f: extern "C" fn(Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
                            f(haptics, sel(c"createEngineWithLocality:"), GCHapticsLocalityDefault)
                        };
                        if engine.is_null() {
                            return;
                        }
                        self.controllers.push((id, Engine { engine: retain(engine), player: std::ptr::null_mut() }));
                    }
                    let engine = &mut self.controllers.iter_mut().find(|(c, _)| *c == id).expect("kept").1;
                    if engine.play(ask.intensity, ask.seconds) {
                        return;
                    }
                }
            }
            Haptics::Phone => {
                for again in [false, true] {
                    if again {
                        self.phone = None;
                    }
                    if self.phone.is_none() && !self.phone_cannot {
                        self.phone = phone_engine();
                        self.phone_cannot = self.phone.is_none();
                    }
                    let Some(engine) = &mut self.phone else { break };
                    if engine.play(ask.intensity, ask.seconds) {
                        return;
                    }
                }
                // SAFETY: a plain C call, with AudioToolbox's vibrate id
                unsafe { AudioServicesPlaySystemSound(SYSTEM_SOUND_VIBRATE) };
            }
        }
    }
}

/// The phone's own CHHapticEngine, if its hardware has haptics (`capabilitiesForHardware`), playing
/// haptics only so it leaves the app's sound (its AudioQueue) be.
fn phone_engine() -> Option<Engine> {
    let caps = get(class(c"CHHapticEngine"), c"capabilitiesForHardware");
    if caps.is_null() {
        return None;
    }
    // SAFETY: objc_msgSend cast to `BOOL (id, SEL)`, then `id (id, SEL, NSError **)` for the
    // alloc/init (an engine already ours), then `void (id, SEL, BOOL)`
    unsafe {
        let supports: extern "C" fn(Id, Id) -> bool = std::mem::transmute(objc_msgSend as *const ());
        if !supports(caps, sel(c"supportsHaptics")) {
            return None;
        }
        let init: extern "C" fn(Id, Id, *mut Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let engine = init(get(class(c"CHHapticEngine"), c"alloc"), sel(c"initAndReturnError:"), std::ptr::null_mut());
        if engine.is_null() {
            return None;
        }
        let set: extern "C" fn(Id, Id, bool) = std::mem::transmute(objc_msgSend as *const ());
        set(engine, sel(c"setPlaysHapticsOnly:"), true);
        Some(Engine { engine, player: std::ptr::null_mut() })
    }
}

// ---- sound ---------------------------------------------------------------------------------------

#[repr(C)]
struct StreamFormat {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
struct QueueBuffer {
    capacity: u32,
    data: *mut c_void,
    size: u32,
    user: *mut c_void,
    packet_capacity: u32,
    packets: *mut c_void,
    packet_count: u32,
}

type Queue = *mut c_void;
type Callback = extern "C" fn(*mut c_void, Queue, *mut QueueBuffer);

unsafe extern "C" {
    fn AudioQueueNewOutput(format: *const StreamFormat, callback: Callback, user: *mut c_void, run_loop: *mut c_void, mode: *const c_void, flags: u32, out: *mut Queue) -> i32;
    fn AudioQueueAllocateBuffer(queue: Queue, bytes: u32, out: *mut *mut QueueBuffer) -> i32;
    fn AudioQueueEnqueueBuffer(queue: Queue, buffer: *mut QueueBuffer, packets: u32, descriptions: *const c_void) -> i32;
    fn AudioQueueStart(queue: Queue, at: *const c_void) -> i32;
    fn AudioQueueDispose(queue: Queue, immediate: u8) -> i32;
}

/// What fills a buffer: interleaved stereo 16-bit samples, as many as the slice holds.
type Fill = Box<dyn FnMut(&mut [i16]) + Send>;

/// A buffer's frames: 1024 at 48 kHz is 21 ms, three of them queued.
const FRAMES: u32 = 1024;
const BUFFERS: usize = 3;

/// The sound going out: an AudioQueue on CoreAudio's own thread, asking `fill` for each buffer as the
/// last one plays. Dropping it stops the queue at once.
pub struct Sound {
    queue: Queue,
    fill: *mut Fill,
}

// SAFETY: the queue is CoreAudio's, callable from any thread; `fill` is only touched by the queue's
// own thread until the drop has disposed of the queue
unsafe impl Send for Sound {}

extern "C" fn refill(user: *mut c_void, queue: Queue, buffer: *mut QueueBuffer) {
    // SAFETY: `user` is the `Fill` `Sound::start` boxed, alive until the queue is disposed; `buffer`
    // is one of the queue's, `capacity` bytes of i16 samples
    unsafe {
        let fill = &mut *(user as *mut Fill);
        let b = &mut *buffer;
        let samples = std::slice::from_raw_parts_mut(b.data as *mut i16, b.capacity as usize / 2);
        fill(samples);
        b.size = b.capacity;
        AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null());
    }
}

impl Sound {
    /// Start playing `rate` Hz stereo out of `fill`.
    pub fn start(rate: u32, fill: Fill) -> Result<Sound, String> {
        let format = StreamFormat {
            sample_rate: rate as f64,
            format_id: u32::from_be_bytes(*b"lpcm"),
            // signed integer, packed
            format_flags: 4 | 8,
            bytes_per_packet: 4,
            frames_per_packet: 1,
            bytes_per_frame: 4,
            channels_per_frame: 2,
            bits_per_channel: 16,
            reserved: 0,
        };
        let fill = Box::into_raw(Box::new(fill));
        let mut queue: Queue = std::ptr::null_mut();
        // SAFETY: the format and out-pointer are ours; a null run loop is CoreAudio's own thread
        let status = unsafe { AudioQueueNewOutput(&format, refill, fill.cast(), std::ptr::null_mut(), std::ptr::null(), 0, &mut queue) };
        if status != 0 {
            // SAFETY: the box was never handed to a queue
            drop(unsafe { Box::from_raw(fill) });
            return Err(format!("no AudioQueue (OSStatus {status})"));
        }
        let sound = Sound { queue, fill };
        for _ in 0..BUFFERS {
            let mut buffer = std::ptr::null_mut();
            // SAFETY: the queue is live; each buffer is primed (filled and queued) before the start
            unsafe {
                if AudioQueueAllocateBuffer(queue, FRAMES * 4, &mut buffer) != 0 {
                    return Err("the AudioQueue gave no buffer".into());
                }
                refill(fill.cast(), queue, buffer);
            }
        }
        // SAFETY: the queue is live and primed
        let status = unsafe { AudioQueueStart(queue, std::ptr::null()) };
        if status != 0 {
            return Err(format!("the AudioQueue did not start (OSStatus {status})"));
        }
        Ok(sound)
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        // SAFETY: an immediate dispose stops the callbacks before it returns, so `fill` is ours again
        unsafe {
            AudioQueueDispose(self.queue, 1);
            drop(Box::from_raw(self.fill));
        }
    }
}

unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, ctx: *mut c_void, work: extern "C" fn(*mut c_void));
}

/// Opens `url` in Safari, or the app its scheme names (`UIApplication.openURL`, on the main queue: an
/// iPhone has no `xdg-open`), as opentennis's `open_url` (B501). Any thread.
pub fn open_url(url: &str) -> Result<(), String> {
    let url = std::ffi::CString::new(url).map_err(|e| e.to_string())?;
    let ctx = Box::into_raw(Box::new(url)).cast();
    // SAFETY: the main queue lives for the process; `open_on_main` takes the box back
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, ctx, open_on_main) };
    Ok(())
}

extern "C" fn open_on_main(ctx: *mut c_void) {
    // SAFETY: `ctx` is the `Box<CString>` `open_url` leaked; objc_msgSend cast to the signatures
    // NSString, NSURL and UIApplication declare
    let url = unsafe { Box::from_raw(ctx.cast::<std::ffi::CString>()) };
    pool(|| unsafe {
        let with: extern "C" fn(Id, Id, *const std::ffi::c_char) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let with_id: extern "C" fn(Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let open: extern "C" fn(Id, Id, Id, Id, Id) = std::mem::transmute(objc_msgSend as *const ());
        let s = with(class(c"NSString"), sel(c"stringWithUTF8String:"), url.as_ptr());
        let u = with_id(class(c"NSURL"), sel(c"URLWithString:"), s);
        if u.is_null() {
            return eprintln!("opening {url:?}: not a URL");
        }
        let app = get(class(c"UIApplication"), c"sharedApplication");
        let options = get(class(c"NSDictionary"), c"dictionary");
        open(app, sel(c"openURL:options:completionHandler:"), u, options, std::ptr::null_mut());
    });
}
