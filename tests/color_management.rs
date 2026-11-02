use moxcms::{ColorProfile, ToneReprCurve};
use pixelcraft::{color::display_pixels, model::*};
#[test]
fn embedded_linear_icc_is_rendered_in_srgb_without_changing_source() {
    let mut profile = ColorProfile::new_srgb();
    profile.cicp = None;
    profile.red_trc = Some(ToneReprCurve::Parametric(vec![1.]));
    profile.green_trc = profile.red_trc.clone();
    profile.blue_trc = profile.red_trc.clone();
    let icc = profile.encode().unwrap();
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [128, 128, 128, 91];
    d.color_profile = vec![0; 16];
    d.color_profile[0] = 2;
    d.color_profile.extend((icc.len() as u32).to_le_bytes());
    d.color_profile.extend(icc);
    let display = display_pixels(&d, 0).unwrap();
    assert!((186..=190).contains(&display[0][0]));
    assert_eq!(display[0][3], 91);
    assert_eq!(d.layers[0].cels[0][0], [128, 128, 128, 91]);
}
#[test]
fn fixed_gamma_linear_srgb() {
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [128, 0, 0, 255];
    d.color_profile = vec![1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!((186..=190).contains(&display_pixels(&d, 0).unwrap()[0][0]));
}
