use std::fs;
use std::path::PathBuf;

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use serde_json::Value;

use crate::{
    DeterministicUidInput, GenerateError, PreparedGenerationRun, UidRole, deterministic_uid,
    sha256_hex,
    validation::{Part10Expectations, PixelDataLengthFormula, validate_part10_file},
};

const PIXEL_RECIPE_VERSION: &str = "0.1.0";
const MONO_PIXELS: [u8; 4] = [0, 85, 170, 255];
const RGB_PLANAR0_PIXELS: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
const RGB_PLANAR1_PIXELS: [u8; 12] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
const MONO_U16_PIXELS: [u8; 8] = [0, 0, 0x55, 0x55, 0xaa, 0xaa, 0xff, 0xff];
const MONO_U16_VALUES: [i32; 4] = [0, 21845, 43690, 65535];
const MONO_I16_PIXELS: [u8; 8] = [0x00, 0x80, 0x55, 0xd5, 0xaa, 0x2a, 0xff, 0x7f];
const MONO_I16_VALUES: [i32; 4] = [-32768, -10923, 10922, 32767];
const MONO_U16_ODD_3X3_PIXELS: [u8; 18] = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0];
const MONO_U16_ODD_3X3_VALUES: [i32; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
const MONO_U16_RECT_2X3_PIXELS: [u8; 12] = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0];
const MONO_U16_RECT_2X3_VALUES: [i32; 6] = [0, 1, 2, 3, 4, 5];
const MONO_U16_TINY_1X1_PIXELS: [u8; 2] = [0xff, 0xff];
const MONO_U16_TINY_1X1_VALUES: [i32; 1] = [65535];
const MONO_U16_PADDING_PIXELS: [u8; 8] = [0, 0, 0xe8, 0x03, 0xd0, 0x07, 0xb8, 0x0b];
const MONO_U16_PADDING_VALUES: [i32; 4] = [0, 1000, 2000, 3000];
const YBR_FULL_PLANAR0_PIXELS: [u8; 12] = [76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128];
const YBR_FULL_422_PIXELS: [u8; 8] = [76, 150, 65, 138, 29, 255, 192, 118];
const PALETTE_COLOR_PIXELS: [u8; 4] = [0, 1, 2, 3];
const PALETTE_COLOR_VALUES: [i32; 4] = [0, 1, 2, 3];
const PALETTE_DESCRIPTOR: [u16; 3] = [4, 0, 16];
const PALETTE_RED_DATA: [u8; 8] = [0xff, 0xff, 0, 0, 0, 0, 0xff, 0xff];
const PALETTE_GREEN_DATA: [u8; 8] = [0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff];
const PALETTE_BLUE_DATA: [u8; 8] = [0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
const PALETTE_COLOR_LUT: PaletteRecipe = PaletteRecipe {
    descriptor: PALETTE_DESCRIPTOR,
    red_data: &PALETTE_RED_DATA,
    green_data: &PALETTE_GREEN_DATA,
    blue_data: &PALETTE_BLUE_DATA,
};

const PIXEL_RECIPES: &[PixelRecipe] = &[
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_explicit_le",
        recipe_id: "sc_mono2_u8",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient",
        semantic_note: "minimum sample value displays as black",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_explicit_le",
        recipe_id: "sc_mono1_u8",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_inverse_monochrome_gradient",
        semantic_note: "minimum sample value displays as white",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar0_explicit_le",
        recipe_id: "sc_rgb_planar0",
        rows: 2,
        columns: 2,
        photometric_interpretation: "RGB",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_red_green_blue_white",
        semantic_note: "RGB samples are interleaved color-by-pixel",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar1_explicit_le",
        recipe_id: "sc_rgb_planar1",
        rows: 2,
        columns: 2,
        photometric_interpretation: "RGB",
        samples_per_pixel: 3,
        planar_configuration: Some(1),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &RGB_PLANAR1_PIXELS,
        pixel_values: &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_planar1_red_green_blue_white",
        semantic_note: "RGB samples are stored contiguously by color plane",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/palette_color_u8_explicit_le",
        recipe_id: "sc_palette_color_u8",
        rows: 2,
        columns: 2,
        photometric_interpretation: "PALETTE COLOR",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &PALETTE_COLOR_PIXELS,
        pixel_values: &PALETTE_COLOR_VALUES,
        pixel_min: 0,
        pixel_max: 3,
        visual_pattern: "2x2_palette_red_green_blue_white",
        semantic_note: "stored pixel values index 16-bit RGB palette lookup tables",
        palette: Some(PALETTE_COLOR_LUT),
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_planar0_explicit_le",
        recipe_id: "sc_ybr_full_planar0",
        rows: 2,
        columns: 2,
        photometric_interpretation: "YBR_FULL",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &YBR_FULL_PLANAR0_PIXELS,
        pixel_values: &[76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128],
        pixel_min: 21,
        pixel_max: 255,
        visual_pattern: "2x2_ybr_full_red_green_blue_white",
        semantic_note: "YBR_FULL samples are interleaved color-by-pixel",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_422_explicit_le",
        recipe_id: "sc_ybr_full_422",
        rows: 2,
        columns: 2,
        photometric_interpretation: "YBR_FULL_422",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &YBR_FULL_422_PIXELS,
        pixel_values: &[76, 150, 65, 138, 29, 255, 192, 118],
        pixel_min: 29,
        pixel_max: 255,
        visual_pattern: "2x2_ybr_full_422_red_green_blue_white",
        semantic_note: "YBR_FULL_422 stores horizontally downsampled chroma with Planar Configuration 0",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_explicit_le",
        recipe_id: "sc_mono2_u16",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_explicit_le",
        recipe_id: "sc_mono2_i16",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_I16_PIXELS,
        pixel_values: &MONO_I16_VALUES,
        pixel_min: -32768,
        pixel_max: 32767,
        visual_pattern: "2x2_monochrome_i16_gradient",
        semantic_note: "16-bit signed MONOCHROME2 samples use 2's complement representation",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_odd_3x3_explicit_le",
        recipe_id: "sc_mono2_u16_odd_3x3",
        rows: 3,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_ODD_3X3_PIXELS,
        pixel_values: &MONO_U16_ODD_3X3_VALUES,
        pixel_min: 0,
        pixel_max: 8,
        visual_pattern: "3x3_monochrome_u16_odd_gradient",
        semantic_note: "odd rows and columns use unsigned 16-bit MONOCHROME2 samples",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_rect_2x3_explicit_le",
        recipe_id: "sc_mono2_u16_rect_2x3",
        rows: 2,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_RECT_2X3_PIXELS,
        pixel_values: &MONO_U16_RECT_2X3_VALUES,
        pixel_min: 0,
        pixel_max: 5,
        visual_pattern: "2x3_monochrome_u16_rect_gradient",
        semantic_note: "rectangular native Pixel Data uses Rows and Columns from the recipe",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_tiny_1x1_explicit_le",
        recipe_id: "sc_mono2_u16_tiny_1x1",
        rows: 1,
        columns: 1,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_TINY_1X1_PIXELS,
        pixel_values: &MONO_U16_TINY_1X1_VALUES,
        pixel_min: 65535,
        pixel_max: 65535,
        visual_pattern: "1x1_monochrome_u16_tiny_maximum",
        semantic_note: "very small native Pixel Data uses a single unsigned 16-bit sample",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_padding_explicit_le",
        recipe_id: "sc_mono2_u16_padding",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_PADDING_PIXELS,
        pixel_values: &MONO_U16_PADDING_VALUES,
        pixel_min: 0,
        pixel_max: 3000,
        visual_pattern: "2x2_monochrome_u16_with_padding_value",
        semantic_note: "Pixel Padding Value 0 identifies a padded unsigned MONOCHROME2 sample",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
];

#[derive(Debug, Clone, Copy)]
struct PixelRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    photometric_interpretation: &'static str,
    samples_per_pixel: u16,
    planar_configuration: Option<u16>,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_representation: u16,
    pixel_vr: VR,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    visual_pattern: &'static str,
    semantic_note: &'static str,
    palette: Option<PaletteRecipe>,
    padding: Option<PixelPaddingRecipe>,
}

