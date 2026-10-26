"""V79: a contact sheet of context/v79/throw.mp4's frames `first..last` every `every`, small, to a png.

usage: python3 -B sheet.py FIRST LAST EVERY OUT.png [COLS] [crop w:h:x:y]"""
import os
import subprocess
import sys

path = os.path.join('context', 'v79', 'throw' + '.mp4')
first, last, every, out = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
cols = int(sys.argv[5]) if len(sys.argv) > 5 else 6
crop = f'crop={sys.argv[6]},' if len(sys.argv) > 6 else ''
n = (last - first) // every + 1
rows = (n + cols - 1) // cols
vf = (f"select='between(n\\,{first}\\,{last})*not(mod(n-{first}\\,{every}))',{crop}"
      f"scale=213:-2,drawtext=text='%{{n}}':x=4:y=4:fontcolor=yellow:fontsize=14,tile={cols}x{rows}")
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', path, '-vf', vf, '-frames:v', '1', out], check=True)
print(out)
