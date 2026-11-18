import os, sys
def io():
    fd = os.open("/proc/self/io", os.O_RDONLY); s = b""
    while True:
        c = os.read(fd, 4096)
        if not c: break
        s += c
    os.close(fd); return s.decode()
def rchar(s): return int(s.split("rchar:")[1].split()[0])
want, which = int(sys.argv[1]), sys.argv[2]
z = os.open("/dev/zero", os.O_RDONLY)
n = want - rchar(io())
while n > 0: n -= len(os.read(z, min(n, 1 << 20)))
if which == "old":
    a = rchar(io()); b = rchar(io()); mark = b + (b - a)
else:
    s = io(); mark = rchar(s) + len(s)
print(rchar(io()) - mark)
