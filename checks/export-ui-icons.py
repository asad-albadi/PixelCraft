#!/usr/bin/env python3
"""Export the UI SVG sources as white RGBA PNGs for theme tinting.

Requires rsvg-convert from librsvg. This is an asset-authoring tool and is
independent of PixelCraft's application build.
"""

from pathlib import Path
from html import escape
import json
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parent.parent / "assets/icons/ui"
NS = "http://www.w3.org/2000/svg"
STROKES = {16: "2.25", 20: "2", 24: "1.75", 32: "1.75", 40: "1.75", 48: "1.75"}


def write_preview(manifest):
    sections = []
    categories = dict.fromkeys(icon["category"] for icon in manifest["icons"])
    for category in categories:
        cards = []
        for icon in manifest["icons"]:
            if icon["category"] != category:
                continue
            name, label = icon["name"], escape(icon["label"])
            downloads = " ".join(
                f'<a href="png/{size}/{name}.png" download>{size} px</a>'
                for size in manifest["pngSizes"]
            )
            cards.append(f'''<article data-search="{escape(category + ' ' + name + ' ' + icon['label'])}">
<a class="source" href="svg/{name}.svg" download title="Download {label} SVG">
<img src="svg/{name}.svg" alt="{label}"><span>{label}</span></a>
<div class="downloads">{downloads}</div></article>''')
        sections.append(f'<section><h2>{escape(category)}</h2><div class="grid">' + "".join(cards) + '</div></section>')
    page = '''<!doctype html>
<html lang="en" data-theme="dark"><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>PixelCraft UI icon assets</title>
<style>
:root { font-family: Inter, system-ui, sans-serif; --size:24px; }
html[data-theme=dark] { color-scheme:dark; --bg:#152125; --surface:#1e3035; --text:#edf2f2; --muted:#a7b8bb; --line:#3b5056; --accent:#256e72; --filter:invert(1); }
html[data-theme=light] { color-scheme:light; --bg:#f6f4ee; --surface:#fffefa; --text:#233337; --muted:#50666b; --line:#bac7c8; --accent:#d0ece9; --filter:none; }
* { box-sizing:border-box; }
body { max-width:1280px; margin:auto; padding:32px; background:var(--bg); color:var(--text); }
h1 { font-size:26px; margin:0 0 8px; font-weight:600; }
p { color:var(--muted); margin:0; line-height:1.6; }
.controls { display:flex; flex-wrap:wrap; gap:12px; align-items:center; margin:24px 0; }
input,select,button { color:var(--text); border:1px solid var(--line); background:var(--surface); padding:9px 12px; border-radius:6px; font:inherit; }
input { min-width:200px; flex:1; }
button { cursor:pointer; }
:focus-visible { outline:2px solid #35b7b1; outline-offset:3px; }
h2 { font-size:16px; font-weight:600; margin:30px 0 12px; }
.grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(145px,1fr)); gap:1px; border:1px solid var(--line); background:var(--line); }
article { text-align:center; background:var(--surface); padding:16px 8px 12px; }
img { width:var(--size); height:var(--size); filter:var(--filter); }
.source { display:flex; min-height:65px; gap:12px; flex-direction:column; align-items:center; justify-content:start; color:var(--text); text-decoration:none; }
.source span { font-size:12px; line-height:1.4; }
.downloads { display:flex; justify-content:center; gap:10px; margin-top:8px; }
.downloads a { font-size:10px; color:var(--muted); text-underline-offset:3px; }
.states { display:flex; gap:18px; flex-wrap:wrap; }
.state { display:flex; flex-direction:column; align-items:center; gap:8px; font-size:12px; color:var(--muted); }
.sample { display:grid; place-items:center; height:36px; width:36px; padding:0; }
.sample img { width:20px; height:20px; }
.hover { background:var(--line); }
.selected { background:var(--accent); border-color:#35b7b1; }
.focused { outline:2px solid #35b7b1; outline-offset:2px; }
.sample:disabled img { opacity:.35; }
article[hidden],section[hidden] { display:none; }
footer { margin-top:28px; font-size:12px; }
@media(max-width:600px) { body { padding:18px; } }
</style>
<h1>PixelCraft UI icons</h1>
<p>__COUNT__ transparent icons. Download SVG sources or white PNGs for theme tinting.</p>
<div class="controls"><input id="search" type="search" placeholder="Find an icon" aria-label="Find an icon">
<label>Size <select id="size"><option>16</option><option>20</option><option selected>24</option></select></label>
<button id="theme">Use light theme</button></div>
<h2>Button states</h2>
<div class="states">
<div class="state"><button class="sample" aria-label="Pencil"><img src="svg/pencil.svg" alt=""></button>Default</div>
<div class="state"><button class="sample hover" aria-label="Pencil hover sample"><img src="svg/pencil.svg" alt=""></button>Hover</div>
<div class="state"><button class="sample selected" aria-label="Pencil selected" aria-pressed="true"><img src="svg/pencil.svg" alt=""></button>Selected</div>
<div class="state"><button class="sample focused" aria-label="Pencil focus sample"><img src="svg/pencil.svg" alt=""></button>Focus</div>
<div class="state"><button class="sample" aria-label="Pencil disabled" disabled><img src="svg/pencil.svg" alt=""></button>Disabled</div>
</div>
__SECTIONS__
<footer><p>Original icons under the repository's MIT license. App integration checks include theme contrast, keyboard and accessibility behavior, and display scaling.</p></footer>
<script>
document.querySelector('#theme').onclick = () => {
  const light = document.documentElement.dataset.theme !== 'light';
  document.documentElement.dataset.theme = light ? 'light' : 'dark';
  document.querySelector('#theme').textContent = light ? 'Use dark theme' : 'Use light theme';
};
document.querySelector('#size').onchange = event => {
  document.documentElement.style.setProperty('--size', event.target.value + 'px');
};
document.querySelector('#search').oninput = event => {
  const query = event.target.value.toLowerCase().trim();
  document.querySelectorAll('article').forEach(card => {
    card.hidden = !card.dataset.search.toLowerCase().includes(query);
  });
  document.querySelectorAll('section').forEach(section => {
    section.hidden = !section.querySelector('article:not([hidden])');
  });
};
</script></html>'''
    (ROOT / "preview.html").write_text(
        page.replace("__COUNT__", str(len(manifest["icons"])))
        .replace("__SECTIONS__", "".join(sections))
    )


