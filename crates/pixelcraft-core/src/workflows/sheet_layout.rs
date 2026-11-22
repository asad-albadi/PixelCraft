use super::{SheetLayout, SheetOptions, SheetSpacing, scaled_image_dimensions};
use crate::{
    atlas::pack_rect_positions,
    model::{MAX_DIMENSION, MAX_PIXELS},
};

pub(super) struct SheetPlacement {
    pub positions: Vec<[usize; 2]>,
    pub dimensions: [usize; 2],
}

#[doc(hidden)]
pub fn validate_sheet_dimensions(options: &SheetOptions) -> Result<(), String> {
    scaled_image_dimensions(
        options.width.unwrap_or(1),
        options.height.unwrap_or(1),
        options.scale,
    )
    .and_then(|_| {
        if options.width == Some(0) || options.height == Some(0) {
            Err("Explicit sheet dimensions must be positive".into())
        } else {
            Ok(())
        }
    })
}

pub(crate) fn uniform_sheet_dimensions(
    size: [usize; 2],
    count: usize,
    options: &SheetOptions,
    spacing: SheetSpacing,
) -> Result<[usize; 2], String> {
    let [width, height] = place_samples(&vec![size; count], options, spacing)?.dimensions;
    Ok([width * options.scale, height * options.scale])
}

pub(crate) fn uniform_sheet_area(
    size: [usize; 2],
    count: usize,
    options: &SheetOptions,
    spacing: SheetSpacing,
) -> Result<usize, String> {
    validate_sheet_dimensions(options)?;
    if options.trims()
        && options.layout == SheetLayout::Rows
        && options.width.is_none()
        && options.height.is_none()
    {
        let columns = options.columns.clamp(1, count);
        return Ok(spacing
            .extent(size[0], columns)
            .saturating_mul(spacing.extent(size[1], count.div_ceil(columns)))
            .saturating_mul(options.scale.saturating_mul(options.scale)));
    }
    match uniform_sheet_dimensions(size, count, options, spacing) {
        Ok([width, height]) => Ok(width * height),
        Err(_) if options.trims() => Ok(MAX_PIXELS),
        Err(error) => Err(error),
    }
}

pub(super) fn place_samples(
    sizes: &[[usize; 2]],
    options: &SheetOptions,
    spacing: SheetSpacing,
) -> Result<SheetPlacement, String> {
    validate_sheet_dimensions(options)?;
    let inset = spacing.inset();
    let padded: Vec<_> = sizes
        .iter()
        .map(|size| size.map(|axis| axis + 2 * inset))
        .collect();
    let fixed = [options.width, options.height];
    let positions = match options.layout {
        SheetLayout::Rows => bands(&padded, fixed, spacing, options.columns.max(1), false, true)?,
        SheetLayout::Columns => bands(&padded, fixed, spacing, options.rows.max(1), true, true)?,
        SheetLayout::Horizontal => bands(&padded, fixed, spacing, usize::MAX, false, false)?,
        SheetLayout::Vertical => bands(&padded, fixed, spacing, usize::MAX, true, false)?,
        SheetLayout::Packed => packed(&padded, fixed, spacing, MAX_DIMENSION / options.scale)?,
    };

    let mut dimensions = [0, 0];
    for (&position, &size) in positions.iter().zip(&padded) {
        for axis in 0..2 {
            dimensions[axis] = dimensions[axis].max(position[axis] + size[axis] + spacing.border);
        }
    }
    for axis in 0..2 {
        if let Some(fixed) = fixed[axis] {
            if dimensions[axis] > fixed {
                return Err("Samples do not fit in the requested sheet dimensions".into());
            }
            dimensions[axis] = fixed;
        }
    }
    if dimensions[0] > MAX_DIMENSION
        || dimensions[1] > MAX_DIMENSION
        || dimensions[0].saturating_mul(dimensions[1]) > MAX_PIXELS
    {
        return Err("Sprite sheet exceeds image limits; use more rows or a tag".into());
    }
    scaled_image_dimensions(dimensions[0], dimensions[1], options.scale)?;
    Ok(SheetPlacement {
        dimensions,
        positions: positions
            .into_iter()
            .map(|p| p.map(|axis| axis + inset))
            .collect(),
    })
}

