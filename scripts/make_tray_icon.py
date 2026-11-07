"""Render the monochrome menu-bar template icon (lens with a spark) as a PNG.

Usage: python3 scripts/make_tray_icon.py src-tauri/icons/tray.png
"""

import math
import struct
import sys
import zlib

SIZE = 44
SUPER = 4


def coverage(x: float, y: float) -> bool:
    cx, cy, r_out, r_in = 19.0, 19.0, 13.0, 9.5
    d = math.hypot(x - cx, y - cy)
    if r_in <= d <= r_out:
        return True
    if handle(x, y):
        return True
    return spark(x - cx, y - cy)


def handle(x: float, y: float) -> bool:
    ax, ay, bx, by, w = 28.0, 28.0, 38.0, 38.0, 2.6
    t = max(0.0, min(1.0, ((x - ax) * (bx - ax) + (y - ay) * (by - ay)) / 200.0))
    px, py = ax + t * (bx - ax), ay + t * (by - ay)
    return math.hypot(x - px, y - py) <= w


def spark(dx: float, dy: float) -> bool:
    ax, ay = abs(dx), abs(dy)
    return ax / 6.5 + ay / 1.6 <= 1.0 or ax / 1.6 + ay / 6.5 <= 1.0


def render() -> bytes:
    rows = []
    for py in range(SIZE):
        row = bytearray([0])
        for px in range(SIZE):
            hits = sum(
                coverage(px + (sx + 0.5) / SUPER, py + (sy + 0.5) / SUPER)
                for sx in range(SUPER)
                for sy in range(SUPER)
            )
            alpha = round(255 * hits / (SUPER * SUPER))
            row += bytes([0, 0, 0, alpha])
        rows.append(bytes(row))
    return b"".join(rows)


def png(raw: bytes) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


if __name__ == "__main__":
    with open(sys.argv[1], "wb") as fh:
        fh.write(png(render()))
