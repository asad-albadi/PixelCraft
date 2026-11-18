use super::{rotation_basis, snap_near_integer};
use crate::model::{MAX_DIMENSION, MAX_PIXELS};

/// Maps sprite edge coordinates using two basis vectors and a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineTransform {
    x_axis: [f64; 2],
    y_axis: [f64; 2],
    offset: [f64; 2],
}

impl AffineTransform {
    pub const IDENTITY: Self = Self {
        x_axis: [1., 0.],
        y_axis: [0., 1.],
        offset: [0., 0.],
    };

    pub fn about(x_axis: [f64; 2], y_axis: [f64; 2], pivot: [f64; 2]) -> Self {
        let mut matrix = Self {
            x_axis,
            y_axis,
            offset: [0., 0.],
        };
        let mapped = matrix.map(pivot);
        matrix.offset = std::array::from_fn(|axis| pivot[axis] - mapped[axis]);
        matrix
    }

    pub fn translation(offset: [f64; 2]) -> Self {
        Self {
            offset,
            ..Self::IDENTITY
        }
    }

    pub fn rotation(degrees: f64, pivot: [f64; 2]) -> Self {
        let [sin, cos] = rotation_basis(degrees);
        Self::about([cos, sin], [-sin, cos], pivot)
    }

    pub fn map_vector(self, [x, y]: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|axis| x * self.x_axis[axis] + y * self.y_axis[axis])
    }

    pub fn map(self, point: [f64; 2]) -> [f64; 2] {
        let point = self.map_vector(point);
        std::array::from_fn(|axis| point[axis] + self.offset[axis])
    }

    /// Apply this transform followed by the next one.
    pub fn then(self, next: Self) -> Self {
        Self {
            x_axis: next.map_vector(self.x_axis),
            y_axis: next.map_vector(self.y_axis),
            offset: next.map(self.offset),
        }
    }

    pub fn inverse(self) -> Option<Self> {
        let [a, c] = self.x_axis;
        let [b, d] = self.y_axis;
        let determinant = a * d - b * c;
        if !determinant.is_finite() || determinant.abs() < 1e-10 {
            return None;
        }
        let mut inverse = Self {
            x_axis: [d / determinant, -c / determinant],
            y_axis: [-b / determinant, a / determinant],
            offset: [0., 0.],
        };
        inverse.offset = inverse.map_vector(self.offset.map(|value| -value));
        Some(inverse)
    }

    pub(super) fn pixel_center(self, position: [i32; 2]) -> [f64; 2] {
        self.map(position.map(|coordinate| f64::from(coordinate) + 0.5))
            .map(snap_near_integer)
    }

    pub(super) fn bounds(self, [x, y, width, height]: [i32; 4]) -> Option<[i32; 4]> {
        let [x, y, width, height] = [x, y, width, height].map(f64::from);
        let corners = [
            [x, y],
            [x + width, y],
            [x, y + height],
            [x + width, y + height],
        ];
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for point in corners.map(|point| self.map(point)) {
            for axis in 0..2 {
                if !point[axis].is_finite() || point[axis].abs() > (MAX_DIMENSION * 8) as f64 {
                    return None;
                }
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        let [left, top] = min.map(|value| snap_near_integer(value).floor() as i32);
        let [right, bottom] = max.map(|value| snap_near_integer(value).ceil() as i32);
        Some([left, top, right - left, bottom - top])
    }

    pub fn fits_selection(self, bounds: [i32; 4]) -> bool {
        self.inverse().is_some()
            && self.bounds(bounds).is_some_and(|[_, _, w, h]| {
                w > 0
                    && h > 0
                    && w as usize <= MAX_DIMENSION
                    && h as usize <= MAX_DIMENSION
                    && (w as usize).saturating_mul(h as usize) <= MAX_PIXELS
            })
    }

    pub(super) fn needs_enlargement(self) -> bool {
        // An axis-aligned resize or a right-angle rotation has no oblique edges.
        let [a, c] = self.x_axis;
        let [b, d] = self.y_axis;
        !((a.abs() < 1e-10 && d.abs() < 1e-10) || (b.abs() < 1e-10 && c.abs() < 1e-10))
    }
}
