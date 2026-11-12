use crate::{model::*, workflows::*};
use image::AnimationDecoder;
use serde_json::{Value, json};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
};

pub fn from_frames(
    frames: impl Iterator<Item = Result<image::Frame, image::ImageError>>,
) -> Result<Document, String> {
    let mut doc: Option<Document> = None;
    for item in frames {
        let frame = item.map_err(|e| e.to_string())?;
        let (num, den) = frame.delay().numer_denom_ms();
        let im = frame.into_buffer();
        let (w, h) = (im.width() as usize, im.height() as usize);
        if w == 0
            || h == 0
            || w > MAX_DIMENSION
            || h > MAX_DIMENSION
            || w.saturating_mul(h) > MAX_PIXELS
        {
            return Err("Animation exceeds limits".into());
        }
        if doc.is_none() {
            doc = Some(Document::new(w, h));
        }
        let d = doc.as_mut().unwrap();
        if d.width != w || d.height != h {
            return Err("Animation frames must use the same canvas".into());
        }
        let f = if d.layers[0].name == "__first__" {
            let f = d.durations.len();
            if f >= 65535 || !d.can_grow(w * h) {
                return Err("Animation exceeds limits".into());
            }
            d.add_frame(f - 1, false);
            f
        } else {
            d.layers[0].name = "__first__".into();
            0
        };
        d.durations[f] = (num / den.max(1)).clamp(1, 65535) as u16;
        d.layers[0].cels[f] = im.pixels().map(|p| p.0).collect();
    }
    let mut d = doc.ok_or("Animation has no frames")?;
    d.layers[0].name = "Animation".into();
    d.validate()?;
    Ok(d)
}
pub fn load_animation(path: &Path) -> Result<Document, String> {
    let file = || {
        std::fs::File::open(path)
            .map(BufReader::new)
            .map_err(|e| e.to_string())
    };
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "gif" => from_frames(
            image::codecs::gif::GifDecoder::new(file()?)
                .map_err(|e| e.to_string())?
                .into_frames(),
        ),
        "png" | "apng" => {
            let dec = image::codecs::png::PngDecoder::new(file()?).map_err(|e| e.to_string())?;
            if dec.is_apng().map_err(|e| e.to_string())? {
                from_frames(dec.apng().map_err(|e| e.to_string())?.into_frames())
            } else {
                Err("Not an animated PNG".into())
            }
        }
        "webp" => load_webp(path),
        _ => Err("Unknown animation format".into()),
    }
}
pub fn load_sequence(paths: &[PathBuf]) -> Result<Document, String> {
    let mut paths = paths.to_vec();
    paths.sort_by_key(|a| natural_key(a));
    let frames = paths.iter().map(|p| {
        image::open(p).map(|im| {
            image::Frame::from_parts(
                im.to_rgba8(),
                0,
                0,
                image::Delay::from_numer_denom_ms(100, 1),
            )
        })
    });
    from_frames(frames)
}
fn natural_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut out = String::new();
    let mut digits = String::new();
    for c in s.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            digits.push(c)
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{:0>20}", digits));
                digits.clear();
            }
            out.push(c);
        }
    }
    out
}
pub fn export_sheet(path: &Path, d: &Document, o: &SheetOptions) -> Result<(), String> {
    let (im, mut meta) = pack_sheet(d, o)?;
    let name = path
        .file_name()
        .ok_or("Invalid filename")?
        .to_string_lossy();
    meta["meta"]["image"] = json!(name);
    im.save(path).map_err(|e| e.to_string())?;
    write_json(&path.with_extension("json"), &meta)?;
    if o.preset == "Godot" {
        std::fs::write(path.with_extension("tres"), godot(&meta, &name)?)
            .map_err(|e| e.to_string())?;
    }
    if o.preset == "Unity" {
        let meta_path = PathBuf::from(format!("{}.meta", path.display()));
        let mut output = unity(&meta, &path.to_string_lossy(), im.height());
        if let Ok(previous) = std::fs::read_to_string(&meta_path)
            && let Some(guid) = previous
                .lines()
                .find_map(|l| l.strip_prefix("guid: "))
                .filter(|v| v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit()))
            && let Some(line) = output.lines().find(|l| l.starts_with("guid: "))
        {
            output = output.replace(line, &format!("guid: {guid}"));
        }
        std::fs::write(meta_path, output).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn godot(meta: &Value, name: &str) -> Result<String, String> {
    let frames = meta["frames"].as_array().unwrap();
    let quoted = serde_json::to_string(&format!("res://{name}")).unwrap();
    let mut s = format!(
        "[gd_resource type=\"SpriteFrames\" load_steps={} format=3]\n\n[ext_resource type=\"Texture2D\" path={quoted} id=\"1\"]\n",
        frames.len() + 2
    );
    for (i, f) in frames.iter().enumerate() {
        let r = &f["frame"];
        s += &format!(
            "\n[sub_resource type=\"AtlasTexture\" id=\"Atlas_{i}\"]\natlas = ExtResource(\"1\")\nregion = Rect2({}, {}, {}, {})\nmargin = Rect2({}, {}, {}, {})\n",
            r["x"],
            r["y"],
            r["w"],
            r["h"],
            f["spriteSourceSize"]["x"],
            f["spriteSourceSize"]["y"],
            f["sourceSize"]["w"].as_i64().unwrap() - r["w"].as_i64().unwrap(),
            f["sourceSize"]["h"].as_i64().unwrap() - r["h"].as_i64().unwrap()
        );
    }
    let default =
        vec![json!({"name":"default","from":0,"to":frames.len()-1,"direction":"forward"})];
    let tags = meta["meta"]["frameTags"]
        .as_array()
        .filter(|v| !v.is_empty())
        .unwrap_or(&default);
    let mut animations = vec![];
    let mut names = std::collections::HashSet::new();
    for t in tags {
        let mut order: Vec<usize> =
            (t["from"].as_u64().unwrap() as usize..=t["to"].as_u64().unwrap() as usize).collect();
        let direction = t["direction"].as_str().unwrap_or("forward");
        if direction == "reverse" || direction == "pingpong_reverse" {
            order.reverse()
        }
        if direction.starts_with("pingpong") && order.len() > 2 {
            let back: Vec<_> = order[1..order.len() - 1].iter().rev().copied().collect();
            order.extend(back)
        }
        let repeat = t["repeat"].as_u64().unwrap_or(0);
        if order.len().saturating_mul(repeat.max(1) as usize) > 1_000_000 {
            return Err("Godot finite animation exceeds one million entries".into());
        }
        if repeat > 0 {
            // SpriteFrames has a boolean loop flag: finite repeats are expanded.
            let cycle = order.clone();
            for _ in 1..repeat {
                order.extend_from_slice(&cycle);
            }
        }
        let base = t["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Animation");
        let mut name = base.to_string();
        let mut suffix = 2;
        while !names.insert(name.clone()) {
            name = format!("{base}_{suffix}");
            suffix += 1;
        }
        let fs: Vec<String> = order
            .iter()
            .map(|i| {
                format!(
                    "{{\"duration\": {}, \"texture\": SubResource(\"Atlas_{i}\")}}",
                    frames[*i]["duration"].as_f64().unwrap() / 1000.
                )
            })
            .collect();
        animations.push(format!(
            "{{\"frames\": [{}], \"loop\": {}, \"name\": &{}, \"speed\": 1.0}}",
            fs.join(", "),
            repeat == 0,
            serde_json::to_string(&name).unwrap()
        ));
    }
    s += &format!("\n[resource]\nanimations = [{}]\n", animations.join(",\n"));
    Ok(s)
}
fn unity(meta: &Value, name: &str, height: u32) -> String {
    // Stable asset identity; export_sheet retains a pre-existing Unity GUID.
    let mut hash = 0xcbf29ce484222325u64;
    for b in name.bytes() {
        hash = (hash ^ b as u64).wrapping_mul(0x100000001b3)
    }
    let mut s = format!(
        "fileFormatVersion: 2\nguid: {hash:016x}{:016x}\nTextureImporter:\n  serializedVersion: 12\n  mipmaps:\n    enableMipMap: 0\n  isReadable: 0\n  filterMode: 0\n  textureSettings:\n    serializedVersion: 2\n    filterMode: 0\n    aniso: 1\n    mipBias: 0\n    wrapU: 1\n    wrapV: 1\n  textureType: 8\n  spriteMode: 2\n  spritePixelsToUnits: 100\n  alphaIsTransparency: 1\n  textureCompression: 0\n  spriteSheet:\n    serializedVersion: 2\n    sprites:\n",
        hash.rotate_left(29)
    );
    for (i, f) in meta["frames"].as_array().unwrap().iter().enumerate() {
        let r = &f["frame"];
        s += &format!(
            "    - serializedVersion: 2\n      name: frame_{i}\n      rect:\n        serializedVersion: 2\n        x: {}\n        y: {}\n        width: {}\n        height: {}\n      alignment: 0\n      pivot: {{x: 0.5, y: 0.5}}\n      border: {{x: 0, y: 0, z: 0, w: 0}}\n      internalID: {}\n",
            r["x"],
            height as u64 - r["y"].as_u64().unwrap() - r["h"].as_u64().unwrap(),
            r["w"],
            r["h"],
            21300000 + i * 2
        );
    }
    s += "    nameFileIdTable:\n";
    for (i, _) in meta["frames"].as_array().unwrap().iter().enumerate() {
        s += &format!("      frame_{i}: {}\n", 21300000 + i * 2);
    }
    s
}
pub fn load_ora(path: &Path) -> Result<Document, String> {
    let root = std::env::temp_dir().join(format!(
        "pixelcraft-ora-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let result = (|| {
        crate::extensions::unzip(&std::fs::read(path).map_err(|e| e.to_string())?, &root)?;
        let xml = std::fs::read_to_string(root.join("stack.xml")).map_err(|e| e.to_string())?;
        let tree = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
        let image = tree.root_element();
        let w = image
            .attribute("w")
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let h = image
            .attribute("h")
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let count = image
            .descendants()
            .filter(|n| n.has_tag_name("layer") || n.has_tag_name("stack"))
            .count();
        if w == 0
            || h == 0
            || w > MAX_DIMENSION
            || h > MAX_DIMENSION
            || w.saturating_mul(h).saturating_mul(count) > MAX_PIXELS
        {
            return Err("OpenRaster exceeds limits".into());
        }
        let mut d = Document::new(w, h);
        d.layers.clear();
        fn visit(
            node: roxmltree::Node,
            parent: Option<usize>,
            root: &Path,
            d: &mut Document,
        ) -> Result<(), String> {
            for n in node.children().filter(|n| n.is_element()).rev() {
                if !n.has_tag_name("layer") && !n.has_tag_name("stack") {
                    continue;
                }
                let mut l = Layer::new(
                    n.attribute("name").unwrap_or("Layer").into(),
                    d.width,
                    d.height,
                    1,
                );
                l.parent = parent;
                l.group = n.has_tag_name("stack");
                l.visible = n.attribute("visibility") != Some("hidden");
                l.opacity = (n
                    .attribute("opacity")
                    .unwrap_or("1")
                    .parse::<f32>()
                    .map_err(|e| e.to_string())?
                    .clamp(0., 1.)
                    * 255.)
                    .round() as u8;
                l.blend = match n.attribute("composite-op").unwrap_or("svg:src-over") {
                    "svg:multiply" => BlendMode::Multiply,
                    "svg:screen" => BlendMode::Screen,
                    "svg:overlay" => BlendMode::Overlay,
                    "svg:darken" => BlendMode::Darken,
                    "svg:lighten" => BlendMode::Lighten,
                    "svg:src-over" => BlendMode::Normal,
                    v => return Err(format!("Unsupported OpenRaster blend: {v}")),
                };
                let index = d.layers.len();
                if !l.group {
                    let src = n.attribute("src").ok_or("Missing OpenRaster source")?;
                    if Path::new(src)
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err("Unsafe OpenRaster source".into());
                    }
                    let im = image::open(root.join(src))
                        .map_err(|e| e.to_string())?
                        .to_rgba8();
                    let ox = n
                        .attribute("x")
                        .unwrap_or("0")
                        .parse::<i32>()
                        .map_err(|e| e.to_string())?;
                    let oy = n
                        .attribute("y")
                        .unwrap_or("0")
                        .parse::<i32>()
                        .map_err(|e| e.to_string())?;
                    for (x, y, p) in im.enumerate_pixels() {
                        let (x, y) = (x as i32 + ox, y as i32 + oy);
                        if let Some(i) = d.index(x, y) {
                            l.cels[0][i] = p.0
                        } else {
                            l.cel_meta[0]
                                .outside
                                .push(OutsidePixel { x, y, color: p.0 });
                        }
                    }
                }
                d.layers.push(l);
                if n.has_tag_name("stack") {
                    visit(n, Some(index), root, d)?;
                }
            }
            Ok(())
        }
        let stack = image
            .children()
            .find(|n| n.has_tag_name("stack"))
            .ok_or("Missing OpenRaster stack")?;
        visit(stack, None, &root, &mut d)?;
        d.validate()?;
        Ok(d)
    })();
    let _ = std::fs::remove_dir_all(root);
    result
}

