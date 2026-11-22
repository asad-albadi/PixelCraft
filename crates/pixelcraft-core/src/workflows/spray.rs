use super::mix_paint_seed;

type Point = (i32, i32);

pub const DEFAULT_RADIUS: i32 = 8;
pub const DEFAULT_RATE: u16 = 60;
pub const MAX_RADIUS: i32 = 64;
pub const MAX_RATE: u16 = 240;
pub const MAX_INTERVAL_MS: u16 = 125;

pub struct SprayPattern {
    state: u64,
    credit: f64,
}

impl SprayPattern {
    pub fn new(seed: u64, rate: u16) -> Self {
        Self {
            state: seed,
            credit: f64::from((rate / 24).max(1)),
        }
    }

    pub fn emit(
        &mut self,
        from: Point,
        to: Point,
        elapsed: f64,
        radius: i32,
        rate: u16,
        mut plot: impl FnMut(Point),
    ) {
        self.credit += elapsed.clamp(0., f64::from(MAX_INTERVAL_MS) / 1000.) * f64::from(rate);
        let count = self.credit.floor() as u16;
        self.credit -= f64::from(count);
        for i in 1..=count {
            let t = f64::from(i) / f64::from(count);
            let (dx, dy) = self.offset(radius);
            let x = from.0 + (f64::from(to.0 - from.0) * t).round() as i32 + dx;
            let y = from.1 + (f64::from(to.1 - from.1) * t).round() as i32 + dy;
            plot((x, y));
        }
    }

    pub fn clear_credit(&mut self) {
        self.credit = 0.;
    }

    pub fn seconds_to_next_dot(&self, rate: u16) -> f64 {
        (1. - self.credit) / f64::from(rate)
    }

    fn offset(&mut self, radius: i32) -> Point {
        let diameter = (2 * radius + 1) as u64;
        loop {
            self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
            let random = mix_paint_seed(self.state);
            let x = ((random >> 32) % diameter) as i32 - radius;
            let y = ((random & u64::from(u32::MAX)) % diameter) as i32 - radius;
            if x * x + y * y <= radius * radius {
                return (x, y);
            }
        }
    }
}
