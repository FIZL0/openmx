"""Everything the kit does that is about the *operating system*, in one place.

The rest of the kit is OS-neutral and goes through these functions. That is deliberate: the Windows
edition of the kit replaces only this module (plus `pad.py`'s backend and the shell wrappers), and
nothing else has to know.

What belongs here: where runtime files live, how a lock is taken, how a PCSX2 config dir is found, how a
window is screenshotted, how a process is checked, how PINE is reached (socket type and path).
"""
from __future__ import annotations

import contextlib
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

WINDOWS = sys.platform.startswith('win')
MAC = sys.platform == 'darwin'
LINUX = not WINDOWS and not MAC


# --- where transient files live ----------------------------------------------

def runtime_dir() -> Path:
    """Per-user directory for pids, locks, fifos: cleared on reboot, not in the repo."""
    d = os.environ.get('XDG_RUNTIME_DIR')
    if d and Path(d).is_dir():
        return Path(d)
    d = Path(os.environ.get('TMPDIR') or '/tmp') / f'ps2kit-{os.getuid() if hasattr(os, "getuid") else "u"}'
    d.mkdir(parents=True, exist_ok=True)
    return d


def rt(name: str) -> Path:
    return runtime_dir() / name


def instance() -> str:
    """Which PCSX2 copy this process talks to: "" = the user's own, "1".."N" = a kit-managed copy.

    Set per slot by `./kit run` (KIT_PCSX2=N) so several agents never touch each other's emulator.
    """
    return os.environ.get('KIT_PCSX2', '')


# --- locks -------------------------------------------------------------------

_HELD: set[str] = set()   # lock files this process holds; see Lock.held_by_other()


