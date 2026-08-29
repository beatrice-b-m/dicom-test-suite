use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleType {
    Bit1,
    UnsignedInteger,
    SignedInteger,
    Float32,
    Float64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanarConfiguration {
    Interleaved = 0,
    Planar = 1,
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
pub enum PixelElement {
    PixelData,
    FloatPixelData,
    DoubleFloatPixelData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelShape {
    pub rows: u32,
    pub columns: u32,
    pub frames: u32,
    pub samples_per_pixel: u8,
    pub photometric_interpretation: PhotometricInterpretation,
    pub sample_type: SampleType,
    pub bits_allocated: u8,
    pub bits_stored: u8,
    pub high_bit: u8,
    pub byte_order: ByteOrder,
    pub planar_configuration: Option<PlanarConfiguration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSpan {
    pub frame_number: u32,
    pub bit_offset: u64,
    pub bit_length: u64,
    pub first_byte_offset: u64,
    pub bytes_touched: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePixelPlan {
    pub shape: PixelShape,
    pub element: PixelElement,
    pub unpadded_value_bytes: u64,
    pub padded_value_bytes: u64,
    pub padding_bytes: u8,
    pub frame_spans: Vec<FrameSpan>,
}

impl NativePixelPlan {
    pub fn plan(shape: PixelShape) -> Result<Self, PixelError> {
        validate_shape(&shape)?;
        let pixels_per_frame = checked_mul(shape.rows as u64, shape.columns as u64)?;
        let bits_per_frame = match shape.photometric_interpretation {
            PhotometricInterpretation::YbrFull422 => checked_mul(
                checked_mul(pixels_per_frame, 2)?,
                shape.bits_allocated as u64,
            )?,
            _ => checked_mul(
                checked_mul(pixels_per_frame, shape.samples_per_pixel as u64)?,
                shape.bits_allocated as u64,
            )?,
        };
        let total_bits = checked_mul(bits_per_frame, shape.frames as u64)?;
        let unpadded_value_bytes = total_bits
            .checked_add(7)
            .ok_or(PixelError::ArithmeticOverflow)?
            / 8;
        let padding_bytes = (unpadded_value_bytes & 1) as u8;
        let padded_value_bytes = unpadded_value_bytes
            .checked_add(padding_bytes as u64)
            .ok_or(PixelError::ArithmeticOverflow)?;
        let mut frame_spans = Vec::with_capacity(shape.frames as usize);
        for frame_index in 0..shape.frames as u64 {
            let bit_offset = checked_mul(frame_index, bits_per_frame)?;
            let end_bit = bit_offset
                .checked_add(bits_per_frame)
                .ok_or(PixelError::ArithmeticOverflow)?;
            let first_byte_offset = bit_offset / 8;
            let end_byte = end_bit
                .checked_add(7)
                .ok_or(PixelError::ArithmeticOverflow)?
                / 8;
            frame_spans.push(FrameSpan {
                frame_number: frame_index as u32 + 1,
                bit_offset,
                bit_length: bits_per_frame,
                first_byte_offset,
                bytes_touched: end_byte - first_byte_offset,
            });
        }
        let element = match shape.sample_type {
            SampleType::Float32 => PixelElement::FloatPixelData,
            SampleType::Float64 => PixelElement::DoubleFloatPixelData,
            _ => PixelElement::PixelData,
        };
        Ok(Self {
            shape,
            element,
            unpadded_value_bytes,
            padded_value_bytes,
            padding_bytes,
            frame_spans,
        })
    }
}

fn validate_shape(shape: &PixelShape) -> Result<(), PixelError> {
    if shape.rows == 0 || shape.columns == 0 || shape.frames == 0 {
        return Err(PixelError::ZeroDimension);
    }
    if shape.bits_stored == 0
        || shape.bits_stored > shape.bits_allocated
        || shape.high_bit.checked_add(1) != Some(shape.bits_stored)
    {
        return Err(PixelError::InvalidStoredBits);
    }
    match shape.sample_type {
        SampleType::Bit1
            if shape.bits_allocated == 1 && shape.bits_stored == 1 && shape.high_bit == 0 => {}
        SampleType::UnsignedInteger | SampleType::SignedInteger
            if matches!(shape.bits_allocated, 8 | 16 | 32) => {}
        SampleType::Float32
            if shape.bits_allocated == 32 && shape.bits_stored == 32 && shape.high_bit == 31 => {}
        SampleType::Float64
            if shape.bits_allocated == 64 && shape.bits_stored == 64 && shape.high_bit == 63 => {}
        _ => return Err(PixelError::InvalidSampleType),
    }
    match shape.photometric_interpretation {
        PhotometricInterpretation::Monochrome1
        | PhotometricInterpretation::Monochrome2
        | PhotometricInterpretation::PaletteColor => {
            if shape.samples_per_pixel != 1 || shape.planar_configuration.is_some() {
                return Err(PixelError::InvalidColorOrganization);
            }
        }
        PhotometricInterpretation::Rgb | PhotometricInterpretation::YbrFull => {
            if shape.samples_per_pixel != 3 || shape.planar_configuration.is_none() {
                return Err(PixelError::InvalidColorOrganization);
            }
        }
        PhotometricInterpretation::YbrFull422 => {
            if shape.samples_per_pixel != 3
                || shape.planar_configuration != Some(PlanarConfiguration::Interleaved)
                || shape.columns % 2 != 0
            {
                return Err(PixelError::InvalidYbrFull422);
            }
        }
    }
    if matches!(shape.sample_type, SampleType::Float32 | SampleType::Float64)
        && (!matches!(
            shape.photometric_interpretation,
            PhotometricInterpretation::Monochrome1 | PhotometricInterpretation::Monochrome2
        ) || shape.samples_per_pixel != 1)
    {
        return Err(PixelError::InvalidFloatOrganization);
    }
    Ok(())
}

fn checked_mul(left: u64, right: u64) -> Result<u64, PixelError> {
    left.checked_mul(right)
        .ok_or(PixelError::ArithmeticOverflow)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelError {
    ZeroDimension,
    InvalidStoredBits,
    InvalidSampleType,
    InvalidColorOrganization,
    InvalidYbrFull422,
    InvalidFloatOrganization,
    ArithmeticOverflow,
}

impl fmt::Display for PixelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ZeroDimension => "pixel rows, columns, and frames must be non-zero",
            Self::InvalidStoredBits => "Bits Stored and High Bit are inconsistent with Bits Allocated",
            Self::InvalidSampleType => "sample type is inconsistent with Bits Allocated",
            Self::InvalidColorOrganization => "photometric interpretation, samples, and planar organization disagree",
            Self::InvalidYbrFull422 => "native YBR_FULL_422 requires even columns and interleaved three-sample organization",
            Self::InvalidFloatOrganization => "float pixels require one monochrome sample",
            Self::ArithmeticOverflow => "pixel length arithmetic overflow",
        })
    }
}

impl std::error::Error for PixelError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(sample_type: SampleType, bits: u8) -> PixelShape {
        PixelShape {
            rows: 3,
            columns: 5,
            frames: 2,
            samples_per_pixel: 1,
            photometric_interpretation: PhotometricInterpretation::Monochrome2,
            sample_type,
            bits_allocated: bits,
            bits_stored: bits,
            high_bit: bits - 1,
            byte_order: ByteOrder::Little,
            planar_configuration: None,
        }
    }

    #[test]
    fn plans_one_bit_continuously_across_frames_and_pads_only_value() {
        let plan = NativePixelPlan::plan(mono(SampleType::Bit1, 1)).unwrap();
        assert_eq!(plan.unpadded_value_bytes, 4);
        assert_eq!(plan.padded_value_bytes, 4);
        assert_eq!(plan.frame_spans[0].bit_offset, 0);
        assert_eq!(plan.frame_spans[1].bit_offset, 15);
        assert_eq!(plan.frame_spans[1].first_byte_offset, 1);
        assert_eq!(plan.frame_spans[1].bytes_touched, 3);
    }

    #[test]
    fn plans_integer_and_float_widths() {
        for (sample_type, bits, element, bytes) in [
            (SampleType::UnsignedInteger, 8, PixelElement::PixelData, 30),
            (SampleType::SignedInteger, 16, PixelElement::PixelData, 60),
            (
                SampleType::UnsignedInteger,
                32,
                PixelElement::PixelData,
                120,
            ),
            (SampleType::Float32, 32, PixelElement::FloatPixelData, 120),
            (
                SampleType::Float64,
                64,
                PixelElement::DoubleFloatPixelData,
                240,
            ),
        ] {
            let plan = NativePixelPlan::plan(mono(sample_type, bits)).unwrap();
            assert_eq!(plan.element, element);
            assert_eq!(plan.unpadded_value_bytes, bytes);
        }
    }

    #[test]
    fn plans_rgb_ybr_palette_and_planar_layouts() {
        for (photometric, planar) in [
            (
                PhotometricInterpretation::Rgb,
                PlanarConfiguration::Interleaved,
            ),
            (PhotometricInterpretation::Rgb, PlanarConfiguration::Planar),
            (
                PhotometricInterpretation::YbrFull,
                PlanarConfiguration::Interleaved,
            ),
            (
                PhotometricInterpretation::YbrFull,
                PlanarConfiguration::Planar,
            ),
        ] {
            let mut shape = mono(SampleType::UnsignedInteger, 8);
            shape.samples_per_pixel = 3;
            shape.photometric_interpretation = photometric;
            shape.planar_configuration = Some(planar);
            assert_eq!(
                NativePixelPlan::plan(shape).unwrap().unpadded_value_bytes,
                90
            );
        }
        let mut palette = mono(SampleType::UnsignedInteger, 8);
        palette.photometric_interpretation = PhotometricInterpretation::PaletteColor;
        assert_eq!(
            NativePixelPlan::plan(palette).unwrap().unpadded_value_bytes,
            30
        );
    }

    #[test]
    fn ybr_full_422_uses_special_length_and_even_columns() {
        let mut shape = mono(SampleType::UnsignedInteger, 8);
        shape.columns = 4;
        shape.samples_per_pixel = 3;
        shape.photometric_interpretation = PhotometricInterpretation::YbrFull422;
        shape.planar_configuration = Some(PlanarConfiguration::Interleaved);
        assert_eq!(
            NativePixelPlan::plan(shape.clone())
                .unwrap()
                .unpadded_value_bytes,
            48
        );
        shape.columns = 5;
        assert_eq!(
            NativePixelPlan::plan(shape),
            Err(PixelError::InvalidYbrFull422)
        );
    }

    #[test]
    fn rejects_invalid_bits_color_and_checked_overflow() {
        let mut bad_bits = mono(SampleType::UnsignedInteger, 16);
        bad_bits.bits_stored = 12;
        assert_eq!(
            NativePixelPlan::plan(bad_bits),
            Err(PixelError::InvalidStoredBits)
        );

        let mut bad_color = mono(SampleType::UnsignedInteger, 8);
        bad_color.samples_per_pixel = 3;
        assert_eq!(
            NativePixelPlan::plan(bad_color),
            Err(PixelError::InvalidColorOrganization)
        );

        let mut huge = mono(SampleType::Float64, 64);
        huge.rows = u32::MAX;
        huge.columns = u32::MAX;
        huge.frames = u32::MAX;
        assert_eq!(
            NativePixelPlan::plan(huge),
            Err(PixelError::ArithmeticOverflow)
        );
    }

    #[test]
    fn odd_total_value_length_gets_one_final_padding_byte() {
        let mut shape = mono(SampleType::UnsignedInteger, 8);
        shape.frames = 1;
        let plan = NativePixelPlan::plan(shape).unwrap();
        assert_eq!(plan.unpadded_value_bytes, 15);
        assert_eq!(plan.padding_bytes, 1);
        assert_eq!(plan.padded_value_bytes, 16);
    }
}
