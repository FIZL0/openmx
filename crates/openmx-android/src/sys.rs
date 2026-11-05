//! The JNI and libc calls (P308). The activity is `MxActivity` (`crates/openmx/android/`), a
//! NativeActivity with a few methods of its own; android-activity hands its VM and its global
//! reference to [`start`].

use jni::objects::{JIntArray, JObject, JString, JValue, JValueOwned};
use jni::{Env, JavaVM, jni_sig, jni_str};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
pub use winit::platform::android::activity::AndroidApp;

static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Keeps the app `android_main` was given, for every call after. First thing in `android_main`.
pub fn start(app: &AndroidApp) {
    let _ = APP.set(app.clone());
}

/// opentennis's B492 for a player's build (`ship`): every variable starting with `prefix` but those
/// in `keep` dropped, before the shell reads any. Returns the names dropped. First thing in
/// `android_main`: the process's other threads are the activity's Java ones, which read none.
pub fn drop_env(prefix: &str, keep: &[&str]) -> Vec<String> {
    let names: Vec<String> = std::env::vars_os()
        .filter_map(|(k, _)| k.into_string().ok())
        .filter(|k| k.starts_with(prefix) && !keep.contains(&k.as_str()))
        .collect();
    for k in &names {
        // SAFETY: no thread of this process's Rust side runs yet, and the Java ones read no
        // environment.
        unsafe { std::env::remove_var(k) };
    }
    names
}

/// The app `android_main` was given (the event loop is built on it).
pub fn app() -> Option<AndroidApp> {
    APP.get().cloned()
}

/// The app's own folder on the shared storage (`/sdcard/Android/data/<package>/files`), which a
/// computer sees over USB and `adb push` reaches; the internal one where there is none.
pub fn files() -> PathBuf {
    let app = APP.get();
    app.and_then(AndroidApp::external_data_path)
        .or_else(|| app.and_then(AndroidApp::internal_data_path))
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/openmx"))
}

/// `f` with the activity on this thread's JNI env (attached for good: the shell's threads come and
/// go rarely). A Java exception is described to logcat, cleared, and gives `None`.
fn with_env<T>(f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    let app = APP.get()?;
    // SAFETY: android-activity's VM pointer, valid for the life of the process
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let out = vm.attach_current_thread(|env| -> jni::errors::Result<T> {
        // SAFETY: the NativeActivity's global reference, alive as long as the activity; `JObject`
        // frees nothing it wraps
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        let r = f(env, &activity);
        if env.exception_check() {
            env.exception_describe();
            env.exception_clear();
        }
        r
    });
    out.map_err(|e| eprintln!("android: a JNI call failed: {e}")).ok()
}

/// An `MxActivity` method that takes nothing.
fn call(name: &'static jni::strings::JNIStr, sig: jni::signature::MethodSignature<'static, 'static>) -> Option<JValueOwned<'static>> {
    with_env(|env, activity| {
        let v = env.call_method(activity, name, &sig, &[])?;
        // only primitives come back through here
        Ok(match v {
            JValueOwned::Int(i) => JValueOwned::Int(i),
            JValueOwned::Bool(b) => JValueOwned::Bool(b),
            _ => JValueOwned::Void,
        })
    })
}

// ---- the log ----------------------------------------------------------------------------------------

/// This launch's log file.
static LOG: OnceLock<std::fs::File> = OnceLock::new();

/// stdout and stderr into `log` as well as logcat (android-activity's forwarder, which they went to
/// before), fresh each launch: a phone's freeze or crash can be read without adb. Call before the
/// app's threads.
pub fn log_to(log: &Path) {
    let Ok(file) = std::fs::File::create(log) else { return };
    let file = LOG.get_or_init(|| file);
    let mut fds = [0; 2];
    // SAFETY: plain pipe/dup/dup2 of fds this process owns, before any thread of ours writes to them
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return;
        }
        let logcat = libc::dup(2);
        libc::dup2(fds[1], 1);
        libc::dup2(fds[1], 2);
        libc::close(fds[1]);
        let mut pipe = std::fs::File::from_raw_fd(fds[0]);
        let mut logcat = (logcat >= 0).then(|| std::fs::File::from_raw_fd(logcat));
        let _ = std::thread::Builder::new().name("openmx-log".into()).spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = (&*file).write_all(&buf[..n]);
                if let Some(l) = &mut logcat {
                    let _ = l.write_all(&buf[..n]);
                }
            }
        });
    }
    // SAFETY: once, at start-up before the app's threads, so nothing reads OLD while it is written;
    // sigaction is plain data (all-zero is valid); on_signal makes only async-signal-safe calls
    unsafe {
        for sig in [libc::SIGABRT, libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGTRAP, libc::SIGFPE] {
            let mut new: libc::sigaction = std::mem::zeroed();
            new.sa_sigaction = on_signal as *const () as libc::sighandler_t;
            new.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
            libc::sigaction(sig, &new, &raw mut OLD[sig as usize]);
        }
    }
}