pub fn load_webp(path: &Path) -> Result<Document, String> {
    // libwebp's animation decoder returns complete composited canvases, including disposal.
    #[repr(C)]
    struct Data {
        bytes: *const u8,
        size: usize,
    }
    #[repr(C)]
    struct Info {
        width: u32,
        height: u32,
        loop_count: u32,
        bgcolor: u32,
        frame_count: u32,
        pad: [u32; 4],
    }
    unsafe {
        let mut library = None;
        for name in [
            "libwebpdemux.so.2",
            "libwebpdemux.so",
            "libwebpdemux.dylib",
            "libwebpdemux.dll",
        ] {
            if let Ok(l) = libloading::Library::new(name) {
                library = Some(l);
                break;
            }
        }
        let lib = library.ok_or("WebP import requires the libwebpdemux runtime on this system")?;
        let new: libloading::Symbol<
            unsafe extern "C" fn(
                *const Data,
                *const std::ffi::c_void,
                i32,
            ) -> *mut std::ffi::c_void,
        > = lib
            .get(b"WebPAnimDecoderNewInternal")
            .map_err(|e| e.to_string())?;
        let info: libloading::Symbol<
            unsafe extern "C" fn(*const std::ffi::c_void, *mut Info) -> i32,
        > = lib
            .get(b"WebPAnimDecoderGetInfo")
            .map_err(|e| e.to_string())?;
        let next: libloading::Symbol<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut *mut u8, *mut i32) -> i32,
        > = lib
            .get(b"WebPAnimDecoderGetNext")
            .map_err(|e| e.to_string())?;
        let delete: libloading::Symbol<unsafe extern "C" fn(*mut std::ffi::c_void)> = lib
            .get(b"WebPAnimDecoderDelete")
            .map_err(|e| e.to_string())?;
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let dec = new(
            &Data {
                bytes: bytes.as_ptr(),
                size: bytes.len(),
            },
            std::ptr::null(),
            0x0107,
        );
        if dec.is_null() {
            return Err("Invalid WebP".into());
        }
        let result = (|| {
            let mut inf = Info {
                width: 0,
                height: 0,
                loop_count: 0,
                bgcolor: 0,
                frame_count: 0,
                pad: [0; 4],
            };
            if info(dec, &mut inf) == 0
                || inf.width == 0
                || inf.height == 0
                || inf.width as usize > MAX_DIMENSION
                || inf.height as usize > MAX_DIMENSION
                || (inf.width as usize)
                    .saturating_mul(inf.height as usize)
                    .saturating_mul(inf.frame_count as usize)
                    > MAX_PIXELS
                || inf.frame_count > 65535
            {
                return Err("WebP exceeds limits".into());
            }
            let mut d = Document::new(inf.width as usize, inf.height as usize);
            let mut last = 0;
            for f in 0..inf.frame_count as usize {
                let mut ptr = std::ptr::null_mut();
                let mut stamp = 0;
                if next(dec, &mut ptr, &mut stamp) == 0 {
                    return Err("Invalid WebP frame".into());
                }
                if f > 0 {
                    d.add_frame(f - 1, false)
                }
                let pixels = std::slice::from_raw_parts(ptr, d.width * d.height * 4);
                d.layers[0].cels[f] = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| [p[0], p[1], p[2], p[3]])
                    .collect();
                d.durations[f] = (stamp - last).clamp(1, 65535) as u16;
                last = stamp;
            }
            d.validate()?;
            Ok(d)
        })();
        delete(dec);
        result
    }
}

