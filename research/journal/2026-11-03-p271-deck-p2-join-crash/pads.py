#!/usr/bin/env python3
"""Stand-in pads for OPENMX_PAD_ROOT: js0 from the start, js1 hot-plugged later, then a script of
presses. Usage: pads.py ROOT SCRIPT -- openmx args...  SCRIPT is `;`-separated steps:
  wait S | plug N | unplug N | press N BTN | hold N BTN | free N BTN | axis N A V
"""
import os, struct, subprocess, sys, time

root, script = sys.argv[1], sys.argv[2]
cmd = sys.argv[sys.argv.index("--") + 1:]
dev, sysd = os.path.join(root, "dev"), os.path.join(root, "sys")
os.makedirs(dev, exist_ok=True)
pads = {}
t0 = time.time()


def sys_entry(node, name, phys):
    d = os.path.join(sysd, node, "device")
    os.makedirs(os.path.join(d, "id"), exist_ok=True)
    for f, v in [("name", name), ("id/vendor", "045e"), ("id/product", "028e"), ("uniq", ""), ("phys", phys)]:
        with open(os.path.join(d, f), "w") as fh:
            fh.write(v + "\n")


def send(n, kind, number, value):
    ms = int((time.time() - t0) * 1000) & 0xffffffff
    os.write(pads[n][1], struct.pack("<IhBB", ms, value, kind, number))


def plug(n):
    node = f"js{n}"
    sys_entry(node, f"Pad {n}", f"usb-{n + 1}/input0")
    path = os.path.join(dev, node)
    if not os.path.exists(path):
        os.mkfifo(path)
    hold = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
    w = os.open(path, os.O_WRONLY)
    pads[n] = (hold, w, path)
    for b in range(11):
        send(n, 0x81, b, 0)
    for a in range(8):
        send(n, 0x82, a, -32767 if a in (2, 5) else 0)
    print(f"[pads] plugged js{n}", flush=True)


def unplug(n):
    hold, w, path = pads.pop(n)
    os.remove(path)
    os.close(w)
    os.close(hold)
    print(f"[pads] unplugged js{n}", flush=True)


BTN = {"cross": 0, "circle": 1, "square": 2, "triangle": 3, "l1": 4, "r1": 5, "select": 6, "start": 7}
plug(0)
env = dict(os.environ, OPENMX_PAD_ROOT=root)
proc = subprocess.Popen(cmd, env=env)
for step in [s.strip() for s in script.split(";") if s.strip()]:
    w = step.split()
    if proc.poll() is not None:
        break
    if w[0] == "wait":
        try:
            proc.wait(timeout=float(w[1]))
        except subprocess.TimeoutExpired:
            pass
    elif w[0] == "plug":
        plug(int(w[1]))
    elif w[0] == "unplug":
        unplug(int(w[1]))
    elif w[0] in ("press", "hold", "free"):
        n, b = int(w[1]), BTN[w[2]]
        if w[0] in ("press", "hold"):
            send(n, 1, b, 1)
        if w[0] == "press":
            time.sleep(0.12)
        if w[0] in ("press", "free"):
            send(n, 1, b, 0)
        print(f"[pads] {step}", flush=True)
    elif w[0] == "end":
        proc.terminate()
        print("[pads] end", flush=True)
    elif w[0] == "axis":
        send(int(w[1]), 2, int(w[2]), int(w[3]))
code = proc.wait(timeout=600)
print(f"[pads] openmx exit {code}", flush=True)
sys.exit(code)