#[derive(Debug, Clone, Copy)]
struct PaletteRecipe {
    descriptor: [u16; 3],
    red_data: &'static [u8],
    green_data: &'static [u8],
    blue_data: &'static [u8],
}

#[derive(Debug, Clone, Copy)]
struct PixelPaddingRecipe {
    value: u16,
    range_limit: Option<u16>,
}

#[derive(Debug, Clone)]
pub(crate) struct GeneratedFile {
    pub case_id: String,
    pub manifest_entry: Value,
}

pub(crate) fn write_supported_cases(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let mut generated_files = Vec::new();
    for recipe in PIXEL_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.push(write_pixel_case(run, case, *recipe, standards_lock_sha256)?);
    }
    Ok(generated_files)
}

fn write_pixel_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: PixelRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_case_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_case_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_case_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "DICOMTEST^SMOKE");
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DICOMTEST-SMOKE-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "SMOKE");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "OT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");

    put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");
    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");

    put_u16(
        &mut obj,
        tags::SAMPLES_PER_PIXEL,
        VR::US,
        recipe.samples_per_pixel,
    );
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        recipe.photometric_interpretation,
    );
    if let Some(planar_configuration) = recipe.planar_configuration {
        put_u16(
            &mut obj,
            tags::PLANAR_CONFIGURATION,
            VR::US,
            planar_configuration,
        );
    }
    put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
    put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
    put_u16(
        &mut obj,
        tags::BITS_ALLOCATED,
        VR::US,
        recipe.bits_allocated,
    );
    put_u16(&mut obj, tags::BITS_STORED, VR::US, recipe.bits_stored);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, recipe.high_bit);
    put_u16(
        &mut obj,
        tags::PIXEL_REPRESENTATION,
        VR::US,
        recipe.pixel_representation,
    );
    if let Some(palette) = recipe.palette {
        put_palette(&mut obj, palette);
    }
    if let Some(padding) = recipe.padding {
        put_u16(&mut obj, tags::PIXEL_PADDING_VALUE, VR::US, padding.value);
        if let Some(range_limit) = padding.range_limit {
            put_u16(
                &mut obj,
                tags::PIXEL_PADDING_RANGE_LIMIT,
                VR::US,
                range_limit,
            );
        }
    }
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        recipe.pixel_vr,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            photometric_interpretation: recipe.photometric_interpretation,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            high_bit: recipe.high_bit,
            pixel_representation: recipe.pixel_representation,
            planar_configuration: recipe.planar_configuration,
            pixel_data_vr: recipe.pixel_vr,
            pixel_data_length: recipe.pixel_bytes.len(),
            pixel_data_length_formula: pixel_data_length_formula(recipe),
            palette: recipe.palette.map(|palette| palette.into()),
            padding: recipe.padding.map(|padding| padding.into()),
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: pixel_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn pixel_manifest_entry(
    case: &Value,
    recipe: PixelRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_sop_class SecondaryCaptureImageStorage",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.8-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element SyntheticData",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "search_standard_text Image Pixel Description Macro",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-11c"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "retrieve_standard_text sect_C.7.6.3.1.2",
            "covered": true,
            "part": "PS3.3",
            "anchor": "sect_C.7.6.3.1.2"
        }),
    ]);
    if recipe.planar_configuration.is_some() {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element PlanarConfiguration",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_C.7.6.3.1.3",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_C.7.6.3.1.3"
            }),
        ]);
    }
    if recipe.palette.is_some() {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_C.7.6.3.1.5",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_C.7.6.3.1.5"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element RedPaletteColorLookupTableDescriptor",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element GreenPaletteColorLookupTableDescriptor",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element BluePaletteColorLookupTableDescriptor",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element RedPaletteColorLookupTableData",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element GreenPaletteColorLookupTableData",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element BluePaletteColorLookupTableData",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
        ]);
    }
    if recipe.padding.is_some() {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element PixelPaddingValue",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element PixelPaddingRangeLimit",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_C.7.5.1.1.2",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_C.7.5.1.1.2"
            }),
        ]);
    }

    let palette_manifest = recipe.palette.map(|palette| {
        serde_json::json!({
            "descriptor": palette.descriptor,
            "red_data_value_length": palette.red_data.len(),
            "green_data_value_length": palette.green_data.len(),
            "blue_data_value_length": palette.blue_data.len()
        })
    });
    let padding_manifest = recipe.padding.map(|padding| {
        serde_json::json!({
            "value": padding.value,
            "range_limit": padding.range_limit
        })
    });

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["smoke"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": PIXEL_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": recipe.samples_per_pixel,
                "photometric_interpretation": recipe.photometric_interpretation,
                "bits_allocated": recipe.bits_allocated,
                "bits_stored": recipe.bits_stored,
                "planar_configuration": recipe.planar_configuration,
                "pixel_values": recipe.pixel_values,
                "palette": palette_manifest,
                "pixel_padding": padding_manifest
            }
        },
        "dicom": {
            "sop_class_uid": uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            "sop_class_name": "Secondary Capture Image Storage",
            "iod_name": "Secondary Capture Image",
            "modality": "OT",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": recipe.samples_per_pixel,
            "photometric_interpretation": recipe.photometric_interpretation,
            "bits_allocated": recipe.bits_allocated,
            "bits_stored": recipe.bits_stored,
            "high_bit": recipe.high_bit,
            "pixel_representation": recipe.pixel_representation,
            "planar_configuration": recipe.planar_configuration
        },
        "pixel_data": {
            "vr": pixel_vr_name(recipe.pixel_vr),
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(recipe.pixel_bytes)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "conversion_type": "SYN",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "pixel_padding": padding_manifest,
            "photometric_semantics": recipe.semantic_note
        },
        "expected_visual_checks": {
            "pattern": recipe.visual_pattern
        },
        "validation": validation,
        "known_stressors": ["minimal_secondary_capture", "native_ob_pixel_data"],
        "standards_evidence": standards_evidence
    })
}

