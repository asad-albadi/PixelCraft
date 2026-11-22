use crate::model::Pixel;
use std::collections::HashMap;

const ENLARGEMENT: usize = 8;
const PATCH: usize = 192;
const OVERLAP: usize = 2;
const MAX_ENLARGED_PIXELS: usize = 4 * 1024 * 1024;

pub(super) fn enlargement_fits(width: usize, height: usize) -> bool {
    width.saturating_mul(height) <= MAX_ENLARGED_PIXELS / ENLARGEMENT.pow(2)
}

type Sample = (Pixel, Option<u8>);

pub(super) struct Enlarged {
    width: usize,
    height: usize,
    ids: Vec<u32>,
    samples: Vec<Sample>,
}

impl Enlarged {
    pub(super) fn new(
        mut width: usize,
        mut height: usize,
        source: impl Fn(usize, usize) -> Option<Sample>,
    ) -> Result<Self, String> {
        if !enlargement_fits(width, height) {
            return Err("Selection is too large for RotSprite. Choose a smaller selection or use Fast rotation.".into());
        }
        // Intern both the color and its indexed slot. ID zero represents a
        // pixel outside the selection, including a clipped part of the source.
        let mut keys = HashMap::new();
        let mut samples = Vec::new();
        let mut ids = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let id = source(x, y).map_or(0, |sample| {
                    *keys.entry(sample).or_insert_with(|| {
                        samples.push(sample);
                        samples.len() as u32
                    })
                });
                ids.push(id);
            }
        }
        for _ in 0..ENLARGEMENT.ilog2() {
            ids = scale2x(&ids, width, height);
            width *= 2;
            height *= 2;
        }
        Ok(Self {
            width,
            height,
            ids,
            samples,
        })
    }

    pub(super) fn sample(&self, x: f64, y: f64) -> Option<Sample> {
        let x = super::snap_near_integer(x * ENLARGEMENT as f64).floor();
        let y = super::snap_near_integer(y * ENLARGEMENT as f64).floor();
        if !(0. ..self.width as f64).contains(&x) || !(0. ..self.height as f64).contains(&y) {
            return None;
        }
        self.sample_enlarged(x as usize, y as usize)
    }

    fn sample_enlarged(&self, x: usize, y: usize) -> Option<Sample> {
        let id = self.ids[y * self.width + x];
        id.checked_sub(1).map(|index| self.samples[index as usize])
    }
}

/// Include the neighbors needed by three Scale2x passes at every patch edge.
fn enlarge_patch(
    old: [usize; 2],
    [left, top, right, bottom]: [usize; 4],
    source: &impl Fn(usize, usize) -> Option<Sample>,
) -> Result<(Enlarged, [usize; 2]), String> {
    let ox = left.saturating_sub(OVERLAP);
    let oy = top.saturating_sub(OVERLAP);
    let width = (right + OVERLAP).min(old[0]) - ox;
    let height = (bottom + OVERLAP).min(old[1]) - oy;
    let enlarged = Enlarged::new(width, height, |x, y| source(x + ox, y + oy))?;
    Ok((enlarged, [ox, oy]))
}

