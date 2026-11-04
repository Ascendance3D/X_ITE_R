#!/usr/bin/env python3
"""World and event script for MovieTexture video frames at fixed movie times (issue #90).

  python3 -I oracle/tools/gen_movie_video.py

Writes oracle/synthetic-movie-video/movie-video.wrl and movie-video.json (WebM VP8 and VP9) and movie-video-av1.wrl and
movie-video-av1.json (AV1, a separate world so that the first recording stays as it is). The media are the synthetic WebM streams of
oracle/synthetic-movie-codecs/media (oracle/tools/gen_movie_codecs.py: ffmpeg test patterns, no third-party media):
vp8-ramp (uniform gray pictures, the frame is readable from the gray level), vp8-testsrc (inter frames, hidden alt-ref
frames, key frames every 6), vp8-parts (four token partitions, split motion vectors) and vp9-vorbis. The streams run at
10 pictures per second, so a picture lasts 0.1 s and the seek times below are the middle of a picture (no
rounding question for the decoder or for Chrome).

Every MovieTexture plays at `speed 0`: X_ITE sets the video element's playbackRate to 0, so the element holds its
picture and the picture shown is the one the harness seeks to. A key press at frame START starts every node (as in
gen_movie_anim.py, X_ITE starts a node from the file only through `loop` at initialize, before the media are in). The
events `movieSeek` (oracle/x_ite-anim.mjs sets the element's currentTime and waits for `seeked`; x3d-drive calls
Runtime::movie_seek) put every node at the next time, then three frames later the picture is captured. The times
include a backwards seek (0.05 after 0.55) and a seek back into a group of pictures (0.35 after 0.05).

The AV1 world (issue #90, after PR #363) plays the same seeks on the AV1 streams of the same directory: av1-testsrc (96x64,
1.5 s), av1-parts and av1-svt (128x80, 1.0 s; tiles and SVT-AV1 coding tools), av1-grain (64x48, film grain synthesis),
av1-10bit (64x48, 10-bit samples) and av1-444 (64x48, 4:4:4 chroma), all at 10 pictures per second. Three columns of two
quads. The pinned Chrome plays AV1 itself; X_ITE_R decodes it with the `av1` feature (rav1d).
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-movie-video"))
os.makedirs(root, exist_ok=True)

TIMES = [0.25, 0.55, 0.05, 0.35]
T0, DT, START = 946684800, 1 / 32, 32
SEEK_AT = [START + 4 + 8 * i for i in range(len(TIMES))]
CAPTURE = [START - 4] + [f + 3 for f in SEEK_AT]
FRAMES = CAPTURE[-1] + 2
QUAD = ("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } "
        "coordIndex [ 0 1 2 3 -1 ] texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] } }")


def write(name, media, ext, position):
    text = "#X3D V4.0 utf8\nPROFILE Full\nViewpoint { position 0 0 10 }\nNavigationInfo { type \"NONE\" headlight FALSE }\n"
    text += "Background { skyColor 0.12 0.14 0.2 }\n"

    for i, medium in enumerate(media):
        x, y = position(i)
        text += (f"Transform {{ translation {x:.1f} {y:.1f} 0 children Shape {{ appearance Appearance {{ "
                 f"texture DEF M{i} MovieTexture {{ url \"../synthetic-movie-codecs/media/{medium}.{ext}\" speed 0 }} }} "
                 f"geometry {QUAD} }} }}\n")

    text += "DEF K KeySensor { }\n"
    text += ("DEF S Script {\n  directOutput TRUE\n  inputOnly SFString press\n"
             f"  initializeOnly MFNode movies [ {' '.join(f'USE M{i}' for i in range(len(media)))} ]\n"
             "  url \"javascript:\nfunction press (value, time)\n{\n"
             "   for (var i = 0; i < movies .length; ++ i)\n   {\n      movies [i] .loop = true;\n      movies [i] .startTime = time;\n   }\n}\n\"\n}\n"
             "ROUTE K.keyPress TO S.press\n")
    open(os.path.join(root, f"{name}.wrl"), "w").write(text)

    events = [{"frame": START, "type": "key", "key": "s"}]
    for f, t in zip(SEEK_AT, TIMES):
        events += [{"frame": f, "type": "movieSeek", "node": f"M{i}", "time": t} for i in range(len(media))]

    script = {"settle": True, "size": [512, 512], "t0": T0, "dt": DT, "frames": FRAMES, "capture": CAPTURE,
              "dump": [FRAMES], "events": events}
    json.dump(script, open(os.path.join(root, f"{name}.json"), "w"), indent=1)


write("movie-video", ["vp8-ramp", "vp8-testsrc", "vp8-parts", "vp9-vorbis"], "webm", lambda i: (-1.1 + 2.2 * (i % 2), 1.1 - 2.2 * (i // 2)))
write("movie-video-av1", ["av1-testsrc", "av1-parts", "av1-svt", "av1-grain", "av1-10bit", "av1-444"], "webm", lambda i: (-2.2 + 2.2 * (i % 3), 1.1 - 2.2 * (i // 3)))