fn deterministic_case_uid(
    standards_lock_sha256: &str,
    recipe: PixelRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: PIXEL_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_implementation_uid(standards_lock_sha256: &str) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    })
}

fn pixel_vr_name(vr: VR) -> &'static str {
    match vr {
        VR::OB => "OB",
        VR::OW => "OW",
        _ => "UN",
    }
}

fn pixel_data_length_formula(recipe: PixelRecipe) -> PixelDataLengthFormula {
    match recipe.photometric_interpretation {
        "YBR_FULL_422" => PixelDataLengthFormula::YbrFull422,
        _ => PixelDataLengthFormula::ContiguousSamples,
    }
}

fn put_str(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    obj.put(DataElement::new(tag, vr, value));
}

fn put_u16(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: u16) {
    obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
}

fn put_palette(obj: &mut InMemDicomObject, palette: PaletteRecipe) {
    for tag in [
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
    ] {
        obj.put(DataElement::new(
            tag,
            VR::US,
            PrimitiveValue::from(palette.descriptor),
        ));
    }
    obj.put(DataElement::new(
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        VR::OW,
        PrimitiveValue::from(palette.red_data),
    ));
    obj.put(DataElement::new(
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        VR::OW,
        PrimitiveValue::from(palette.green_data),
    ));
    obj.put(DataElement::new(
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        VR::OW,
        PrimitiveValue::from(palette.blue_data),
    ));
}