pub(super) fn rotate(
    old: [usize; 2],
    clip: [i32; 4],
    forward: impl Fn([f64; 2]) -> [f64; 2],
    inverse: impl Fn([i32; 2]) -> [f64; 2],
    source: impl Fn(usize, usize) -> Option<Sample>,
    mut write: impl FnMut([i32; 2], Sample),
) -> Result<(), String> {
    for top in (0..old[1]).step_by(PATCH) {
        let bottom = (top + PATCH).min(old[1]);
        for left in (0..old[0]).step_by(PATCH) {
            let right = (left + PATCH).min(old[0]);
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for point in [[left, top], [right, top], [left, bottom], [right, bottom]] {
                let transformed = forward(point.map(|value| value as f64));
                for axis in 0..2 {
                    min[axis] = min[axis].min(transformed[axis]);
                    max[axis] = max[axis].max(transformed[axis]);
                }
            }
            let x0 = (min[0].floor() as i32).max(clip[0]);
            let y0 = (min[1].floor() as i32).max(clip[1]);
            let x1 = (max[0].ceil() as i32).min(clip[2]);
            let y1 = (max[1].ceil() as i32).min(clip[3]);
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            let (patch, [ox, oy]) = enlarge_patch(old, [left, top, right, bottom], &source)?;
            for y in y0..y1 {
                for x in x0..x1 {
                    let [sx, sy] = inverse([x, y]);
                    if (left as f64..right as f64).contains(&sx)
                        && (top as f64..bottom as f64).contains(&sy)
                        && let Some(sample) = patch.sample(sx - ox as f64, sy - oy as f64)
                    {
                        write([x, y], sample);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn resample(
    old: [usize; 2],
    new: [usize; 2],
    source: impl Fn(usize, usize) -> Option<Sample>,
    mut write: impl FnMut(usize, Sample),
) -> Result<(), String> {
    let [sx, sy]: [Vec<usize>; 2] = std::array::from_fn(|axis| {
        (0..new[axis])
            .map(|position| (2 * position + 1) * old[axis] * ENLARGEMENT / (2 * new[axis]))
            .collect()
    });
    for top in (0..old[1]).step_by(PATCH) {
        let bottom = (top + PATCH).min(old[1]);
        let y0 = sy.partition_point(|&y| y < top * ENLARGEMENT);
        let y1 = sy.partition_point(|&y| y < bottom * ENLARGEMENT);
        if y0 == y1 {
            continue;
        }
        for left in (0..old[0]).step_by(PATCH) {
            let right = (left + PATCH).min(old[0]);
            let x0 = sx.partition_point(|&x| x < left * ENLARGEMENT);
            let x1 = sx.partition_point(|&x| x < right * ENLARGEMENT);
            if x0 == x1 {
                continue;
            }
            let (patch, [ox, oy]) = enlarge_patch(old, [left, top, right, bottom], &source)?;
            for (dy, &y) in sy[y0..y1].iter().enumerate() {
                for (dx, &x) in sx[x0..x1].iter().enumerate() {
                    if let Some(sample) =
                        patch.sample_enlarged(x - ox * ENLARGEMENT, y - oy * ENLARGEMENT)
                    {
                        write((y0 + dy) * new[0] + x0 + dx, sample);
                    }
                }
            }
        }
    }
    Ok(())
}

// Scale2x chooses a matching pair across each corner at a color boundary.
// See https://www.scale2x.it/algorithm for the corner and border rules.
fn scale2x(source: &[u32], width: usize, height: usize) -> Vec<u32> {
    let stride = width * 2;
    let mut output = vec![0; source.len() * 4];
    for y in 0..height {
        let above = y.saturating_sub(1) * width;
        let row = y * width;
        let below = (y + 1).min(height - 1) * width;
        for x in 0..width {
            let center = source[row + x];
            let top = source[above + x];
            let bottom = source[below + x];
            let left = source[row + x.saturating_sub(1)];
            let right = source[row + (x + 1).min(width - 1)];
            let corner = |vertical, horizontal| {
                if top != bottom && left != right && vertical == horizontal {
                    horizontal
                } else {
                    center
                }
            };
            let at = y * 2 * stride + x * 2;
            output[at..at + 2].copy_from_slice(&[corner(top, left), corner(top, right)]);
            output[at + stride..at + stride + 2]
                .copy_from_slice(&[corner(bottom, left), corner(bottom, right)]);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_patches_match_a_full_enlargement_across_seams() {
        let old = [197, 198];
        let width = 250usize;
        let height = 250usize;
        let (sin, cos) = 21.0f64.to_radians().sin_cos();
        let forward = |[x, y]: [f64; 2]| [40.1 + x * cos - y * sin, y * cos + x * sin - 5.6];
        let inverse = |[x, y]: [i32; 2]| {
            let (x, y) = (f64::from(x) + 0.5 - 40.1, f64::from(y) + 0.5 + 5.6);
            [x * cos + y * sin, y * cos - x * sin]
        };
        let ink = [90, 40, 150, 255];
        let source = |x: usize, y: usize| match (x + y) % 9 {
            0..=2 => Some((ink, Some(1))),
            3..=5 => Some((ink, Some(2))),
            _ => None,
        };
        let full = Enlarged::new(old[0], old[1], source).unwrap();
        let mut actual = vec![None; width * height];
        rotate(
            old,
            [0, 0, width as i32, height as i32],
            forward,
            inverse,
            source,
            |[x, y], sample| {
                let previous = actual[y as usize * width + x as usize].replace(sample);
                assert!(previous.is_none());
            },
        )
        .unwrap();
        for (index, actual) in actual.into_iter().enumerate() {
            let [x, y] = inverse([(index % width) as i32, (index / width) as i32]);
            assert_eq!(actual, full.sample(x, y), "target index {index}");
        }
    }

    #[test]
    fn patch_resampling_matches_one_full_enlargement_at_both_seams() {
        let old = [197, 198];
        let new = [269, 207];
        let ink = [31, 71, 211, 255];
        let source = |x: usize, y: usize| match (x + y) % 9 {
            0..=2 => Some((ink, Some(1))),
            3..=5 => Some((ink, Some(2))),
            _ => None,
        };
        let full = Enlarged::new(old[0], old[1], source).unwrap();
        let mut actual = vec![None; new[0] * new[1]];
        resample(old, new, source, |index, sample| {
            actual[index] = Some(sample)
        })
        .unwrap();
        for (index, actual) in actual.into_iter().enumerate() {
            let x = ((2 * (index % new[0]) + 1) * old[0] * ENLARGEMENT) / (2 * new[0]);
            let y = ((2 * (index / new[0]) + 1) * old[1] * ENLARGEMENT) / (2 * new[1]);
            assert_eq!(actual, full.sample_enlarged(x, y), "target index {index}");
        }
    }
}
