import csv, math
import sys
CSV = sys.argv[1] if len(sys.argv) > 1 else "context/capture/p34/recording2-camera.csv"
rows = list(csv.DictReader(open(CSV)))
g = lambda r, k: float(r[k]) if r[k] not in ("", None) else float("nan")
print("rows", len(rows), "frame_counter", rows[0]["frame_counter"], "..", rows[-1]["frame_counter"],
      "ctl_src", {r["rider_ctl_src"] for r in rows}, "type", {r["rider_type"] for r in rows})
# the flag against the on-ground word, every row
agree = sum(1 for r in rows if int(r["tgt_flag"]) == (0 if int(r["on_ground"]) else 1))
print("tgt_flag == !on_ground on", agree, "of", len(rows))
print("fc  gnd flag flip  fwd.y   up.y  speed  behind  look_vs_vel  look_vs_fwd")
worst = {}
for r in rows:
    fc = int(r["frame_counter"])
    px, pz = g(r, "tgt_point.x"), g(r, "tgt_point.z")
    vx, vz = g(r, "tgt_vel.x"), g(r, "tgt_vel.z")
    fx, fz = g(r, "tgt_fwd.x"), g(r, "tgt_fwd.z")
    ex, ez = g(r, "render_cam.m30"), g(r, "render_cam.m32")
    h = math.hypot(vx, vz) or 1e-6
    behind = -((ex - px) * vx + (ez - pz) * vz) / h
    look = math.atan2(px - ex, pz - ez)
    d = lambda a: (math.degrees(a) + 180) % 360 - 180
    vs_vel = d(look - math.atan2(vx, vz))
    vs_fwd = d(look - math.atan2(fx, fz))
    for lo, hi in ((2500, 2610), (3284, 3430)):
        if lo <= fc <= hi:
            w = worst.setdefault((lo, hi), [1e9, 0, 0])
            if int(r["on_ground"]) == 0:
                w[0] = min(w[0], behind); w[1] = max(w[1], abs(vs_vel)); w[2] = max(w[2], abs(vs_fwd))
            if fc % 6 == 0:
                print(fc, r["on_ground"], r["tgt_flag"], r["flip_on"], "%6.3f %6.3f %6.1f %7.2f %8.1f %8.1f" % (
                    g(r, "tgt_fwd.y"), g(r, "tgt_up.y"), g(r, "tgt_speed"), behind, vs_vel, vs_fwd))
for k, w in worst.items():
    print("flip", k, "in the air: nearest behind %.2f ft, view off the velocity %.1f deg, off the forward row %.1f deg" % tuple(w))
