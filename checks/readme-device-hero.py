#!/usr/bin/env python3
"""Compose real PixelCraft captures in original device frames; requires rsvg-convert.

Run from any directory. Desktop captures must be 1920x1080; Android captures
retain their native aspect ratio. No screenshot content is painted or recolored.
"""
from pathlib import Path
import base64
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def screenshot(path):
    return 'data:image/png;base64,' + base64.b64encode((ROOT / path).read_bytes()).decode()


def build(theme):
    dark = theme == 'dark'
    edge = '#727b85' if dark else '#a1a8ae'
    desktop = screenshot('shots/pixelcraft.png' if dark else 'shots/pixelcraft-light.png')
    tablet = screenshot(f'docs/screenshots/android-tablet-timeline-{theme}.png')
    phone = screenshot(f'docs/screenshots/android-phone-portrait-{theme}.png')
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1920" height="1080" viewBox="0 0 1920 1080">
      <defs>
        <linearGradient id="metal" x2="0" y2="1"><stop stop-color="#c9ced3"/><stop offset=".55" stop-color="#8d969f"/><stop offset="1" stop-color="#c1c7cc"/></linearGradient>
        <linearGradient id="bezel" x2="1" y2="1"><stop stop-color="#30363d"/><stop offset=".6" stop-color="#111519"/><stop offset="1" stop-color="#343c44"/></linearGradient>
        <radialGradient id="shadow"><stop stop-color="#000" stop-opacity=".26"/><stop offset="1" stop-color="#000" stop-opacity="0"/></radialGradient>
        <clipPath id="laptop"><rect x="318" y="144" width="1280" height="720" rx="5"/></clipPath>
        <clipPath id="tablet"><rect x="1122" y="570" width="704" height="440" rx="16"/></clipPath>
        <clipPath id="phone"><rect x="113" y="383" width="252" height="560" rx="29"/></clipPath>
      </defs>
      <ellipse cx="970" cy="945" rx="850" ry="90" fill="url(#shadow)"/>
      <rect x="296" y="116" width="1324" height="777" rx="29" fill="url(#bezel)" stroke="{edge}" stroke-width="2"/>
      <image x="318" y="144" width="1280" height="720" xlink:href="{desktop}" clip-path="url(#laptop)"/>
      <circle cx="958" cy="131" r="3" fill="#526674"/>
      <path d="M296 893 L1620 893 L1720 936 Q1723 946 1697 951 L222 951 Q196 946 200 936 Z" fill="url(#metal)" stroke="#8b949e"/>
      <path d="M822 893 H1094 L1081 906 Q1076 910 1068 910 H848 Q840 910 836 906 Z" fill="#717a83"/>
      <path d="M211 937 H1708" stroke="#e2e6e9" stroke-width="2"/>
      <ellipse cx="1474" cy="1024" rx="428" ry="32" fill="url(#shadow)"/>
      <rect x="1103" y="551" width="742" height="478" rx="33" fill="url(#bezel)" stroke="{edge}" stroke-width="2"/>
      <image x="1122" y="570" width="704" height="440" xlink:href="{tablet}" clip-path="url(#tablet)"/>
      <circle cx="1112" cy="790" r="3" fill="#526674"/>
      <g transform="rotate(10 239 663)">
        <rect x="99" y="370" width="280" height="587" rx="42" fill="url(#metal)" stroke="{edge}" stroke-width="2"/>
        <rect x="103" y="373" width="272" height="580" rx="39" fill="#111519"/>
        <image x="113" y="383" width="252" height="560" xlink:href="{phone}" clip-path="url(#phone)"/>
        <rect x="96" y="488" width="3" height="49" rx="1.5" fill="#89939c"/>
        <rect x="379" y="502" width="3" height="64" rx="1.5" fill="#89939c"/>
      </g>
    </svg>'''
    with tempfile.TemporaryDirectory(prefix='pixelcraft-readme-hero-') as directory:
        source = Path(directory) / 'devices.svg'
        source.write_text(svg)
        subprocess.run(['rsvg-convert', str(source), '-o', str(ROOT / f'shots/pixelcraft-devices-{theme}.png')], check=True)


if __name__ == '__main__':
    for mode in ('light', 'dark'):
        build(mode)