class Lock:
    """An advisory, process-wide lock file. Used to give one tool PCSX2 to itself.

    with Lock('pcsx2') as l:  ...        # blocks, printing one line, until it is free
    Lock('pcsx2').take(block=False)      # False if someone else holds it

    A kernel lock here (`flock`, or `LockFile` on Windows) belongs to the open file *description*, not to
    the process, so probing a lock this process already holds looks exactly like someone else holding it.
    The lock files this process holds are therefore tracked in `_HELD` and consulted first, so
    `held_by_other()` answers the question its name asks.
    """

    def __init__(self, name: str, note: str = ''):
        self.path = rt(f'{name}{instance()}.lock')
        self.note = note or f'another process is using {name}'
        self._f = None

    def take(self, block: bool = True) -> bool:
        self._f = open(self.path, 'w')
        if WINDOWS:  # pragma: no cover - the Windows edition overrides this module
            import msvcrt
            flags = msvcrt.LK_LOCK if block else msvcrt.LK_NBLCK
            try:
                msvcrt.locking(self._f.fileno(), flags, 1)
                _HELD.add(str(self.path))
                return True
            except OSError:
                return False
        import fcntl
        try:
            fcntl.flock(self._f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            _HELD.add(str(self.path))
            return True
        except BlockingIOError:
            if not block:
                return False
            print(f'waiting: {self.note}', file=sys.stderr, flush=True)
            fcntl.flock(self._f, fcntl.LOCK_EX)
            _HELD.add(str(self.path))
            return True

    def release(self) -> None:
        if self._f:
            self._f.close()
            self._f = None
        _HELD.discard(str(self.path))

    def held_by_other(self) -> bool:
        """True when a *different* process holds this lock. False for one this process holds (see the class
        docstring): `./kit pcsx2 status` must not report the emulator busy because of its own lock."""
        if str(self.path) in _HELD:
            return False
        probe = Lock(self.path.stem.rstrip('0123456789'))
        probe.path = self.path
        if probe.take(block=False):
            probe.release()
            return False
        return True

    def __enter__(self):
        self.take()
        return self

    def __exit__(self, *_):
        self.release()


# --- PINE transport ----------------------------------------------------------

def pine_endpoint(slot: int) -> tuple[int, str | tuple[str, int]]:
    """(socket family, address) for PCSX2's PINE server on `slot`.

    Linux/macOS: a unix socket in the runtime dir, named `pcsx2.sock` for the default slot and
    `pcsx2.sock.<slot>` otherwise. Windows: a TCP socket on localhost:<slot>.
    """
    if WINDOWS:  # pragma: no cover
        return socket.AF_INET, ('127.0.0.1', slot)
    base = runtime_dir() / ('pcsx2.sock' if slot == 28011 else f'pcsx2.sock.{slot}')
    return socket.AF_UNIX, str(base)


# --- PCSX2 install and config ------------------------------------------------

def pcsx2_config_dir(instance_n: str = '') -> Path:
    """Where PCSX2 keeps `inis/PCSX2.ini`, for the user's own install or for kit copy N."""
    if instance_n:
        return copies_root() / f's{instance_n}' / 'PCSX2'
    if WINDOWS:  # pragma: no cover
        return Path(os.environ['USERPROFILE']) / 'Documents' / 'PCSX2'
    if MAC:
        return Path.home() / 'Library' / 'Application Support' / 'PCSX2'
    return Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config')) / 'PCSX2'


def copies_root() -> Path:
    """Where kit-managed PCSX2 copies live: a sibling of the repo, never inside it."""
    from ps2kit.config import ROOT
    return ROOT.parent / f'{ROOT.name}-slots' / 'pcsx2'


def pcsx2_cmd(cmd: str) -> list[str]:
    """Turn the configured pcsx2 setting into something runnable (handles an AppImage or a path)."""
    p = Path(cmd).expanduser()
    if p.exists():
        return [str(p)]
    found = shutil.which(cmd)
    if found:
        return [found]
    for guess in ('pcsx2-qt', 'PCSX2', 'pcsx2'):
        if (found := shutil.which(guess)):
            return [found]
    raise SystemExit(f'PCSX2 not found (tried {cmd!r} and PATH). Set [emu] pcsx2 in kit.toml.')


# --- processes ---------------------------------------------------------------

def pids_named(name: str) -> list[int]:
    """Pids of running processes whose executable is exactly `name` — never a command-line match.

    A command-line match (`pgrep -f`) is the classic self-shooting foot: the pattern is in the
    searching shell's own command line too, so `pkill -f pat` kills the caller. The kit only ever
    matches on the program name, and tracks its own children by pid file.
    """
    if WINDOWS:  # pragma: no cover
        out = subprocess.run(['tasklist', '/FI', f'IMAGENAME eq {name}.exe', '/FO', 'CSV', '/NH'],
                             capture_output=True, text=True).stdout
        return [int(l.split(',')[1].strip('"')) for l in out.splitlines() if l.startswith('"')]
    out = subprocess.run(['pgrep', '-x', name], capture_output=True, text=True).stdout
    return [int(l) for l in out.split()]


def alive(pid: int | None) -> bool:
    if not pid:
        return False
    if WINDOWS:  # pragma: no cover
        return bool(subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/NH'],
                                   capture_output=True, text=True).stdout.strip().startswith('"'))
    try:
        os.kill(pid, 0)
        return True
    except (ProcessLookupError, PermissionError) as e:
        return isinstance(e, PermissionError)


def proc_name(pid: int) -> str:
    if WINDOWS:  # pragma: no cover
        out = subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/FO', 'CSV', '/NH'],
                             capture_output=True, text=True).stdout
        return out.split(',')[0].strip('"').removesuffix('.exe') if out.startswith('"') else ''
    return subprocess.run(['ps', '-o', 'comm=', '-p', str(pid)], capture_output=True, text=True).stdout.strip()


def terminate(pid: int) -> None:
    if WINDOWS:  # pragma: no cover
        subprocess.run(['taskkill', '/PID', str(pid)], capture_output=True)
        return
    with contextlib.suppress(ProcessLookupError):
        os.kill(pid, 15)


def spawn_detached(argv: list[str], log: Path | None = None, env: dict | None = None) -> int:
    """Start a program that outlives this process; returns its pid."""
    out = open(log, 'ab') if log else subprocess.DEVNULL
    kw: dict = {'stdout': out, 'stderr': out, 'stdin': subprocess.DEVNULL,
                'env': {**os.environ, **(env or {})}}
    if WINDOWS:  # pragma: no cover
        kw['creationflags'] = 0x00000008 | 0x00000200  # DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    else:
        kw['start_new_session'] = True
    return subprocess.Popen(argv, **kw).pid


