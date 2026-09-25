#!/usr/bin/env python3
"""Bitfold — GIF artifact generator (deterministic).

Renders the same six frames as src/svg.rs (760x320, house palette,
Consolas) into:

  assets/anim.gif   — 6-frame animation, 2 s per frame, infinite loop
  assets/sheet.gif  — static 3x2 contact sheet (0.34 scale, same layout)

No timestamps, no randomness: the output bytes are a pure function of the
pinned corpus/ledger numbers below (the frozen values from
assets/report.txt). Re-run any time; the hashes must match.
"""

import os

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "assets")

# House palette (src/svg.rs).
PAPER = (0xF7, 0xF6, 0xF1)
INK = (0x25, 0x25, 0x24)
MUTED = (0x67, 0x66, 0x62)
HAIR = (0xDC, 0xDA, 0xD1)
GREEN = (0x22, 0xAC, 0x80)
INDIGO = (0x5B, 0x51, 0xC7)
BRICK = (0xA7, 0x42, 0x21)  # noqa: F841 (kept for palette parity)
ORANGE = (0xE8, 0x83, 0x3A)
BLACK = (0, 0, 0)
WHITE = (255, 255, 255)

PALETTE_COLORS = [PAPER, INK, MUTED, HAIR, GREEN, INDIGO, ORANGE, BLACK, WHITE]

W, H = 760, 320

# Pinned data (frozen 2026-01-01 report — see assets/report.txt).
PARTS = [
    ("rfc1951", 36944, 92, 4.33486),
    ("rfc8439", 88847, 97, 4.72736),
    ("b0000796", 148898, 12, 3.50329),
]
TOTAL_BYTES = 274689
TOTAL_H = 4.53187
TOTAL_BOUND = 1.7653
KAT_GROUPS = [
    ("rfc1951", 13),
    ("bits", 4),
    ("blocks", 5),
    ("hash", 162),
    ("xval", 4),
]
KAT_TOTAL = 188
ENCODER = dict(
    bits=753194,
    ratio="0.3428",
    below=491662,
    eff="165.28",
    lit=23227,
    match=41141,
    copied=251462,
)


def fonts():
    base = r"C:\Windows\Fonts"
    reg = os.path.join(base, "consola.ttf")
    bold = os.path.join(base, "consolab.ttf")
    cache = {}

    def f(size, b=False):
        key = (size, b)
        if key not in cache:
            cache[key] = ImageFont.truetype(bold if b else reg, size)
        return cache[key]

    return f


def to_palette(img):
    pal = Image.new("P", (1, 1), 0)
    flat = [c for col in PALETTE_COLORS for c in col]
    flat += [0] * (768 - len(flat))
    pal.putpalette(flat)
    return img.convert("P", palette=pal, dither=Image.Dither.NONE)


class Ctx:
    def __init__(self, img):
        self.d = ImageDraw.Draw(img)
        self.f = fonts()

    def text(self, x, y, s, size, color=INK, bold=False, center=False):
        fnt = self.f(size, bold)
        w = self.d.textlength(s, font=fnt)
        if center:
            x = x - w / 2
        self.d.text((x, y), s, font=fnt, fill=color)

    def text_right(self, x, y, s, size, color=INK, bold=False):
        fnt = self.f(size, bold)
        w = self.d.textlength(s, font=fnt)
        self.d.text((x - w, y), s, font=fnt, fill=color)

    def rule(self, x1, x2, y, color=HAIR, width=2):
        self.d.line([(x1, y), (x2, y)], fill=color, width=width)


def f0_title(c):
    c.text(380, 78, "bitfold", 54, INK, bold=True, center=True)
    c.rule(180, 580, 150)
    c.text(
        380,
        172,
        "RFC 1951 DEFLATE + CRC-32 / Adler-32 — from scratch, std-only Rust",
        15,
        MUTED,
        center=True,
    )
    c.text(
        380, 200, "entropy folds the data down; the ledger measures how far", 14, INDIGO, center=True
    )
    c.text(380, 282, "bitfold 1.0.0 · 6 frames · 2 s each", 11, MUTED, center=True)


def f1_corpus(c):
    c.text(60, 42, "the corpus", 24, INK, bold=True)
    c.rule(60, 700, 74)
    for i, (name, n, sym, _h) in enumerate(PARTS):
        y = 104 + i * 34
        c.text(150, y, name, 16, INK)
        c.text_right(560, y, f"{n} bytes", 16, INK)
        c.text_right(640, y, f"{sym} syms", 13, MUTED)
    y = 104 + 3 * 34 + 16
    c.text(150, y, "total", 16, INK, bold=True)
    c.text_right(560, y, f"{TOTAL_BYTES} bytes", 16, INK, bold=True)
    c.text_right(640, y, "public-domain", 13, MUTED)
    c.text(60, 282, "RFC 1951 (the spec) · RFC 8439 (ChaCha20-Poly1305) · OEIS A000796 (prime digits)", 11, MUTED)


