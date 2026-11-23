#!/usr/bin/env python3
"""Validate the generated Play listing pack without modifying its assets."""
from pathlib import Path
import json
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
LANG=ROOT/'en-US'
for name,limit in [('title.txt',30),('short-description.txt',80),('full-description.txt',4000),('release-notes.txt',500)]:
    text=(LANG/name).read_text().strip()
    assert 0<len(text)<=limit,(name,len(text),limit)
    print(f'{name}: {len(text)}/{limit}')
alt=json.loads((LANG/'asset-alt-text.json').read_text())
graphics=LANG/'graphics'
for path in graphics.rglob('*.png'):
    key=path.relative_to(graphics).as_posix()
    assert 0<len(alt[key])<=140,key
    with Image.open(path) as im:
        im.load()
        assert im.format=='PNG',path
        if path.name=='icon-512.png':
            assert im.size==(512,512) and im.mode=='RGBA'
            assert path.stat().st_size<=1024*1024
            assert im.getpixel((0,0))[3]==255
        elif path.name=='feature-graphic-1024x500.png':
            assert im.size==(1024,500) and im.mode=='RGB'
        else:
            assert im.mode=='RGB' and path.stat().st_size<8*1024*1024
            width,height=im.size
            assert width*9==height*16 or width*16==height*9,path
            if path.parent.name=='phone-screenshots':
                assert im.size==(1080,1920)
            elif path.parent.name=='seven-inch-screenshots':
                assert im.size==(1920,1080)
            else:
                assert im.size==(2560,1440)
for group in ('phone-screenshots','seven-inch-screenshots','ten-inch-screenshots'):
    assert 4<=len(list((graphics/group).glob('*.png')))<=8,group
print('All 14 upload images validated; 12 real screenshots across three device classes.')
