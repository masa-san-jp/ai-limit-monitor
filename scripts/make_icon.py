#!/usr/bin/env python3
"""仮アイコン(256x256の青い円、透過PNG)を app/src-tauri/icons/icon.png に生成する。
差し替えたい場合は任意の1024x1024 PNG を置いて `cargo tauri icon <png>` を実行。"""
import os, struct, zlib

W = H = 256
rows = []
for y in range(H):
    row = bytearray([0])
    for x in range(W):
        dx, dy = x - 128, y - 128
        row += bytes((55, 138, 221, 255)) if dx * dx + dy * dy < 110 * 110 else bytes((0, 0, 0, 0))
    rows.append(bytes(row))

def chunk(t, d):
    c = struct.pack(">I", len(d)) + t + d
    return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(b"".join(rows)))
       + chunk(b"IEND", b""))
out = os.path.join(os.path.dirname(__file__), "..", "app", "src-tauri", "icons")
os.makedirs(out, exist_ok=True)
open(os.path.join(out, "icon.png"), "wb").write(png)
print("wrote", os.path.normpath(os.path.join(out, "icon.png")))
