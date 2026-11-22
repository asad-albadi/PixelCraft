use crate::{model::Pixel, storage::mcp_io};
use image::RgbaImage;
use std::{
    collections::BTreeMap,
    io::{self, BufWriter, Write},
    path::Path,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum VectorFormat {
    Svg,
    Css,
}

pub(super) fn write(
    path: &Path,
    image: &RgbaImage,
    scale: usize,
    format: VectorFormat,
) -> Result<(), String> {
    let mut output = BufWriter::new(mcp_io::create(path).map_err(io_text)?);
    let (width, height) = (
        image.width() as usize * scale,
        image.height() as usize * scale,
    );
    match format {
        VectorFormat::Svg => {
            writeln!(output, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" shape-rendering="crispEdges">"#).map_err(io_text)?;
            visit_rectangles(image, |rect| {
                let [x, y, w, h] = rect.bounds.map(|value| value * scale);
                let [r, g, b, a] = rect.color;
                write!(output, r##"  <rect x="{x}" y="{y}" width="{w}" height="{h}" fill="#{r:02x}{g:02x}{b:02x}""##)?;
                if a != 255 {
                    write!(output, r#" fill-opacity="{:.8}""#, f64::from(a) / 255.0)?;
                }
                writeln!(output, " />")
            }).map_err(io_text)?;
            writeln!(output, "</svg>").map_err(io_text)?;
        }
        VectorFormat::Css => {
            writeln!(output, ".pixel-art {{\n  display: inline-block;\n  width: {width}px;\n  height: {height}px;").map_err(io_text)?;
            let mut first = true;
            visit_rectangles(image, |rect| {
                if first {
                    write!(output, "  background:\n    ")?;
                    first = false;
                } else {
                    write!(output, ",\n    ")?;
                }
                let [x, y, w, h] = rect.bounds.map(|value| value * scale);
                let [r, g, b, a] = rect.color;
                let color = format!("#{r:02x}{g:02x}{b:02x}{a:02x}");
                write!(
                    output,
                    "linear-gradient({color}, {color}) {x}px {y}px / {w}px {h}px no-repeat"
                )
            })
            .map_err(io_text)?;
            if first {
                write!(output, "  background: transparent").map_err(io_text)?;
            }
            writeln!(output, ";\n}}").map_err(io_text)?;
        }
    }
    output.flush().map_err(io_text)
}

fn io_text(error: io::Error) -> String {
    error.to_string()
}

struct Rectangle {
    bounds: [usize; 4],
    color: Pixel,
}

/// Coalesce matching runs on adjacent rows. Keep only the active row's rectangles.
fn visit_rectangles(
    image: &RgbaImage,
    mut emit: impl FnMut(Rectangle) -> io::Result<()>,
) -> io::Result<()> {
    // (start column, run length, RGBA) -> top row.
    let mut active = BTreeMap::new();
    let stride = image.width() as usize * 4;
    for (y, row) in image.as_raw().chunks_exact(stride).enumerate() {
        let mut next = BTreeMap::new();
        let mut x = 0;
        let row_width = image.width() as usize;
        while x < row_width {
            let color: Pixel = row[x * 4..x * 4 + 4].try_into().unwrap();
            let width = row[x * 4..]
                .chunks_exact(4)
                .take_while(|&p| p == color)
                .count();
            if color[3] != 0 {
                let key = (x, width, color);
                next.insert(key, active.remove(&key).unwrap_or(y));
            }
            x += width;
        }
        for ((x, width, color), top) in active {
            emit(Rectangle {
                bounds: [x, top, width, y - top],
                color,
            })?;
        }
        active = next;
    }
    for ((x, width, color), top) in active {
        emit(Rectangle {
            bounds: [x, top, width, image.height() as usize - top],
            color,
        })?;
    }
    Ok(())
}
