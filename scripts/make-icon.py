#!/usr/bin/env python3
"""Render the Ondera app icon and write desktop/assets/Ondera.icns (macOS only: uses sips and
iconutil). Pure Python so it needs no image libraries. Colours are the theme's PANEL charcoal,
the raised-control face and the teal accent from desktop/src/theme.rs."""
import math, os, struct, subprocess, sys, tempfile, zlib

SIZE = 1024
PANEL = (0x2C, 0x2C, 0x2B)
FACE_TOP = (0x42, 0x42, 0x3F)
FACE_BOTTOM = (0x31, 0x31, 0x30)
ACCENT = (71, 214, 207)
ACCENT_LO = (0, 169, 162)
LED = (0xF0, 0xE8, 0xD8)


def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


def rounded_sdf(x, y, cx, cy, hw, hh, r):
    dx, dy = abs(x - cx) - hw + r, abs(y - cy) - hh + r
    return math.hypot(max(dx, 0), max(dy, 0)) + min(max(dx, dy), 0) - r


def coverage(d):
    return min(1.0, max(0.0, 0.5 - d))


def render():
    rows = []
    c = SIZE / 2
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            px, py = x + 0.5, y + 0.5
            a = coverage(rounded_sdf(px, py, c, c, 0.41 * SIZE, 0.41 * SIZE, 0.185 * SIZE))
            if a <= 0:
                row += b"\0\0\0\0"
                continue
            t = (py - c) / SIZE + 0.5
            col = lerp(PANEL, (0x24, 0x24, 0x23), t)
            # Raised transport slab.
            d = rounded_sdf(px, py, c, c, 0.29 * SIZE, 0.29 * SIZE, 0.11 * SIZE)
            k = coverage(d)
            slab = lerp(FACE_TOP, FACE_BOTTOM, min(1, max(0, (py - 0.21 * SIZE) / (0.58 * SIZE))))
            if d < 0 and d > -4:
                slab = lerp(slab, (0x58, 0x58, 0x55), coverage(d + 4) * 0.6) if py < c else slab
            col = lerp(col, slab, k)
            # Teal ring: the "O".
            rr = math.hypot(px - c, py - c)
            ring = coverage(abs(rr - 0.165 * SIZE) - 0.052 * SIZE)
            if ring > 0:
                ang = math.atan2(py - c, px - c)
                tone = lerp(ACCENT_LO, ACCENT, 0.5 + 0.5 * math.cos(ang + 0.9))
                col = lerp(col, tone, ring)
            # Playhead LED in the ring's centre.
            led = coverage(rr - 0.045 * SIZE)
            glow = max(0.0, 1 - rr / (0.10 * SIZE)) ** 2 * 0.35
            if led > 0 or glow > 0:
                col = lerp(col, LED, max(led, glow))
            row += bytes((*col, int(round(a * 255))))
        rows.append(row)
    return rows


def png(rows):
    raw = b"".join(b"\0" + bytes(r) for r in rows)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def main():
    out = os.path.join(os.path.dirname(__file__), "..", "desktop", "assets", "Ondera.icns")
    with tempfile.TemporaryDirectory() as tmp:
        master = os.path.join(tmp, "master.png")
        with open(master, "wb") as f:
            f.write(png(render()))
        iconset = os.path.join(tmp, "Ondera.iconset")
        os.mkdir(iconset)
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                name = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
                subprocess.run(["sips", "-z", str(size * scale), str(size * scale), master, "--out",
                                os.path.join(iconset, name)], check=True, capture_output=True)
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", out], check=True)
        if len(sys.argv) > 1:
            subprocess.run(["cp", master, sys.argv[1]], check=True)
    print(f"Wrote {os.path.normpath(out)}")


if __name__ == "__main__":
    main()