fn bands(
    sizes: &[[usize; 2]],
    fixed: [Option<usize>; 2],
    spacing: SheetSpacing,
    per_band: usize,
    transpose: bool,
    wrap: bool,
) -> Result<Vec<[usize; 2]>, String> {
    let [width, height] = if transpose {
        [fixed[1], fixed[0]]
    } else {
        fixed
    };
    let mut positions = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut across, mut thickness) = (spacing.border, spacing.border, 0, 0);
    for &size in sizes {
        let [w, h] = if transpose { [size[1], size[0]] } else { size };
        if wrap
            && across > 0
            && (across == per_band || width.is_some_and(|width| x + w + spacing.border > width))
        {
            x = spacing.border;
            y += thickness + spacing.gap;
            across = 0;
            thickness = 0;
        }
        if width.is_some_and(|width| x + w + spacing.border > width)
            || height.is_some_and(|height| y + h + spacing.border > height)
        {
            return Err("Samples do not fit in the requested sheet dimensions".into());
        }
        positions.push(if transpose { [y, x] } else { [x, y] });
        x += w + spacing.gap;
        across += 1;
        thickness = thickness.max(h);
    }
    Ok(positions)
}

fn packed(
    sizes: &[[usize; 2]],
    fixed: [Option<usize>; 2],
    spacing: SheetSpacing,
    limit: usize,
) -> Result<Vec<[usize; 2]>, String> {
    let [first, ..] = sizes else {
        return Err("No sheet samples to pack".into());
    };
    let reserved: Vec<_> = sizes
        .iter()
        .map(|&[w, h]| (w + spacing.gap, h + spacing.gap))
        .collect();
    let [max_width, max_height] = fixed.map(|axis| axis.unwrap_or(limit));
    let bin = |edge: usize| {
        edge.checked_sub(2 * spacing.border)
            .map(|edge| edge + spacing.gap)
    };
    let (Some(bin_width), Some(bin_height)) = (bin(max_width), bin(max_height)) else {
        return Err("Samples do not fit in the requested sheet dimensions".into());
    };
    if bin_width == 0 || bin_height == 0 {
        return Err("Samples do not fit in the requested sheet dimensions".into());
    }
    let widest = reserved.iter().map(|size| size.0).max().unwrap();
    let tallest = reserved.iter().map(|size| size.1).max().unwrap();
    let area = reserved
        .iter()
        .map(|(w, h)| w.saturating_mul(*h))
        .fold(0usize, usize::saturating_add);
    let min_width = widest.max(area.div_ceil(bin_height));
    if min_width > bin_width || tallest > bin_height {
        return Err("Samples do not fit in the requested sheet dimensions".into());
    }
    if sizes.iter().all(|size| size == first) {
        let [step_x, step_y] = first.map(|axis| axis + spacing.gap);
        let max_columns = bin_width / step_x;
        let max_rows = bin_height / step_y;
        let ideal = area.isqrt().div_ceil(step_x);
        let columns = ideal.max(sizes.len().div_ceil(max_rows)).min(max_columns);
        if sizes.len().div_ceil(columns) <= max_rows {
            return bands(sizes, fixed, spacing, columns, false, true);
        }
        return Err("Samples do not fit in the requested sheet dimensions".into());
    }

    let mut width = if fixed[0].is_some() {
        bin_width
    } else {
        area.isqrt().max(min_width).min(bin_width)
    };
    let mut remaining = 30_000_000;
    loop {
        let mut height = if fixed[1].is_some() {
            bin_height
        } else {
            tallest.max(area.div_ceil(width))
        };
        loop {
            if let Some(positions) = pack_rect_positions(&reserved, width, height, &mut remaining) {
                return Ok(positions
                    .into_iter()
                    .map(|p| p.map(|axis| axis + spacing.border))
                    .collect());
            }
            if remaining == 0 {
                return Err(
                    "Packed sheet search exceeded its work limit; try Rows or fewer samples".into(),
                );
            }
            if height >= bin_height {
                break;
            }
            height = height.saturating_mul(2).min(bin_height);
        }
        if width >= bin_width {
            return Err("Samples do not fit in the requested sheet dimensions".into());
        }
        width = width.saturating_mul(2).min(bin_width);
    }
}
