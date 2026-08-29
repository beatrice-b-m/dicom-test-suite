//! Frontend-neutral planning and deterministic construction of native pixels.
//!
//! This module deliberately knows nothing about recipes, composition, DICOM
//! writers, output paths, or codecs. Frontends translate their declarations
//! into [`NativePixelRequest`]; encoding adapters consume the resulting bytes
//! and frame identities.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::hashing::StreamingSha256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredValueType {
    U1,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
}

impl StoredValueType {
    pub const fn bits_allocated(self) -> u16 {
        match self {
            Self::U1 => 1,
            Self::U8 | Self::I8 => 8,
            Self::U16 | Self::I16 => 16,
            Self::U32 | Self::I32 => 32,
        }
    }

    pub const fn pixel_representation(self) -> u16 {
        match self {
            Self::I8 | Self::I16 | Self::I32 => 1,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelDataVr {
    Ob,
    Ow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhotometricInterpretation {
    #[serde(rename = "MONOCHROME1")]
    Monochrome1,
    #[serde(rename = "MONOCHROME2")]
    Monochrome2,
    #[serde(rename = "PALETTE COLOR")]
    PaletteColor,
    #[serde(rename = "RGB")]
    Rgb,
    #[serde(rename = "YBR_FULL")]
    YbrFull,
    #[serde(rename = "YBR_FULL_422")]
    YbrFull422,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChromaSubsampling {
    None,
    Horizontal2To1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorOrganization {
    pub planar_configuration: u8,
    pub chroma_subsampling: ChromaSubsampling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelPadding {
    pub value: i64,
    pub range_limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub descriptor: [u32; 3],
    pub red: Vec<u16>,
    pub green: Vec<u16>,
    pub blue: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelShape {
    pub rows: u32,
    pub columns: u32,
    pub frames: u32,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: PhotometricInterpretation,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    pub stored_value_type: StoredValueType,
    pub byte_order: ByteOrder,
    pub pixel_data_vr: PixelDataVr,
    pub color: Option<ColorOrganization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSpan {
    pub frame_number: u32,
    pub value_offset: u64,
    pub value_count: u64,
    pub bit_offset: u64,
    pub bit_length: u64,
    pub first_byte_offset: u64,
    pub bytes_touched: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePixelPlan {
    pub shape: PixelShape,
    pub stored_values_per_frame: u64,
    pub unpadded_value_bytes: u64,
    pub padded_value_bytes: u64,
    pub padding_bytes: u8,
    pub frame_spans: Vec<FrameSpan>,
}

impl NativePixelPlan {
    pub fn plan(shape: PixelShape) -> Result<Self, NativePixelError> {
        Self::plan_with_limits(shape, NativePixelLimits::default())
    }

    pub fn plan_with_limits(
        shape: PixelShape,
        limits: NativePixelLimits,
    ) -> Result<Self, NativePixelError> {
        validate_shape(&shape)?;
        limits.validate()?;
        let pixels = checked_mul(u64::from(shape.rows), u64::from(shape.columns))?;
        let stored_samples_per_pixel =
            if shape.photometric_interpretation == PhotometricInterpretation::YbrFull422 {
                2
            } else {
                u64::from(shape.samples_per_pixel)
            };
        let stored_values_per_frame = checked_mul(pixels, stored_samples_per_pixel)?;
        let bits_per_frame = checked_mul(stored_values_per_frame, u64::from(shape.bits_allocated))?;
        let total_bits = checked_mul(bits_per_frame, u64::from(shape.frames))?;
        let total_values = checked_mul(stored_values_per_frame, u64::from(shape.frames))?;
        let unpadded_value_bytes = total_bits
            .checked_add(7)
            .ok_or(NativePixelError::ArithmeticOverflow)?
            / 8;
        let padding_bytes = (unpadded_value_bytes & 1) as u8;
        let padded_value_bytes = unpadded_value_bytes
            .checked_add(u64::from(padding_bytes))
            .ok_or(NativePixelError::ArithmeticOverflow)?;
        enforce_limit("frames", limits.max_frames, u64::from(shape.frames))?;
        enforce_limit("stored_values", limits.max_stored_values, total_values)?;
        enforce_limit("value_bytes", limits.max_value_bytes, padded_value_bytes)?;
        let frame_capacity =
            usize::try_from(shape.frames).map_err(|_| NativePixelError::ArithmeticOverflow)?;
        let mut frame_spans = Vec::new();
        frame_spans
            .try_reserve_exact(frame_capacity)
            .map_err(|_| NativePixelError::AllocationFailed)?;
        for frame_index in 0..u64::from(shape.frames) {
            let bit_offset = checked_mul(frame_index, bits_per_frame)?;
            let end_bit = bit_offset
                .checked_add(bits_per_frame)
                .ok_or(NativePixelError::ArithmeticOverflow)?;
            let end_byte = end_bit
                .checked_add(7)
                .ok_or(NativePixelError::ArithmeticOverflow)?
                / 8;
            frame_spans.push(FrameSpan {
                frame_number: u32::try_from(frame_index + 1)
                    .map_err(|_| NativePixelError::ArithmeticOverflow)?,
                value_offset: checked_mul(frame_index, stored_values_per_frame)?,
                value_count: stored_values_per_frame,
                bit_offset,
                bit_length: bits_per_frame,
                first_byte_offset: bit_offset / 8,
                bytes_touched: end_byte - bit_offset / 8,
            });
        }
        Ok(Self {
            shape,
            stored_values_per_frame,
            unpadded_value_bytes,
            padded_value_bytes,
            padding_bytes,
            frame_spans,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePixelLimits {
    pub max_frames: u64,
    pub max_stored_values: u64,
    pub max_value_bytes: u64,
}

impl NativePixelLimits {
    pub const DEFAULT_MAX_FRAMES: u64 = 65_536;
    pub const DEFAULT_MAX_STORED_VALUES: u64 = 16_777_216;
    pub const DEFAULT_MAX_VALUE_BYTES: u64 = 268_435_456;

    fn validate(self) -> Result<(), NativePixelError> {
        if self.max_frames == 0 || self.max_stored_values == 0 || self.max_value_bytes == 0 {
            return Err(NativePixelError::InvalidResourceLimits);
        }
        Ok(())
    }
}

impl Default for NativePixelLimits {
    fn default() -> Self {
        Self {
            max_frames: Self::DEFAULT_MAX_FRAMES,
            max_stored_values: Self::DEFAULT_MAX_STORED_VALUES,
            max_value_bytes: Self::DEFAULT_MAX_VALUE_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePixelRequest {
    pub shape: PixelShape,
    pub stored_values: Vec<i64>,
    pub declared_pixel_min: i64,
    pub declared_pixel_max: i64,
    pub expected_frame_sha256: Vec<String>,
    pub padding: Option<PixelPadding>,
    pub palette: Option<Palette>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePixelFrame {
    pub frame_number: u32,
    /// One byte per logical U1 value; native serialized bytes for other types.
    pub decoded_bytes: Vec<u8>,
    pub decoded_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePixelContent {
    pub plan: NativePixelPlan,
    /// Native Value Field before DICOM's even-length padding byte.
    pub unpadded_bytes: Vec<u8>,
    /// Native Value Field including the deterministic zero padding byte.
    pub padded_bytes: Vec<u8>,
    pub unpadded_sha256: String,
    pub padded_sha256: String,
    pub frames: Vec<NativePixelFrame>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub padding: Option<PixelPadding>,
    pub palette: Option<Palette>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativePixelFactory;

impl NativePixelFactory {
    pub fn create(
        &self,
        request: NativePixelRequest,
    ) -> Result<NativePixelContent, NativePixelError> {
        self.create_with_limits(request, NativePixelLimits::default())
    }

    pub fn create_with_limits(
        &self,
        request: NativePixelRequest,
        limits: NativePixelLimits,
    ) -> Result<NativePixelContent, NativePixelError> {
        let plan = NativePixelPlan::plan_with_limits(request.shape, limits)?;
        let expected_values =
            checked_mul(plan.stored_values_per_frame, u64::from(plan.shape.frames))?;
        let actual_values = u64::try_from(request.stored_values.len())
            .map_err(|_| NativePixelError::ArithmeticOverflow)?;
        if actual_values != expected_values {
            return Err(NativePixelError::ValueCount {
                expected: expected_values,
                actual: actual_values,
            });
        }
        if request.stored_values.is_empty() {
            return Err(NativePixelError::ValueCount {
                expected: expected_values,
                actual: 0,
            });
        }
        let (type_min, type_max) =
            stored_range(plan.shape.stored_value_type, plan.shape.bits_stored)?;
        for (index, value) in request.stored_values.iter().copied().enumerate() {
            if value < type_min || value > type_max {
                return Err(NativePixelError::StoredValueOutOfRange {
                    index,
                    value,
                    minimum: type_min,
                    maximum: type_max,
                });
            }
        }
        let observed_min = *request
            .stored_values
            .iter()
            .min()
            .expect("non-empty values");
        let observed_max = *request
            .stored_values
            .iter()
            .max()
            .expect("non-empty values");
        if (request.declared_pixel_min, request.declared_pixel_max) != (observed_min, observed_max)
        {
            return Err(NativePixelError::DeclaredRangeMismatch {
                declared_minimum: request.declared_pixel_min,
                declared_maximum: request.declared_pixel_max,
                observed_minimum: observed_min,
                observed_maximum: observed_max,
            });
        }
        validate_padding(request.padding.as_ref(), type_min, type_max)?;
        validate_palette(
            plan.shape.photometric_interpretation,
            request.palette.as_ref(),
            &request.stored_values,
        )?;

        let values_per_frame = usize::try_from(plan.stored_values_per_frame)
            .map_err(|_| NativePixelError::ArithmeticOverflow)?;
        let mut frames = Vec::with_capacity(plan.frame_spans.len());
        for (frame_index, values) in request
            .stored_values
            .chunks_exact(values_per_frame)
            .enumerate()
        {
            let decoded_bytes = serialize_decoded_frame(values, &plan.shape)?;
            frames.push(NativePixelFrame {
                frame_number: u32::try_from(frame_index + 1)
                    .map_err(|_| NativePixelError::ArithmeticOverflow)?,
                decoded_sha256: sha256_hex(&decoded_bytes),
                decoded_bytes,
            });
        }
        if !request.expected_frame_sha256.is_empty() {
            if request.expected_frame_sha256.len() != frames.len() {
                return Err(NativePixelError::FrameHashCount {
                    expected: frames.len(),
                    actual: request.expected_frame_sha256.len(),
                });
            }
            for (index, (expected, actual)) in request
                .expected_frame_sha256
                .iter()
                .zip(&frames)
                .enumerate()
            {
                if expected != &actual.decoded_sha256 {
                    return Err(NativePixelError::FrameHashMismatch {
                        frame_number: u32::try_from(index + 1)
                            .map_err(|_| NativePixelError::ArithmeticOverflow)?,
                        expected: expected.clone(),
                        actual: actual.decoded_sha256.clone(),
                    });
                }
            }
        }

        let unpadded_bytes = if plan.shape.stored_value_type == StoredValueType::U1 {
            pack_u1_continuous(&request.stored_values)
        } else {
            frames
                .iter()
                .flat_map(|frame| frame.decoded_bytes.iter().copied())
                .collect()
        };
        if u64::try_from(unpadded_bytes.len()).ok() != Some(plan.unpadded_value_bytes) {
            return Err(NativePixelError::InternalLengthMismatch);
        }
        let mut padded_bytes = unpadded_bytes.clone();
        if plan.padding_bytes == 1 {
            padded_bytes.push(0);
        }
        let unpadded_sha256 = sha256_hex(&unpadded_bytes);
        let padded_sha256 = sha256_hex(&padded_bytes);
        Ok(NativePixelContent {
            plan,
            unpadded_bytes,
            padded_bytes,
            unpadded_sha256,
            padded_sha256,
            frames,
            pixel_min: observed_min,
            pixel_max: observed_max,
            padding: request.padding,
            palette: request.palette,
        })
    }

    pub fn create_pattern(
        &self,
        request: NativePixelPatternRequest,
    ) -> Result<NativePixelContent, NativePixelError> {
        self.create_pattern_with_limits(request, NativePixelLimits::default())
    }

    pub fn create_pattern_with_limits(
        &self,
        request: NativePixelPatternRequest,
        limits: NativePixelLimits,
    ) -> Result<NativePixelContent, NativePixelError> {
        match request {
            NativePixelPatternRequest::MonochromeHorizontalRamp {
                rows,
                columns,
                frames,
                column_step,
            } => {
                let shape = simple_u8_shape(
                    rows,
                    columns,
                    frames,
                    1,
                    PhotometricInterpretation::Monochrome2,
                    None,
                );
                NativePixelPlan::plan_with_limits(shape.clone(), limits)?;
                let value_count = checked_mul(
                    checked_mul(u64::from(rows), u64::from(columns))?,
                    u64::from(frames),
                )?;
                let capacity = usize::try_from(value_count)
                    .map_err(|_| NativePixelError::ArithmeticOverflow)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(capacity)
                    .map_err(|_| NativePixelError::AllocationFailed)?;
                for _frame in 0..frames {
                    for _row in 0..rows {
                        for column in 0..columns {
                            let value = column
                                .checked_mul(u32::from(column_step))
                                .ok_or(NativePixelError::ArithmeticOverflow)?;
                            let value = u8::try_from(value).map_err(|_| {
                                NativePixelError::PatternValueOutOfRange(i64::from(value))
                            })?;
                            values.push(i64::from(value));
                        }
                    }
                }
                self.create_with_limits(simple_u8_request(shape, values), limits)
            }
            NativePixelPatternRequest::RgbCoordinates {
                rows,
                columns,
                frames,
            } => {
                let color = Some(ColorOrganization {
                    planar_configuration: 0,
                    chroma_subsampling: ChromaSubsampling::None,
                });
                let shape = simple_u8_shape(
                    rows,
                    columns,
                    frames,
                    3,
                    PhotometricInterpretation::Rgb,
                    color,
                );
                NativePixelPlan::plan_with_limits(shape.clone(), limits)?;
                let value_count = checked_mul(
                    checked_mul(
                        checked_mul(u64::from(rows), u64::from(columns))?,
                        u64::from(frames),
                    )?,
                    3,
                )?;
                let capacity = usize::try_from(value_count)
                    .map_err(|_| NativePixelError::ArithmeticOverflow)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(capacity)
                    .map_err(|_| NativePixelError::AllocationFailed)?;
                for _frame in 0..frames {
                    for row in 0..rows {
                        for column in 0..columns {
                            let red = u8::try_from(
                                column
                                    .checked_mul(8)
                                    .ok_or(NativePixelError::ArithmeticOverflow)?,
                            )
                            .map_err(|_| {
                                NativePixelError::PatternValueOutOfRange(i64::from(column))
                            })?;
                            let green = u8::try_from(
                                row.checked_mul(8)
                                    .ok_or(NativePixelError::ArithmeticOverflow)?,
                            )
                            .map_err(|_| {
                                NativePixelError::PatternValueOutOfRange(i64::from(row))
                            })?;
                            let blue = (column as u8).wrapping_add(row as u8).wrapping_mul(4);
                            values.extend([i64::from(red), i64::from(green), i64::from(blue)]);
                        }
                    }
                }
                self.create_with_limits(simple_u8_request(shape, values), limits)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pattern", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativePixelPatternRequest {
    MonochromeHorizontalRamp {
        rows: u32,
        columns: u32,
        frames: u32,
        column_step: u8,
    },
    RgbCoordinates {
        rows: u32,
        columns: u32,
        frames: u32,
    },
}

fn simple_u8_shape(
    rows: u32,
    columns: u32,
    frames: u32,
    samples_per_pixel: u16,
    photometric_interpretation: PhotometricInterpretation,
    color: Option<ColorOrganization>,
) -> PixelShape {
    PixelShape {
        rows,
        columns,
        frames,
        samples_per_pixel,
        photometric_interpretation,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        stored_value_type: StoredValueType::U8,
        byte_order: ByteOrder::Little,
        pixel_data_vr: PixelDataVr::Ob,
        color,
    }
}

fn simple_u8_request(shape: PixelShape, stored_values: Vec<i64>) -> NativePixelRequest {
    let minimum = stored_values.iter().copied().min().unwrap_or(0);
    let maximum = stored_values.iter().copied().max().unwrap_or(0);
    NativePixelRequest {
        shape,
        stored_values,
        declared_pixel_min: minimum,
        declared_pixel_max: maximum,
        expected_frame_sha256: Vec::new(),
        padding: None,
        palette: None,
    }
}

fn validate_shape(shape: &PixelShape) -> Result<(), NativePixelError> {
    if shape.rows == 0 || shape.columns == 0 || shape.frames == 0 {
        return Err(NativePixelError::ZeroDimension);
    }
    if shape.samples_per_pixel == 0 {
        return Err(NativePixelError::InvalidColorOrganization);
    }
    if shape.bits_allocated != shape.stored_value_type.bits_allocated()
        || shape.pixel_representation != shape.stored_value_type.pixel_representation()
    {
        return Err(NativePixelError::StoredTypeMismatch);
    }
    if shape.bits_stored == 0
        || shape.bits_stored > shape.bits_allocated
        || shape.high_bit.checked_add(1) != Some(shape.bits_stored)
    {
        return Err(NativePixelError::InvalidStoredBits);
    }
    match shape.photometric_interpretation {
        PhotometricInterpretation::Monochrome1 | PhotometricInterpretation::Monochrome2 => {
            if shape.samples_per_pixel != 1 || shape.color.is_some() {
                return Err(NativePixelError::InvalidColorOrganization);
            }
        }
        PhotometricInterpretation::PaletteColor => {
            if shape.samples_per_pixel != 1 || shape.color.is_some() {
                return Err(NativePixelError::InvalidColorOrganization);
            }
        }
        PhotometricInterpretation::Rgb | PhotometricInterpretation::YbrFull => {
            let Some(color) = &shape.color else {
                return Err(NativePixelError::InvalidColorOrganization);
            };
            if shape.samples_per_pixel != 3
                || color.planar_configuration > 1
                || color.chroma_subsampling != ChromaSubsampling::None
            {
                return Err(NativePixelError::InvalidColorOrganization);
            }
        }
        PhotometricInterpretation::YbrFull422 => {
            let Some(color) = &shape.color else {
                return Err(NativePixelError::InvalidYbrFull422);
            };
            if shape.samples_per_pixel != 3
                || shape.bits_allocated != 8
                || shape.columns % 2 != 0
                || color.planar_configuration != 0
                || color.chroma_subsampling != ChromaSubsampling::Horizontal2To1
            {
                return Err(NativePixelError::InvalidYbrFull422);
            }
        }
    }
    Ok(())
}

fn stored_range(
    stored_type: StoredValueType,
    bits_stored: u16,
) -> Result<(i64, i64), NativePixelError> {
    if stored_type == StoredValueType::U1 {
        return Ok((0, 1));
    }
    if stored_type.pixel_representation() == 0 {
        let maximum = 1_i64
            .checked_shl(u32::from(bits_stored))
            .and_then(|value| value.checked_sub(1))
            .ok_or(NativePixelError::ArithmeticOverflow)?;
        Ok((0, maximum))
    } else {
        let magnitude = 1_i64
            .checked_shl(u32::from(bits_stored - 1))
            .ok_or(NativePixelError::ArithmeticOverflow)?;
        Ok((-magnitude, magnitude - 1))
    }
}

fn validate_padding(
    padding: Option<&PixelPadding>,
    minimum: i64,
    maximum: i64,
) -> Result<(), NativePixelError> {
    let Some(padding) = padding else {
        return Ok(());
    };
    for value in [Some(padding.value), padding.range_limit]
        .into_iter()
        .flatten()
    {
        if value < minimum || value > maximum {
            return Err(NativePixelError::PaddingOutOfRange(value));
        }
    }
    Ok(())
}

fn validate_palette(
    photometric: PhotometricInterpretation,
    palette: Option<&Palette>,
    stored_values: &[i64],
) -> Result<(), NativePixelError> {
    match (photometric, palette) {
        (PhotometricInterpretation::PaletteColor, None) => Err(NativePixelError::MissingPalette),
        (PhotometricInterpretation::PaletteColor, Some(palette)) => {
            let entries = if palette.descriptor[0] == 0 {
                65_536
            } else {
                usize::try_from(palette.descriptor[0])
                    .map_err(|_| NativePixelError::ArithmeticOverflow)?
            };
            if palette.red.len() != entries
                || palette.green.len() != entries
                || palette.blue.len() != entries
                || !(1..=16).contains(&palette.descriptor[2])
            {
                return Err(NativePixelError::InvalidPalette);
            }
            let max = if palette.descriptor[2] == 16 {
                u16::MAX
            } else {
                (1_u16 << palette.descriptor[2]) - 1
            };
            if palette
                .red
                .iter()
                .chain(&palette.green)
                .chain(&palette.blue)
                .any(|value| *value > max)
            {
                return Err(NativePixelError::InvalidPalette);
            }
            let minimum = i64::from(palette.descriptor[1]);
            let maximum = minimum
                .checked_add(
                    i64::try_from(entries).map_err(|_| NativePixelError::ArithmeticOverflow)? - 1,
                )
                .ok_or(NativePixelError::ArithmeticOverflow)?;
            if let Some((index, value)) = stored_values
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| *value < minimum || *value > maximum)
            {
                return Err(NativePixelError::PaletteIndexOutOfRange {
                    index,
                    value,
                    minimum,
                    maximum,
                });
            }
            Ok(())
        }
        (_, Some(_)) => Err(NativePixelError::UnexpectedPalette),
        (_, None) => Ok(()),
    }
}

fn serialize_decoded_frame(
    values: &[i64],
    shape: &PixelShape,
) -> Result<Vec<u8>, NativePixelError> {
    if shape.stored_value_type == StoredValueType::U1 {
        return values
            .iter()
            .copied()
            .map(|value| u8::try_from(value).map_err(|_| NativePixelError::InternalLengthMismatch))
            .collect();
    }
    let bytes_per_value = usize::from(shape.bits_allocated / 8);
    let capacity = values
        .len()
        .checked_mul(bytes_per_value)
        .ok_or(NativePixelError::ArithmeticOverflow)?;
    let mut bytes = Vec::with_capacity(capacity);
    for value in values.iter().copied() {
        match (shape.stored_value_type, shape.byte_order) {
            (StoredValueType::U8, _) => bytes.push(value as u8),
            (StoredValueType::I8, _) => bytes.push(value as i8 as u8),
            (StoredValueType::U16, ByteOrder::Little) => {
                bytes.extend_from_slice(&(value as u16).to_le_bytes())
            }
            (StoredValueType::U16, ByteOrder::Big) => {
                bytes.extend_from_slice(&(value as u16).to_be_bytes())
            }
            (StoredValueType::I16, ByteOrder::Little) => {
                bytes.extend_from_slice(&(value as i16).to_le_bytes())
            }
            (StoredValueType::I16, ByteOrder::Big) => {
                bytes.extend_from_slice(&(value as i16).to_be_bytes())
            }
            (StoredValueType::U32, ByteOrder::Little) => {
                bytes.extend_from_slice(&(value as u32).to_le_bytes())
            }
            (StoredValueType::U32, ByteOrder::Big) => {
                bytes.extend_from_slice(&(value as u32).to_be_bytes())
            }
            (StoredValueType::I32, ByteOrder::Little) => {
                bytes.extend_from_slice(&(value as i32).to_le_bytes())
            }
            (StoredValueType::I32, ByteOrder::Big) => {
                bytes.extend_from_slice(&(value as i32).to_be_bytes())
            }
            (StoredValueType::U1, _) => unreachable!("U1 returned above"),
        }
    }
    Ok(bytes)
}

fn pack_u1_continuous(values: &[i64]) -> Vec<u8> {
    let mut packed = vec![0_u8; values.len().div_ceil(8)];
    for (index, value) in values.iter().copied().enumerate() {
        if value == 1 {
            packed[index / 8] |= 1 << (index % 8);
        }
    }
    packed
}

fn checked_mul(left: u64, right: u64) -> Result<u64, NativePixelError> {
    left.checked_mul(right)
        .ok_or(NativePixelError::ArithmeticOverflow)
}

fn enforce_limit(
    resource: &'static str,
    limit: u64,
    requested: u64,
) -> Result<(), NativePixelError> {
    if requested > limit {
        return Err(NativePixelError::ResourceLimitExceeded {
            resource,
            limit,
            requested,
        });
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = StreamingSha256::new();
    hasher.update(bytes);
    hasher.finish_hex()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePixelError {
    ZeroDimension,
    ArithmeticOverflow,
    AllocationFailed,
    InvalidResourceLimits,
    ResourceLimitExceeded {
        resource: &'static str,
        limit: u64,
        requested: u64,
    },
    InvalidStoredBits,
    StoredTypeMismatch,
    InvalidColorOrganization,
    InvalidYbrFull422,
    ValueCount {
        expected: u64,
        actual: u64,
    },
    StoredValueOutOfRange {
        index: usize,
        value: i64,
        minimum: i64,
        maximum: i64,
    },
    DeclaredRangeMismatch {
        declared_minimum: i64,
        declared_maximum: i64,
        observed_minimum: i64,
        observed_maximum: i64,
    },
    PaddingOutOfRange(i64),
    MissingPalette,
    UnexpectedPalette,
    InvalidPalette,
    PaletteIndexOutOfRange {
        index: usize,
        value: i64,
        minimum: i64,
        maximum: i64,
    },
    FrameHashCount {
        expected: usize,
        actual: usize,
    },
    FrameHashMismatch {
        frame_number: u32,
        expected: String,
        actual: String,
    },
    PatternValueOutOfRange(i64),
    InternalLengthMismatch,
}

impl fmt::Display for NativePixelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativePixelError {}
