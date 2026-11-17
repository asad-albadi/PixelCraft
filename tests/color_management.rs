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
fn profile_assignment_and_conversion_have_distinct_pixel_semantics() {
    use pixelcraft::color::{assign_document_profile, convert_document_profile, icc_profile};
    let mut linear = ColorProfile::new_srgb();
    linear.cicp = None;
    linear.red_trc = Some(ToneReprCurve::Parametric(vec![1.]));
    linear.green_trc = linear.red_trc.clone();
    linear.blue_trc = linear.red_trc.clone();
    let linear = icc_profile(&linear.encode().unwrap()).unwrap();
    let mut doc = Document::new(2, 1);
    let source = [128, 128, 128, 91];
    doc.layers[0].cels[0].set(0, source);
    doc.palette[1] = source;
    doc.layers[0].link_duplicates = true;
    doc.add_frame(0, true);
    let assigned = assign_document_profile(&doc, &linear).unwrap();
    assert_eq!(assigned.layers[0].cels[0][0], source);
    assert!(assigned.layers[0].cels[0].shares_storage(&doc.layers[0].cels[0]));
    let converted = convert_document_profile(&assigned, &[]).unwrap();
    assert!(converted.color_profile.is_empty());
    let srgb = converted.layers[0].cels[0][0];
    assert!((186..=190).contains(&srgb[0]));
    assert_eq!(srgb[3], 91);
    assert_eq!(converted.palette[1], srgb);
    assert_eq!(converted.layers[0].cels[0][1], CLEAR);
    assert!(converted.layers[0].cels[0].shares_storage(&converted.layers[0].cels[1]));
    assert_eq!(assigned.layers[0].cels[0][0], source);
    assert!(icc_profile(b"invalid").is_err());
}

#[test]
fn image_import_policies_and_layer_conversion_respect_working_profiles() {
    use pixelcraft::{
        color::{ImportProfile, apply_import_profile},
        productivity::prepare_raster_layer,
    };
    let pixel = [128, 128, 128, 91];
    let mut linear = Document::new(1, 1);
    linear.layers[0].cels[0].set(0, pixel);
    linear.color_profile = vec![1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let preserved = apply_import_profile(linear.clone(), ImportProfile::Preserve).unwrap();
    assert_eq!(preserved, linear);
    let assigned = apply_import_profile(linear.clone(), ImportProfile::AssignSrgb).unwrap();
    assert_eq!(assigned.layers[0].cels[0][0], pixel);
    assert!(assigned.color_profile.is_empty());
    let converted = apply_import_profile(linear.clone(), ImportProfile::ConvertToSrgb).unwrap();
    assert!(converted.color_profile.is_empty());
    assert!((186..=190).contains(&converted.layers[0].cels[0][0][0]));

    let imported = prepare_raster_layer(&linear, &assigned, 0, "From sRGB".into()).unwrap();
    assert!((53..=57).contains(&imported.cels[0][0][0]));
    assert_eq!(imported.cels[0][0][3], 91);
    let imported = prepare_raster_layer(&assigned, &linear, 0, "From linear".into()).unwrap();
    assert!((186..=190).contains(&imported.cels[0][0][0]));
    assert_eq!(imported.cels[0][0][3], 91);
}

#[test]
fn fixed_gamma_linear_srgb() {
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [128, 0, 0, 255];
    d.color_profile = vec![1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!((186..=190).contains(&display_pixels(&d, 0).unwrap()[0][0]));
}