# --- screen capture ----------------------------------------------------------

def screenshot_tools() -> list[str]:
    """Which screen-capture helpers this machine has, best first. `./kit doctor` reports them; empty is
    not fatal, because the emulator's own screenshot hotkey is always available as a fallback."""
    names = ('grim', 'hyprctl', 'xdotool', 'import', 'spectacle') if not WINDOWS else ('powershell',)
    return [t for t in names if shutil.which(t)]


# A capture tool that hangs is worse than one that fails: PCSX2's game window wedges wlr-screencopy on
# some compositors (seen on Hyprland 0.56.2 — `grim` returns at once with the emulator stopped, and never
# returns once a game is running), and an unbounded wait takes the hotkey fallback away from `shot()`.
GRAB_TIMEOUT = 15


def _grab(argv: list[str]) -> bool:
    try:
        return subprocess.run(argv, timeout=GRAB_TIMEOUT).returncode == 0
    except (subprocess.TimeoutExpired, OSError):
        return False


def screenshot_window(out: Path, pattern: str = 'pcsx2') -> bool:
    """Grab a window whose class/title contains `pattern`, without focusing it. False if it can't.

    Several desktops, tried in order; the emulator's own screenshot hotkey is the fallback that always
    works (see `ps2kit/emu.py`), including for a copy on a hidden workspace where no compositor can see it.
    """
    out.parent.mkdir(parents=True, exist_ok=True)
    if WINDOWS:  # pragma: no cover
        return False
    # Hyprland / wlroots
    if shutil.which('hyprctl') and shutil.which('grim'):
        import json
        try:
            clients = json.loads(subprocess.run(['hyprctl', 'clients', '-j'], capture_output=True,
                                                text=True, check=True, timeout=GRAB_TIMEOUT).stdout)
        except Exception:
            clients = []
        for c in clients:
            if pattern.lower() in f'{c.get("class", "")} {c.get("title", "")}'.lower():
                geom = f'{c["at"][0]},{c["at"][1]} {c["size"][0]}x{c["size"][1]}'
                return _grab(['grim', '-g', geom, str(out)])
    # GNOME / KDE Wayland portal, X11
    if shutil.which('xdotool') and shutil.which('import'):
        wid = subprocess.run(['xdotool', 'search', '--name', pattern], capture_output=True, text=True,
                             timeout=GRAB_TIMEOUT).stdout.split()
        if wid:
            return _grab(['import', '-window', wid[0], str(out)])
    if shutil.which('spectacle'):
        return _grab(['spectacle', '-abno', str(out)])
    return False


def wait_for_file(d: Path, before: set[str], tries: int = 40, delay: float = 0.3) -> Path | None:
    """Wait for a new, fully-written file to appear in `d` (emulator screenshots land asynchronously)."""
    for _ in range(tries):
        time.sleep(delay)
        new = {f.name for f in d.iterdir()} - before if d.is_dir() else set()
        if new:
            f = d / sorted(new)[-1]
            size = -1
            for _ in range(30):  # PCSX2 creates the file, then writes it
                time.sleep(delay)
                s = f.stat().st_size
                if s and s == size:
                    return f
                size = s
            return f
    return None


# --- virtual gamepad backend -------------------------------------------------

def pad_backend() -> str:
    """Which virtual-pad implementation `ps2kit/pad.py` should use on this OS."""
    if LINUX:
        return 'uinput'
    if WINDOWS:  # pragma: no cover
        return 'vigem'
    return 'none'