impl From<PaletteRecipe> for crate::validation::PaletteExpectations {
    fn from(palette: PaletteRecipe) -> Self {
        Self {
            descriptor: palette.descriptor,
            red_data_length: palette.red_data.len(),
            green_data_length: palette.green_data.len(),
            blue_data_length: palette.blue_data.len(),
        }
    }
}

impl From<PixelPaddingRecipe> for crate::validation::PixelPaddingExpectations {
    fn from(padding: PixelPaddingRecipe) -> Self {
        Self {
            value: padding.value,
            range_limit: padding.range_limit,
        }
    }
}

fn registry_case<'a>(
    registry: &'a Value,
    case_id: &str,
) -> Result<Option<&'a Value>, GenerateError> {
    let cases =
        registry
            .get("cases")
            .and_then(Value::as_array)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "missing cases array",
            })?;
    Ok(cases
        .iter()
        .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id)))
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, GenerateError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case profiles must be a string array",
        })?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("cases/registry.json"),
                    message: "case profiles must be a string array",
                })
        })
        .collect()
}

fn case_matches_profile(profiles: &[String], requested: &str, include_stress: bool) -> bool {
    match requested {
        "all" => profiles.iter().any(|profile| {
            matches!(profile.as_str(), "smoke" | "core" | "extended" | "legacy")
                || (include_stress && profile == "stress")
        }),
        profile => profiles.iter().any(|case_profile| case_profile == profile),
    }
}