def f2_entropy(c):
    c.text(60, 42, "entropy ledger", 24, INK, bold=True)
    c.rule(60, 700, 74)
    c.text(60, 88, "Shannon H (bits/byte), bar = H / 8 of the full byte", 12, MUTED)
    base_w = 340.0
    for i, (name, _n, _s, h) in enumerate(PARTS):
        y = 118 + i * 52
        c.text(150, y - 4, name, 14, INK)
        c.text_right(690, y - 4, f"H = {h:.5f} b/B", 13, MUTED)
        c.d.rectangle([330, y + 8, 330 + base_w, y + 8 + 14], outline=HAIR)
        bw = max(1, int(base_w * h / 8.0)) if h > 0 else 0
        if bw:
            c.d.rectangle([330, y + 8, 330 + bw, y + 8 + 14], fill=INDIGO)
    c.text(150, 280, "total", 14, INK, bold=True)
    c.text_right(690, 280, f"H = {TOTAL_H:.5f} b/B · bound {TOTAL_BOUND:.4f}", 14, INDIGO)


def f3_encoder(c):
    c.text(60, 42, "the encoder", 24, INK, bold=True)
    c.rule(60, 700, 74)
    c.text(60, 104, "LZ77: chained hash + lazy matching · window 32768 · match 3..258", 15, MUTED)
    c.text(60, 136, "Huffman: canonical codes · stored / fixed / dynamic blocks", 15, MUTED)
    c.text(60, 168, "bits: data LSB-first · codes MSB-first · extras LSB-first (zlib order)", 15, MUTED)
    c.text(
        60,
        208,
        f"corpus → {ENCODER['bits']} logical bits → ratio {ENCODER['ratio']}",
        15,
        INK,
        bold=True,
    )
    c.text(60, 240, "zlib container: 78 9c header + Adler-32 trailer (BE)", 15, INK)
    c.text(
        60,
        282,
        f"{ENCODER['lit']} literals · {ENCODER['match']} matches · {ENCODER['copied']} bytes copied",
        11,
        MUTED,
    )


def f4_verification(c):
    c.text(60, 42, "verification", 24, INK, bold=True)
    c.rule(60, 700, 74)
    for i, (name, t) in enumerate(KAT_GROUPS):
        y = 96 + i * 34
        c.text(150, y, name, 15, INK)
        c.text_right(560, y, f"{t} checks", 15, INK)
        c.text_right(640, y, f"{t} pass", 15, GREEN)
    y = 96 + 5 * 34 + 12
    c.text(150, y, "total", 16, INK, bold=True)
    c.text_right(560, y, f"{KAT_TOTAL} checks", 16, INK, bold=True)
    c.text_right(640, y, f"{KAT_TOTAL} pass · ALL PASS", 16, GREEN, bold=True)


def f5_verdict(c):
    c.text(380, 84, "byte-identical", 30, GREEN, bold=True, center=True)
    c.rule(180, 580, 135)
    c.text(380, 158, "dual in-process render · SHA-256 compared · pinned project date", 15, INK, center=True)
    c.text(380, 184, "cross-verified both directions against CPython zlib", 14, MUTED, center=True)
    c.text(380, 210, "our dynamic stream decodes in zlib · zlib's streams decode here", 14, MUTED, center=True)
    c.text(380, 278, "(c) 2026 Adithya N Raj — determinism is the brand", 12, MUTED, center=True)


def render_frames():
    frames = []
    for draw in (f0_title, f1_corpus, f2_entropy, f3_encoder, f4_verification, f5_verdict):
        img = Image.new("RGB", (W, H), PAPER)
        draw(Ctx(img))
        frames.append(to_palette(img))
    return frames


def make_anim(path, frames):
    frames[0].save(
        path,
        save_all=True,
        append_images=frames[1:],
        duration=2000,
        loop=0,
        optimize=False,
    )


def make_sheet(path, frames):
    scale = 0.34
    cw, ch = int(W * scale), int(H * scale)  # 258 x 108
    gap, margin = 8, 8
    title_h = 22
    cols, rows = 3, 2
    w = margin * 2 + gap + cols * (cw + gap)
    h = title_h + margin + gap + rows * (ch + gap)
    img = Image.new("RGB", (w, h), PAPER)
    c = Ctx(img)
    c.text(margin, 6, "bitfold — 6-frame story (static)", 13, INK, bold=True)
    for i, f in enumerate(frames):
        r, col = divmod(i, cols)
        x = margin + gap + col * (cw + gap)
        y = title_h + margin + gap + r * (ch + gap)
        small = f.resize((cw, ch), Image.Resampling.LANCZOS)
        img.paste(small, (x, y))
        c.d.rectangle([x, y, x + cw - 1, y + ch - 1], outline=HAIR)
    to_palette(img).save(path, optimize=False)


def main():
    frames = render_frames()
    os.makedirs(ASSETS, exist_ok=True)
    anim = os.path.join(ASSETS, "anim.gif")
    sheet = os.path.join(ASSETS, "sheet.gif")
    make_anim(anim, frames)
    make_sheet(sheet, frames)
    import hashlib

    for p in (anim, sheet):
        h = hashlib.sha256(open(p, "rb").read()).hexdigest()
        print(f"wrote {p} ({os.path.getsize(p)} bytes) sha256 {h}")


if __name__ == "__main__":
    main()