pub fn export_tiled(path: &Path, d: &Document, f: usize) -> Result<(), String> {
    if d.tilesets.is_empty() {
        return Err("Import a tileset first".into());
    }
    let first = &d.tilesets[0];
    if d.tilesets
        .iter()
        .any(|t| t.width != first.width || t.height != first.height)
    {
        return Err("Tiled map export requires matching tile sizes".into());
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let mut sets = vec![];
    let mut gid = 1u32;
    let mut offsets = vec![];
    for (i, t) in d.tilesets.iter().enumerate() {
        let columns = 16.min(t.tiles.len()).max(1);
        let rows = t.tiles.len().div_ceil(columns);
        if t.width
            .saturating_mul(columns)
            .saturating_mul(t.height)
            .saturating_mul(rows)
            > MAX_PIXELS
        {
            return Err("Tileset exceeds image limits".into());
        }
        let mut im = image::RgbaImage::new((columns * t.width) as u32, (rows * t.height) as u32);
        for (n, tile) in t.tiles.iter().enumerate() {
            for y in 0..t.height {
                for x in 0..t.width {
                    im.put_pixel(
                        ((n % columns) * t.width + x) as u32,
                        ((n / columns) * t.height + y) as u32,
                        image::Rgba(tile[y * t.width + x]),
                    );
                }
            }
        }
        let png = format!("{stem}-tiles-{i}.png");
        let tsj = format!("{stem}-tiles-{i}.tsj");
        im.save(path.with_file_name(&png))
            .map_err(|e| e.to_string())?;
        write_json(
            &path.with_file_name(&tsj),
            &json!({"type":"tileset","version":"1.10","name":t.name,"tilewidth":t.width,"tileheight":t.height,"tilecount":t.tiles.len(),"columns":columns,"image":png,"imagewidth":im.width(),"imageheight":im.height(),"margin":0,"spacing":0}),
        )?;
        sets.push(json!({"firstgid":gid,"source":tsj}));
        offsets.push(gid);
        gid += t.tiles.len() as u32;
    }
    let mut layers = vec![];
    for (i, l) in d.layers.iter().enumerate() {
        if let Some(ti) = l.tileset {
            let t = &d.tilesets[ti];
            let g = l.cel_meta[f].tilemap.as_ref();
            let columns = g.map_or(d.width.div_ceil(t.width), |g| g.columns);
            let rows = g.map_or(d.height.div_ceil(t.height), |g| g.rows);
            let data: Vec<u32> = l.tile_cels[f]
                .iter()
                .map(|id| {
                    let n = id & 0x1fffffff;
                    if n == 0 {
                        0
                    } else {
                        (id & 0xe0000000) | (n - 1 + offsets[ti])
                    }
                })
                .collect();
            layers.push(json!({"id":i+1,"type":"tilelayer","name":l.name,"width":columns,"height":rows,"x":0,"y":0,"offsetx":g.map_or(0,|g|g.x),"offsety":g.map_or(0,|g|g.y),"visible":l.visible,"opacity":l.opacity as f32/255.,"data":data}));
        }
    }
    if layers.is_empty() {
        return Err("No tile layers to export".into());
    }
    write_json(
        path,
        &json!({"type":"map","version":"1.10","tiledversion":"1.10","orientation":"orthogonal","renderorder":"right-down","infinite":false,"width":d.width.div_ceil(first.width),"height":d.height.div_ceil(first.height),"tilewidth":first.width,"tileheight":first.height,"layers":layers,"tilesets":sets,"nextlayerid":d.layers.len()+1,"nextobjectid":1}),
    )
}
pub fn load_tiled(path: &Path) -> Result<Document, String> {
    let v: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let num = |v: &Value, k: &str| {
        v[k].as_u64()
            .map(|x| x as usize)
            .ok_or_else(|| format!("Missing Tiled {k}"))
    };
    let base = path.parent().unwrap_or(Path::new("."));
    fn tileset(v: &Value, base: &Path) -> Result<Tileset, String> {
        let mut v = v.clone();
        let mut base = base.to_path_buf();
        if let Some(source) = v["source"].as_str() {
            let p = base.join(source);
            v = serde_json::from_slice(&std::fs::read(&p).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            base = p.parent().unwrap().into();
        }
        let n = |k: &str| {
            v[k].as_u64()
                .map(|x| x as usize)
                .ok_or_else(|| format!("Missing tileset {k}"))
        };
        let (w, h, count) = (n("tilewidth")?, n("tileheight")?, n("tilecount")?);
        if w == 0 || h == 0 || count == 0 || w.saturating_mul(h).saturating_mul(count) > MAX_PIXELS
        {
            return Err("Tileset exceeds limits".into());
        }
        let columns = n("columns")?;
        if columns == 0 {
            return Err("Image collection tilesets are unsupported".into());
        }
        let im = image::open(base.join(v["image"].as_str().ok_or("Missing tileset image")?))
            .map_err(|e| e.to_string())?
            .to_rgba8();
        let margin = v["margin"].as_u64().unwrap_or(0) as usize;
        let spacing = v["spacing"].as_u64().unwrap_or(0) as usize;
        let mut tiles = vec![];
        for i in 0..count {
            let (x, y) = (
                margin + i % columns * (w + spacing),
                margin + i / columns * (h + spacing),
            );
            if x + w > im.width() as usize || y + h > im.height() as usize {
                return Err("Tile outside image".into());
            }
            tiles.push(
                (0..w * h)
                    .map(|p| im.get_pixel((x + p % w) as u32, (y + p / w) as u32).0)
                    .collect(),
            );
        }
        Ok(Tileset {
            name: v["name"].as_str().unwrap_or("Tileset").into(),
            width: w,
            height: h,
            tiles,
        })
    }
    if v["type"] == "tileset" {
        let t = tileset(&v, base)?;
        let mut d = Document::new(t.width, t.height);
        d.tilesets.push(t);
        d.add_tile_layer(0)?;
        d.remove_layer(0);
        d.validate()?;
        return Ok(d);
    }
    if v["orientation"] != "orthogonal" || v["infinite"] == true {
        return Err("Tiled import supports finite orthogonal JSON maps".into());
    }
    let (tw, th) = (num(&v, "tilewidth")?, num(&v, "tileheight")?);
    let (w, h) = (
        num(&v, "width")?.saturating_mul(tw),
        num(&v, "height")?.saturating_mul(th),
    );
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w.saturating_mul(h) > MAX_PIXELS
    {
        return Err("Tiled map exceeds limits".into());
    }
    let mut d = Document::new(w, h);
    let sets = v["tilesets"].as_array().ok_or("Missing tilesets")?;
    let mut gids = vec![];
    for s in sets {
        let t = tileset(s, base)?;
        if t.width != tw || t.height != th {
            return Err("Mixed tile sizes are unsupported".into());
        }
        d.tilesets.push(t);
        gids.push(num(s, "firstgid")? as u32);
    }
    for l in v["layers"].as_array().ok_or("Missing Tiled layers")? {
        if l["type"] != "tilelayer" {
            return Err("Tiled import supports tile layers only".into());
        }
        let data = l["data"]
            .as_array()
            .ok_or("Use uncompressed JSON tile arrays")?;
        let (cols, rows) = (num(l, "width")?, num(l, "height")?);
        if cols == 0
            || rows == 0
            || cols.saturating_mul(rows) != data.len()
            || cols.saturating_mul(rows) > MAX_PIXELS
        {
            return Err("Invalid tile layer dimensions".into());
        }
        for (ti, &first) in gids.iter().enumerate() {
            let end = first + d.tilesets[ti].tiles.len() as u32;
            let entries: Result<Vec<u32>, String> =
                data.iter()
                    .map(|n| {
                        let id = n
                            .as_u64()
                            .filter(|v| *v <= u32::MAX as u64)
                            .ok_or("Invalid tile ID")? as u32;
                        let gid = id & 0x1fffffff;
                        if gid != 0
                            && !gids.iter().enumerate().any(|(i, g)| {
                                gid >= *g && gid < *g + d.tilesets[i].tiles.len() as u32
                            })
                        {
                            return Err("Unknown tile ID".into());
                        }
                        Ok(if gid >= first && gid < end {
                            (id & 0xe0000000) | (gid - first + 1)
                        } else {
                            0
                        })
                    })
                    .collect();
            let entries = entries?;
            if !entries.iter().any(|v| *v != 0) && ti > 0 {
                continue;
            }
            d.add_tile_layer(ti)?;
            let layer = d.layers.last_mut().unwrap();
            layer.name = l["name"].as_str().unwrap_or("Tile layer").into();
            layer.visible = l["visible"].as_bool().unwrap_or(true);
            layer.opacity =
                (l["opacity"].as_f64().unwrap_or(1.).clamp(0., 1.) * 255.).round() as u8;
            layer.tile_cels[0] = entries;
            layer.cel_meta[0].tilemap = Some(TilemapGeometry {
                x: (l["x"].as_i64().unwrap_or(0) * tw as i64 + l["offsetx"].as_i64().unwrap_or(0))
                    as i32,
                y: (l["y"].as_i64().unwrap_or(0) * th as i64 + l["offsety"].as_i64().unwrap_or(0))
                    as i32,
                columns: cols,
                rows,
            });
        }
    }
    if d.layers.len() > 1 {
        d.remove_layer(0)
    }
    d.validate()?;
    Ok(d)
}
