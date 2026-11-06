"""The pixels of a PNG written by the oracle recorders, with the standard library only (no PIL on every recording host, e.g. the
Apple M1 of issue #461). 8-bit RGB or RGBA, no interlace; anything else stops with an AssertionError.

    pixels(path) -> (width, height, rows), each row `width * 4` bytes of RGBA (alpha 255 for an RGB file)
"""
import struct
import zlib
def pixels(path):
    """The (width, height, RGBA rows) of a PNG of the recorders (8-bit RGB or RGBA, no interlace)."""
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    i, idat = 8, b""
    while i < len(data):
        n, kind = struct.unpack(">I", data[i:i + 4])[0], data[i + 4:i + 8]
        body = data[i + 8:i + 8 + n]
        if kind == b"IHDR":
            w, h, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert depth == 8 and color in (2, 6) and interlace == 0, path
        elif kind == b"IDAT":
            idat += body
        i += 12 + n
    raw, bpp = zlib.decompress(idat), 3 if color == 2 else 4
    stride, prev, rows, o = w * bpp, bytearray(w * bpp), [], 0
    for _ in range(h):
        f, line = raw[o], bytearray(raw[o + 1:o + 1 + stride])
        o += 1 + stride
        for x in range(stride):
            a, b, c = line[x - bpp] if x >= bpp else 0, prev[x], prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line) if bpp == 4 else b"".join(bytes(line[k:k + 3]) + b"\xff" for k in range(0, stride, 3)))
        prev = line
    return w, h, rows
