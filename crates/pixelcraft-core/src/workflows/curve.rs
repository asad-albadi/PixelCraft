use crate::model::{
    BrushShape, Document, MAX_BRUSH_SIZE, MAX_DIMENSION, MAX_PIXELS, Pixels, WrapAxes,
};

type PixelPoint = (i32, i32);
type Point = [f64; 2];

fn interpolate(a: Point, b: Point, t: f64) -> Point {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn distance_sq(a: Point, b: Point) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

fn pixel(point: Point) -> PixelPoint {
    (point[0].round() as i32, point[1].round() as i32)
}

fn flatten([a, b, c, d]: [Point; 4], path: &mut Vec<PixelPoint>) {
    // Comparing the controls with a straight cubic also catches collinear curves
    // that double back outside the interval between their endpoints.
    if distance_sq(b, interpolate(a, d, 1. / 3.)) <= 0.0625
        && distance_sq(c, interpolate(a, d, 2. / 3.)) <= 0.0625
    {
        let next = pixel(d);
        if path.last() != Some(&next) {
            path.push(next);
        }
    } else {
        let ab = interpolate(a, b, 0.5);
        let bc = interpolate(b, c, 0.5);
        let cd = interpolate(c, d, 0.5);
        let left = interpolate(ab, bc, 0.5);
        let right = interpolate(bc, cd, 0.5);
        let mid = interpolate(left, right, 0.5);
        flatten([a, ab, left, mid], path);
        flatten([mid, right, cd, d], path);
    }
}

/// Pixel centers for a cubic Bézier in start, first control, second control, end order.
pub fn cubic_polyline(points: [PixelPoint; 4]) -> Result<Vec<PixelPoint>, String> {
    if points
        .iter()
        .any(|&(x, y)| x.unsigned_abs().max(y.unsigned_abs()) as usize > 2 * MAX_DIMENSION)
    {
        return Err("Curve coordinates exceed the drawing limits".into());
    }
    let mut path = vec![points[0]];
    flatten(points.map(|(x, y)| [f64::from(x), f64::from(y)]), &mut path);
    Ok(path)
}

/// Apply the native brush footprint along the flattened curve.
pub fn cubic_coverage(
    (width, height): (usize, usize),
    points: [PixelPoint; 4],
    (size, shape): (i32, BrushShape),
    axes: WrapAxes,
) -> Result<Pixels, String> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("Invalid curve canvas dimensions".into());
    }
    let size = size.clamp(1, MAX_BRUSH_SIZE);
    let path = cubic_polyline(points)?;
    let stamps: u64 = path
        .windows(2)
        .map(|pair| {
            let [(ax, ay), (bx, by)] = [pair[0], pair[1]];
            u64::from(ax.abs_diff(bx).max(ay.abs_diff(by))) + 1
        })
        .sum();
    if stamps * u64::from(size as u32).pow(2) > MAX_PIXELS as u64 {
        return Err("Curve exceeds the drawing work limit".into());
    }
    let mut brush = Document::new(width, height);
    brush.brush_shape = shape;
    let edges = path.windows(2).map(|pair| (pair[0], pair[1]));
    if path.len() == 1 {
        brush.line_on_axes((0, 0), path[0], path[0], [255; 4], size, axes);
    } else {
        for (from, to) in edges {
            brush.line_on_axes((0, 0), from, to, [255; 4], size, axes);
        }
    }
    Ok(std::mem::replace(
        &mut brush.layers[0].cels[0],
        Pixels::empty(width * height),
    ))
}
