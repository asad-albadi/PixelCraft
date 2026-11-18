use super::Pixel;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum IndexedFit {
    #[default]
    Default,
    Rgb,
    LinearRgb,
    Xyz,
    Lab,
}

impl IndexedFit {
    pub const ALL: [Self; 5] = [
        Self::Default,
        Self::Rgb,
        Self::LinearRgb,
        Self::Xyz,
        Self::Lab,
    ];

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "default" => Ok(Self::Default),
            "rgb" | "RGB" => Ok(Self::Rgb),
            "linear-rgb" | "linearizedRGB" => Ok(Self::LinearRgb),
            "xyz" | "CIEXYZ" => Ok(Self::Xyz),
            "lab" | "CIELAB" => Ok(Self::Lab),
            _ => Err("Unsupported indexed color-fit criterion".into()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Rgb => "RGB",
            Self::LinearRgb => "Linear RGB",
            Self::Xyz => "CIE XYZ",
            Self::Lab => "CIE Lab",
        }
    }

    pub(super) fn coordinates(self, color: Pixel) -> [f64; 4] {
        let [r, g, b, a] = color.map(f64::from);
        if self == Self::Default {
            return [r, g, b, a];
        }
        let alpha = a / 128.;
        if self == Self::Rgb {
            return [r, g, b, alpha];
        }
        let [r, g, b] = [r, g, b].map(|channel| {
            let value = channel / 255.;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        if self == Self::LinearRgb {
            return [r, g, b, alpha];
        }
        let x = 41.24564 * r + 35.75761 * g + 18.04375 * b;
        let y = 21.26729 * r + 71.51522 * g + 7.2175 * b;
        let z = 1.93339 * r + 11.91920 * g + 95.03041 * b;
        if self == Self::Xyz {
            return [x, y, z, alpha];
        }
        let f = |t: f64| {
            if t > 0.00885645171 {
                t.cbrt()
            } else {
                t / 0.12841855 + 0.137931034
            }
        };
        let fy = f(y / 100.);
        [
            116. * fy - 16.,
            500. * (f(x / 95.0489) - fy),
            200. * (fy - f(z / 108.884)),
            alpha,
        ]
    }
}
