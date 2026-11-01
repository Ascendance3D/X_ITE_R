#!/usr/bin/env python3
"""Clean synthetic media of the web MovieTexture 01 cases (issue #139): four solid 16 x 16 frames of one second each (red,
green, blue, yellow) as a GIF and an APNG, and the same colors as 64 x 64 WebM (VP8) and MP4 (H.264) video by ffmpeg.

  python3 -I oracle/tools/gen_movie_web.py crates/x3d-web/tests/browser/worlds/movie

The video is made like this (the files are committed; ffmpeg is not needed to run the tests):

  ffmpeg -f lavfi -i color=red:s=64x64:r=10:d=1 -f lavfi -i color=lime:s=64x64:r=10:d=1 \\
         -f lavfi -i color=blue:s=64x64:r=10:d=1 -f lavfi -i color=yellow:s=64x64:r=10:d=1 \\
         -filter_complex "[0][1][2][3]concat=n=4:v=1:a=0,format=yuv420p" -c:v libvpx -b:v 1M -g 5 steps4.webm
  (the same with -c:v libx264 -g 5 -movflags +faststart for steps4.mp4)

broken.webm is a text file: a video element fires `error` for it.
"""
import sys
from PIL import Image

out = sys.argv[1]
cols = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0)]
fr = [Image.new("RGB", (16, 16), c) for c in cols]
fr[0].save(out + "/steps4.gif", save_all=True, append_images=fr[1:], duration=1000, loop=0, disposal=1, optimize=False)
fr[0].save(out + "/steps4.png", save_all=True, append_images=fr[1:], duration=1000, loop=0, format="PNG")