/// The handlers before ours (debuggerd's, which writes the tombstone), by signal number.
// SAFETY: sigaction is plain data; all-zero is SIG_DFL with no flags
static mut OLD: [libc::sigaction; 32] = unsafe { std::mem::zeroed() };

extern "C" fn on_signal(sig: libc::c_int, info: *mut libc::siginfo_t, ctx: *mut libc::c_void) {
    let msg: &[u8] = match sig {
        libc::SIGABRT => b"fatal signal: SIGABRT (abort)\n",
        libc::SIGSEGV => b"fatal signal: SIGSEGV (bad memory access)\n",
        libc::SIGBUS => b"fatal signal: SIGBUS (bad memory access)\n",
        libc::SIGILL => b"fatal signal: SIGILL\n",
        libc::SIGTRAP => b"fatal signal: SIGTRAP\n",
        _ => b"fatal signal: SIGFPE\n",
    };
    // SAFETY: write is async-signal-safe; OLD was filled before this handler went in and is never
    // written again; the old handler is called with the signature its SA_SIGINFO flag says it has
    unsafe {
        if let Some(f) = LOG.get() {
            libc::write(f.as_raw_fd(), msg.as_ptr().cast(), msg.len());
        }
        let old = OLD[sig as usize];
        if old.sa_flags & libc::SA_SIGINFO != 0 && old.sa_sigaction > libc::SIG_IGN {
            let h: extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void) = std::mem::transmute(old.sa_sigaction);
            h(sig, info, ctx);
        } else if old.sa_sigaction > libc::SIG_IGN {
            let h: extern "C" fn(libc::c_int) = std::mem::transmute(old.sa_sigaction);
            h(sig);
        }
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

// ---- HTTPS (B418) -----------------------------------------------------------------------------------

/// Whether [`init_tls`] set the phone's certificate check up.
static TLS: AtomicBool = AtomicBool::new(false);

/// Hands the activity to `rustls-platform-verifier` (its JNI init: the VM, the activity as the
/// context, and the activity's class loader, which finds the verifier's Java classes `tools/apk.sh`
/// bundles). reqwest's rustls checks every certificate through it: Android's trust store,
/// user-installed CAs and revocation included. Before any HTTPS.
pub fn init_tls() -> bool {
    let ok = with_env(|env, activity| {
        // a reference of its own for the verifier, whose init keeps a global one
        let context = env.new_local_ref(activity)?;
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .is_some();
    TLS.store(ok, Ordering::Relaxed);
    ok
}

/// Whether the phone checks HTTPS certificates ([`init_tls`] worked): without it a request would
/// panic in the verifier, so the shell's HTTPS stays off.
pub fn tls() -> bool {
    TLS.load(Ordering::Relaxed)
}

// ---- the disc (M99) ---------------------------------------------------------------------------------

/// Opens the system's document picker (`MxActivity.pick`: any file, a disc image has no MIME type).
pub fn pick() -> bool {
    call(jni_str!("pick"), jni_sig!("()V")).is_some()
}

/// What the picker answered since the last call.
pub enum Picked {
    /// Nothing yet.
    Waiting,
    /// The player backed out.
    Cancelled,
    /// The file, opened for reading.
    File(std::fs::File),
}

/// The picker's answer (`MxActivity.taken`).
pub fn taken() -> Picked {
    match call(jni_str!("taken"), jni_sig!("()I")).and_then(|v| v.i().ok()) {
        None | Some(-2) => Picked::Waiting,
        Some(fd) if fd < 0 => Picked::Cancelled,
        // SAFETY: `ParcelFileDescriptor.detachFd` handed this descriptor over to us
        Some(fd) => Picked::File(unsafe { std::fs::File::from_raw_fd(fd) }),
    }
}

// ---- the window (M102) ------------------------------------------------------------------------------

fn decor<'a>(env: &mut Env<'a>, activity: &JObject) -> jni::errors::Result<JObject<'a>> {
    let window = env.call_method(activity, jni_str!("getWindow"), jni_sig!("()Landroid/view/Window;"), &[])?.l()?;
    env.call_method(&window, jni_str!("getDecorView"), jni_sig!("()Landroid/view/View;"), &[])?.l()
}

/// The window's insets, left, top, right, bottom, in pixels: the camera cutout and any system bar
/// shown (the activity hides them, so mostly the cutout).
pub fn safe_area() -> Option<[u32; 4]> {
    with_env(|env, activity| {
        let decor = decor(env, activity)?;
        let insets = env.call_method(&decor, jni_str!("getRootWindowInsets"), jni_sig!("()Landroid/view/WindowInsets;"), &[])?.l()?;
        if insets.is_null() {
            return Ok([0; 4]);
        }
        let sdk = env.get_static_field(jni_str!("android/os/Build$VERSION"), jni_str!("SDK_INT"), jni_sig!("I"))?.i()?;
        let mut v = [0i32; 4];
        if sdk >= 30 {
            let ty = jni_str!("android/view/WindowInsets$Type");
            let bars = env.call_static_method(ty, jni_str!("systemBars"), jni_sig!("()I"), &[])?.i()?;
            let cutout = env.call_static_method(ty, jni_str!("displayCutout"), jni_sig!("()I"), &[])?.i()?;
            let i = env
                .call_method(&insets, jni_str!("getInsets"), jni_sig!("(I)Landroid/graphics/Insets;"), &[JValue::Int(bars | cutout)])?
                .l()?;
            for (v, side) in v.iter_mut().zip([jni_str!("left"), jni_str!("top"), jni_str!("right"), jni_str!("bottom")]) {
                *v = env.get_field(&i, side, jni_sig!("I"))?.i()?;
            }
        } else {
            let sides = [
                jni_str!("getSystemWindowInsetLeft"),
                jni_str!("getSystemWindowInsetTop"),
                jni_str!("getSystemWindowInsetRight"),
                jni_str!("getSystemWindowInsetBottom"),
            ];
            for (v, m) in v.iter_mut().zip(sides) {
                *v = env.call_method(&insets, m, jni_sig!("()I"), &[])?.i()?;
            }
        }
        Ok(v.map(|v| v.max(0) as u32))
    })
}

/// `Build.MODEL`, for the log and the graphics preset's device line.
pub fn model() -> Option<String> {
    with_env(|env, _| {
        let v = env.get_static_field(jni_str!("android/os/Build"), jni_str!("MODEL"), jni_sig!("Ljava/lang/String;"))?.l()?;
        let s = env.cast_local::<JString>(v)?;
        s.try_to_string(env)
    })
}

/// Opens `url` in the phone's browser (an `ACTION_VIEW` intent from the activity).
pub fn open_url(url: &str) -> Result<(), String> {
    with_env(|env, activity| {
        let s = env.new_string(url)?;
        let uri = env
            .call_static_method(jni_str!("android/net/Uri"), jni_str!("parse"), jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"), &[JValue::Object(&s)])?
            .l()?;
        let action = env.new_string("android.intent.action.VIEW")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
            &[JValue::Object(&action), JValue::Object(&uri)],
        )?;
        env.call_method(activity, jni_str!("startActivity"), jni_sig!("(Landroid/content/Intent;)V"), &[JValue::Object(&intent)]).map(drop)
    })
    .ok_or_else(|| format!("no browser opened {url}"))
}

/// Whether the activity is finishing (closed, swiped away) rather than destroyed to be made again.
pub fn finishing() -> bool {
    call(jni_str!("isFinishing"), jni_sig!("()Z")).and_then(|v| v.z().ok()).unwrap_or(false)
}

// ---- pads (M101) ------------------------------------------------------------------------------------

/// An input device Android lists as a gamepad or joystick.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: i32,
    pub name: String,
}