def main():
    renderer = shutil.which("rsvg-convert")
    if renderer is None:
        raise SystemExit("Install librsvg's rsvg-convert to export the icon PNGs.")
    manifest = json.loads((ROOT / "manifest.json").read_text())
    with tempfile.TemporaryDirectory(prefix="pixelcraft-icon-export-") as temp:
        stylesheet = Path(temp) / "icons.css"
        stylesheet.write_text("svg { color: #ffffff; }")
        for icon in manifest["icons"]:
            source = ROOT / "svg" / f"{icon['name']}.svg"
            document = ET.parse(source)
            assert document.getroot().attrib["viewBox"] == "0 0 24 24", source
            assert document.getroot().tag == f"{{{NS}}}svg", source
            for element in document.iter():
                assert element.tag.split("}")[-1] in {
                    "svg", "title", "path", "rect", "circle", "ellipse", "g"
                }, source
                assert all("href" not in key for key in element.attrib), source
            for size in manifest["pngSizes"]:
                svg = source.read_text().replace(
                    'stroke-width="1.75"', f'stroke-width="{STROKES[size]}"', 1
                )
                destination = ROOT / "png" / str(size) / f"{icon['name']}.png"
                destination.parent.mkdir(parents=True, exist_ok=True)
                subprocess.run(
                    [renderer, "--stylesheet", str(stylesheet),
                     "--width", str(size), "--height", str(size),
                     "--output", str(destination)],
                    input=svg.encode(), check=True, capture_output=True,
                )
    write_preview(manifest)
    print(f"Exported {len(manifest['icons'])} icons at 16/20/24 logical and 32/40/48 density sizes with a local preview.")


if __name__ == "__main__":
    main()