def pad_available() -> tuple[bool, str]:
    """(usable, why not) for the virtual pad, so `./kit doctor` can explain the fix."""
    if LINUX:
        if not Path('/dev/uinput').exists():
            return False, '/dev/uinput is missing (load the uinput module: sudo modprobe uinput)'
        if not os.access('/dev/uinput', os.W_OK):
            return False, ('/dev/uinput is not writable by you — add a udev rule or an ACL:\n'
                           '    sudo setfacl -m u:$USER:rw /dev/uinput   # until reboot\n'
                           '  or /etc/udev/rules.d/99-uinput.rules:\n'
                           '    KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"')
        try:
            import evdev  # noqa: F401
        except ImportError:
            return False, 'the python-evdev package is missing (pip install evdev)'
        return True, ''
    if WINDOWS:  # pragma: no cover
        try:
            import vgamepad  # noqa: F401
        except ImportError:
            return False, 'install the ViGEmBus driver and `pip install vgamepad`'
        return True, ''
    return False, f'no virtual-pad backend for {sys.platform}'


# --- memory, and windows kept off the desktop --------------------------------
# K16 (2026-10-30): the desktop's compositor (Hyprland 0.56.2) grew to 105 GiB of GPU memory over ten days of
# runs and the kernel OOM-killed the runner. A run must not lean on the compositor the user sits at.

def mem_available_gib() -> float | None:
    """MemAvailable in GiB, or None where there is no /proc/meminfo (Windows, macOS)."""
    try:
        for line in Path('/proc/meminfo').read_text().splitlines():
            if line.startswith('MemAvailable:'):
                return int(line.split()[1]) / 1048576
    except (OSError, ValueError, IndexError):
        pass
    return None


def gtt_mib(pid: int) -> float | None:
    """GPU memory in GTT (system RAM the GPU maps) that a process holds, in MiB, from its DRM fdinfo.

    One DRM client shows on several of a process's fds, so each `drm-client-id` counts once. `free` shows this
    memory as used with no process's RSS to account for it, which is how a compositor leak hides."""
    if not LINUX:
        return None
    seen: dict[str, int] = {}
    try:
        fds = list(Path(f'/proc/{pid}/fdinfo').iterdir())
    except OSError:
        return None
    for f in fds:
        try:
            text = f.read_text()
        except OSError:
            continue
        client = kib = None
        for line in text.splitlines():
            k, _, v = line.partition(':')
            if k == 'drm-client-id':
                client = v.strip()
            elif k == 'drm-memory-gtt':
                kib = int(v.split()[0])
        if client is not None and kib is not None:
            seen[client] = kib
    return sum(seen.values()) / 1024


DISPLAY_FIRST = 200  # private X displays from here up
COMPOSITORS = ('Hyprland', 'sway', 'kwin_wayland', 'gnome-shell', 'Xwayland')


def compositor_gtt_mib() -> dict[str, float]:
    """GTT MiB of each desktop compositor (and its Xwayland) running for this user, by program name."""
    out = {}
    for name in COMPOSITORS:
        for pid in pids_named(name) if LINUX else []:
            if pid != os.getpid() and (g := gtt_mib(pid)) is not None:
                out[name] = out.get(name, 0.0) + g
    return out