const SOURCE_GAMEPAD: i32 = 0x0000_0401;
const SOURCE_JOYSTICK: i32 = 0x0100_0010;

/// The pads Android lists now (`InputDevice.getDeviceIds`, the virtual ones left out); `None` when
/// the list could not be read.
pub fn pads() -> Option<Vec<Device>> {
    with_env(|env, _| {
        let class = jni_str!("android/view/InputDevice");
        let ids = env.call_static_method(class, jni_str!("getDeviceIds"), jni_sig!("()[I"), &[])?.l()?;
        let ids = env.cast_local::<JIntArray>(ids)?;
        let mut buf = vec![0; ids.len(env)?];
        ids.get_region(env, 0, &mut buf)?;
        let mut out = Vec::new();
        for id in buf {
            let dev = env.call_static_method(class, jni_str!("getDevice"), jni_sig!("(I)Landroid/view/InputDevice;"), &[JValue::Int(id)])?.l()?;
            if dev.is_null() {
                continue;
            }
            let sources = env.call_method(&dev, jni_str!("getSources"), jni_sig!("()I"), &[])?.i()?;
            let virt = env.call_method(&dev, jni_str!("isVirtual"), jni_sig!("()Z"), &[])?.z()?;
            if virt || (sources & SOURCE_GAMEPAD != SOURCE_GAMEPAD && sources & SOURCE_JOYSTICK != SOURCE_JOYSTICK) {
                continue;
            }
            let name = env.call_method(&dev, jni_str!("getName"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
            let name = if name.is_null() { String::new() } else { env.cast_local::<JString>(name)?.try_to_string(env)? };
            out.push(Device { id, name });
        }
        Ok(out)
    })
}

// ---- the updater (M140) -----------------------------------------------------------------------------

/// Whether Google Play installed this app (`MxActivity.fromStore`): then only Play may update it.
pub fn from_store() -> bool {
    call(jni_str!("fromStore"), jni_sig!("()Z")).and_then(|v| v.z().ok()).unwrap_or(false)
}

/// Hands the checked update .apk at `apk` to the system's installer (`MxActivity.install`, a
/// `PackageInstaller` session; the player confirms, and allows installs from this app the first
/// time). [`install_status`] says how it went.
pub fn install_apk(apk: &Path) -> Result<(), String> {
    let path = apk.to_string_lossy().into_owned();
    with_env(|env, activity| {
        let s = env.new_string(&path)?;
        env.call_method(activity, jni_str!("install"), jni_sig!("(Ljava/lang/String;)V"), &[JValue::Object(&s)]).map(drop)
    })
    .ok_or_else(|| "the system's installer didn't take it".into())
}

/// The installer's answer once there is one: `PackageInstaller`'s status (0 installed, 3 the player
/// said no, others a failure). The app usually ends before 0 arrives: the system replaces it.
pub fn install_status() -> Option<i32> {
    call(jni_str!("installed"), jni_sig!("()I")).and_then(|v| v.i().ok()).filter(|&s| s != -99)
}
