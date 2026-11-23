#!/usr/bin/env python3
"""Render original PixelCraft branding and Rust-exported sample art with librsvg.
No generated/fabricated UI is used. Run from any directory.
"""
from pathlib import Path
import base64
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'packaging/google-play/en-US/graphics'
SRC = Path(__file__).resolve().parent


def uri(path):
    return 'data:image/png;base64,' + base64.b64encode(path.read_bytes()).decode()


def render(svg, target):
    with tempfile.TemporaryDirectory(prefix='pixelcraft-play-') as d:
        p=Path(d)/'source.svg';p.write_text(svg)
        subprocess.run(['rsvg-convert',str(p),'-o',str(target)],check=True)


def main():
    logo=uri(ROOT/'assets/pixelcraft-logo.png')
    render(f'''<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="512" height="512">
      <rect width="512" height="512" fill="#142c30"/>
      <image x="0" y="0" width="512" height="512" xlink:href="{logo}" style="image-rendering:pixelated"/>
    </svg>''',OUT/'icon-512.png')
    from PIL import Image
    with Image.open(SRC/'garden-sample.png') as sample: sample.verify()
    art=uri(SRC/'garden-sample.png')
    svg=f'''<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1024" height="500" viewBox="0 0 1024 500">
      <rect width="1024" height="500" fill="#11575b"/>
      <path d="M0 444 H1024 V500 H0Z" fill="#0c484b"/>
      <g fill="#2a7170"><rect x="790" y="44" width="22" height="22"/><rect x="840" y="88" width="12" height="12"/><rect x="930" y="165" width="16" height="16"/><rect x="490" y="346" width="16" height="16"/><rect x="558" y="56" width="12" height="12"/></g>
      <g font-family="Noto Sans, sans-serif" fill="#e8f5e9">
        <text x="76" y="126" font-size="28" font-weight="700">PixelCraft</text>
        <text x="72" y="205" font-size="52" font-weight="700">Pixel art.</text>
        <text x="72" y="270" font-size="52" font-weight="700">Made by you.</text>
        <text x="76" y="335" font-size="23" fill="#b9dfd5">Draw. Layer. Animate.</text>
      </g>
      <image x="560" y="62" width="360" height="360" xlink:href="{art}" style="image-rendering:pixelated"/>
      <g transform="translate(76 378)">
        <rect width="24" height="24" fill="#62d2b0"/><rect x="34" width="24" height="24" fill="#e76268"/><rect x="68" width="24" height="24" fill="#f0c467"/><rect x="102" width="24" height="24" fill="#b2ce72"/>
      </g>
    </svg>'''
    render(svg,OUT/'feature-graphic-1024x500.png')
    # SVG contains no transparency. Save as true RGB PNG, as required for the feature graphic.
    from PIL import Image
    p=OUT/'feature-graphic-1024x500.png'
    with Image.open(p) as im: im.convert('RGB').save(p)
    p=OUT/'icon-512.png'
    with Image.open(p) as im: im.convert('RGBA').save(p)
    (SRC/'feature-graphic.svg').write_text(svg)


if __name__=='__main__':main()
