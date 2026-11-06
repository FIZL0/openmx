"""C28: PCSX2 System -> Screenshot over AT-SPI, copied to OUT. usage: shot.py PID OUT.png

PCSX2 writes it into the KIT_PCSX2 copy's snaps/ folder; `./kit shot` needs a uinput pad, which is
root-only here.
"""
import glob, os, shutil, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'journal',
                                '2026-10-20-c6-c6-recording2-capture', 'playback'))
import atspi_lib as A

pid, out = int(sys.argv[1]), sys.argv[2]
snaps = os.path.expanduser(f"~/repos/openmx-slots/pcsx2/s{os.environ.get('KIT_PCSX2', '2')}/PCSX2/snaps")
before = set(glob.glob(f'{snaps}/*.png'))
A.press(A.menu_path(A.app_of(pid), ['System', 'Screenshot']))
for _ in range(40):
    new = set(glob.glob(f'{snaps}/*.png')) - before
    if new:
        time.sleep(0.3)
        shutil.copy(new.pop(), out)
        print(out)
        sys.exit(0)
    time.sleep(0.25)
sys.exit('no screenshot appeared')