class PrivateDisplay:
    """A headless weston with a rootful Xwayland in it: test windows land here, not on the desktop.

        with PrivateDisplay() as env:    # {'DISPLAY': ':N', 'WAYLAND_DISPLAY': None}, or {} if it cannot start
            subprocess.run(cmd, env={**os.environ, **env})

    Every window then draws into GPU memory weston and this Xwayland own, both of which end with the block, so
    nothing a run opens can pile up in the compositor the user sits at. X11 only (WAYLAND_DISPLAY is dropped),
    as the shell's software window speaks X11 and its GPU window takes X11 when there is no Wayland. `why`
    says what went wrong when the env is empty; the caller falls back to whatever display it had.
    """

    def __init__(self, size: str = '1920x1080', name: str = ''):
        self.size = size
        self.name = name or f'kit-display-{os.getpid()}'
        self.why = ''
        self.procs: list[subprocess.Popen] = []
        self.log = rt(f'{self.name}.log')

    def __enter__(self) -> dict:
        try:
            return self._start()
        except Exception as e:  # never fail the caller's run for want of a private display
            self.why = f'{type(e).__name__}: {e}'
            self.__exit__()
            return {}

    def _start(self) -> dict:
        if not LINUX:
            self.why = 'not Linux'
            return {}
        weston = shutil.which('weston')
        if not weston or not shutil.which('Xwayland'):
            self.why = 'weston or Xwayland is not installed (pacman -S weston xorg-xwayland)'
            return {}
        w, h = self.size.split('x')
        bare = {k: v for k, v in os.environ.items() if k not in ('DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET')}
        logf = open(self.log, 'wb')
        # --idle-time=0: an idle headless output stops repainting, and then no window's frame ever comes back
        self.procs.append(subprocess.Popen(
            [weston, '--backend=headless', '--renderer=gl', '--shell=kiosk', f'--socket={self.name}',
             f'--width={w}', f'--height={h}', '--idle-time=0'],
            env=bare, stdin=subprocess.DEVNULL, stdout=logf, stderr=subprocess.STDOUT, start_new_session=True))
        sock = runtime_dir() / self.name
        deadline = time.monotonic() + 15
        while not sock.exists():
            if self.procs[0].poll() is not None or time.monotonic() > deadline:
                self.why = f'weston did not come up (log {self.log})'
                self.__exit__()
                return {}
            time.sleep(0.05)
        # Never -displayfd: it walks up from :0, and on a display the desktop already serves Xtrans unlinks the
        # desktop's socket file before it gives up on that number, so every later client of :0 is refused
        # (K16 took the desktop's /tmp/.X11-unix/X0 that way, 10-30). A number with neither a lock nor a
        # socket, well above the desktop's, and Xwayland's own lock file settles two gates picking the same one.
        for n in range(DISPLAY_FIRST, DISPLAY_FIRST + 200):
            lock, sock_x = Path(f'/tmp/.X{n}-lock'), Path(f'/tmp/.X11-unix/X{n}')
            try:  # a private display a killed run left behind: its server is gone, so the number is free
                if not alive(int(lock.read_text().strip())):
                    lock.unlink()
                    sock_x.unlink(missing_ok=True)
            except (OSError, ValueError):
                pass
            if lock.exists() or sock_x.exists():
                continue
            x = subprocess.Popen(
                ['Xwayland', f':{n}', '-geometry', self.size, '-noreset', '-nolisten', 'tcp'],
                env={**bare, 'WAYLAND_DISPLAY': self.name}, stdin=subprocess.DEVNULL, stdout=logf,
                stderr=subprocess.STDOUT, start_new_session=True)
            self.procs.append(x)
            while not Path(f'/tmp/.X11-unix/X{n}').exists() and x.poll() is None and time.monotonic() < deadline:
                time.sleep(0.05)
            if x.poll() is None and Path(f'/tmp/.X11-unix/X{n}').exists():
                break
            self.procs.remove(x)
            if x.poll() is None:
                x.kill()
                x.wait()
            if time.monotonic() > deadline:
                n = None
                break
        else:
            n = None
        if n is None:
            self.why = f'Xwayland did not come up (log {self.log})'
            self.__exit__()
            return {}
        return {'DISPLAY': f':{n}', 'WAYLAND_DISPLAY': None}

    def __exit__(self, *_) -> None:
        for p in reversed(self.procs):
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(10)
                except subprocess.TimeoutExpired:
                    p.kill()
                    p.wait()
        self.procs.clear()


def with_env(base: dict, extra: dict) -> dict:
    """`base` with `extra` laid over it, where a None in `extra` removes that variable."""
    out = {**base, **{k: v for k, v in extra.items() if v is not None}}
    for k, v in extra.items():
        if v is None:
            out.pop(k, None)
    return out


def main(argv: list[str]) -> int:
    """`python3 -m ps2kit.platform_ room GIB`: exit 1 while MemAvailable is under GIB (agent-runner's
    `[run] before_start` holds every new slot on it); exit 0 where it cannot be read."""
    if argv[:1] == ['room'] and len(argv) == 2:
        have, need = mem_available_gib(), float(argv[1])
        if have is not None and have < need:
            print(f'MemAvailable {have:.1f} GiB < {need:g} GiB: no new slot (compositor GTT '
                  f'{ {k: round(v) for k, v in compositor_gtt_mib().items()} } MiB)', file=sys.stderr)
            return 1
        return 0
    print('usage: python3 -m ps2kit.platform_ room GIB', file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
