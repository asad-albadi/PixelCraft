use super::Point;
use crate::model::line_pixels;
use crate::productivity::pixel_perfect_corner;
use serde::Deserialize;
use std::convert::Infallible;

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(try_from = "u8")]
pub(super) enum Algorithm {
    #[default]
    Regular,
    PixelPerfect,
    Dots,
}

impl TryFrom<u8> for Algorithm {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Regular),
            1 => Ok(Self::PixelPerfect),
            2 => Ok(Self::Dots),
            _ => Err("Invalid freehand algorithm"),
        }
    }
}

pub(super) fn visit(
    points: &[Point],
    algorithm: Algorithm,
    mut position: impl FnMut(Point) -> Option<Point>,
    mut emit: impl FnMut(Point),
) {
    if algorithm == Algorithm::Dots {
        points.iter().copied().filter_map(position).for_each(emit);
        return;
    }

    // Keep the last two connected centers until the next one tells us whether
    // the pending center is an orthogonal corner. This avoids a second stroke
    // buffer proportional to the length of the interpolated path.
    let mut previous = None;
    let mut pending = None;
    let mut accept = |point| -> Result<(), Infallible> {
        let point = position(point);
        if algorithm == Algorithm::Regular {
            if let Some(point) = point {
                emit(point);
            }
            return Ok(());
        }
        if point == pending {
            return Ok(());
        }
        if let Some(last) = pending
            && !previous
                .zip(point)
                .is_some_and(|(a, c)| pixel_perfect_corner(a, last, c))
        {
            emit(last);
            previous = Some(last);
        }
        if point.is_none() {
            previous = None;
        }
        pending = point;
        Ok(())
    };
    if let [point] = points {
        _ = accept(*point);
    }
    for segment in points.windows(2) {
        _ = line_pixels(segment[0], segment[1], &mut accept);
    }
    if let Some(last) = pending {
        emit(last);
    }
}
