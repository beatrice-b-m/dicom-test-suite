use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use super::{
    AttributeAddress, CanonicalContent, ContentError, DicomVr, LocalContentResolver,
    NativePixelPlan, PixelError, PixelShape, StagedAsset,
};

#[cfg(test)]
use crate::sha256_hex;
#[cfg(test)]
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNativePixelOutput {
    pub plan: NativePixelPlan,
    pub content: CanonicalContent,
    pub frame_sha256: Vec<String>,
}

pub fn resolve_raw_native_pixels(
    resolver: &mut LocalContentResolver,
    relative_path: impl AsRef<Path>,
    expected_sha256: Option<&str>,
    shape: PixelShape,
) -> Result<RawNativePixelOutput, RawContentError> {
    let asset = resolver.resolve(
        "pixels",
        "native_pixels",
        relative_path.as_ref(),
        expected_sha256,
    )?;
    resolve_staged_native_pixels(asset, shape)
}

pub(crate) fn resolve_staged_native_pixels(
    asset: StagedAsset,
    shape: PixelShape,
) -> Result<RawNativePixelOutput, RawContentError> {
    let plan = NativePixelPlan::plan(shape)?;
    if asset.size_bytes != plan.unpadded_value_bytes {
        return Err(RawContentError::Length {
            path: asset.spec_relative_path,
            expected: plan.unpadded_value_bytes,
            actual: asset.size_bytes,
        });
    }
    let frame_sha256 = match &asset.inline_bytes {
        Some(bytes) => hash_inline_frames(bytes, &plan.frame_spans)?,
        None => hash_staged_frames(&asset.staged_path, &plan.frame_spans)?,
    };
    let mut content = asset.into_canonical_content(
        AttributeAddress::from_normalized_tag("7FE0,0010").expect("Pixel Data is a known tag"),
        if plan.shape.bits_allocated <= 8 {
            DicomVr::OB
        } else {
            DicomVr::OW
        },
    );
    content.properties.extend(BTreeMap::from([
        (
            "frame_sha256".into(),
            serde_json::to_string(&frame_sha256).expect("frame hashes serialize"),
        ),
        (
            "pixel_shape".into(),
            serde_json::to_string(&plan.shape).expect("pixel shape serializes"),
        ),
    ]));
    Ok(RawNativePixelOutput {
        plan,
        content,
        frame_sha256,
    })
}

fn hash_inline_frames(
    bytes: &[u8],
    spans: &[crate::composition::FrameSpan],
) -> Result<Vec<String>, RawContentError> {
    let source_length = u64::try_from(bytes.len()).map_err(|_| RawContentError::Range)?;
    let mut reader = Cursor::new(bytes);
    crate::native_pixel::hash_native_frames(&mut reader, source_length, &neutral_frame_spans(spans))
        .map_err(|_| RawContentError::Range)
}

fn hash_staged_frames(
    path: &Path,
    frames: &[super::FrameSpan],
) -> Result<Vec<String>, RawContentError> {
    let mut file = File::open(path).map_err(|source| RawContentError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let source_length = file
        .metadata()
        .map_err(|source| RawContentError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    crate::native_pixel::hash_native_frames(&mut file, source_length, &neutral_frame_spans(frames))
        .map_err(|error| match error {
            crate::native_pixel::NativeFrameHashError::Io(source) => RawContentError::Io {
                path: path.to_path_buf(),
                source,
            },
            crate::native_pixel::NativeFrameHashError::InvalidSpan
            | crate::native_pixel::NativeFrameHashError::ArithmeticOverflow
            | crate::native_pixel::NativeFrameHashError::Range => RawContentError::Range,
        })
}

fn neutral_frame_spans(frames: &[super::FrameSpan]) -> Vec<crate::native_pixel::FrameHashSpan> {
    frames
        .iter()
        .map(|frame| crate::native_pixel::FrameHashSpan {
            frame_number: frame.frame_number,
            bit_offset: frame.bit_offset,
            bit_length: frame.bit_length,
        })
        .collect()
}

#[derive(Debug)]
pub enum RawContentError {
    Content(ContentError),
    Pixel(PixelError),
    Length {
        path: String,
        expected: u64,
        actual: u64,
    },
    Range,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl From<ContentError> for RawContentError {
    fn from(error: ContentError) -> Self {
        Self::Content(error)
    }
}

impl From<PixelError> for RawContentError {
    fn from(error: PixelError) -> Self {
        Self::Pixel(error)
    }
}

impl fmt::Display for RawContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RawContentError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::composition::{
        ByteOrder, ContentLimits, PhotometricInterpretation, PlanarConfiguration, SampleType,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn roots(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "dts-raw-native-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let spec = root.join("spec");
        let staging = root.join("staging");
        fs::create_dir_all(&spec).unwrap();
        fs::create_dir_all(&staging).unwrap();
        (root, spec, staging)
    }

    fn resolver(spec: &Path, staging: &Path) -> LocalContentResolver {
        LocalContentResolver::new(
            spec,
            staging,
            ContentLimits {
                max_files: 2,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        )
        .unwrap()
    }

    fn read_materialized(content: &CanonicalContent) -> Vec<u8> {
        match content.materialization.as_ref().unwrap() {
            super::super::ContentMaterialization::Inline(bytes) => bytes.clone(),
            super::super::ContentMaterialization::StagedFile(path) => fs::read(path).unwrap(),
            other => panic!("unexpected native pixel materialization {other:?}"),
        }
    }

    fn inline_and_staged(
        label: &str,
        bytes: &[u8],
        shape: PixelShape,
    ) -> (PathBuf, RawNativePixelOutput, RawNativePixelOutput) {
        let (root, spec, staging) = roots(label);
        let limits = ContentLimits {
            max_files: 2,
            max_file_bytes: 4096,
            max_total_bytes: 8192,
        };
        let mut inline_resolver = LocalContentResolver::new_read_only(&spec, limits).unwrap();
        let inline_asset = inline_resolver
            .resolve_inline("pixels", "native_pixels", bytes, Some(&sha256_hex(bytes)))
            .unwrap();
        let inline = resolve_staged_native_pixels(inline_asset, shape.clone()).unwrap();

        let mut staged_resolver = LocalContentResolver::new(&spec, &staging, limits).unwrap();
        let staged_asset = staged_resolver
            .resolve_inline("pixels", "native_pixels", bytes, Some(&sha256_hex(bytes)))
            .unwrap();
        let staged = resolve_staged_native_pixels(staged_asset, shape).unwrap();
        (root, inline, staged)
    }

    #[test]
    fn monochrome_multiframe_hashes_each_exact_frame() {
        let (root, spec, staging) = roots("mono");
        let bytes = vec![0, 1, 2, 3, 4, 5, 6, 7];
        fs::write(spec.join("pixels.raw"), &bytes).unwrap();
        let output = resolve_raw_native_pixels(
            &mut resolver(&spec, &staging),
            "pixels.raw",
            Some(&sha256_hex(&bytes)),
            PixelShape {
                rows: 2,
                columns: 2,
                frames: 2,
                samples_per_pixel: 1,
                photometric_interpretation: PhotometricInterpretation::Monochrome2,
                sample_type: SampleType::UnsignedInteger,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                byte_order: ByteOrder::Little,
                planar_configuration: None,
            },
        )
        .unwrap();
        assert_eq!(
            output.frame_sha256,
            vec![sha256_hex(&bytes[..4]), sha256_hex(&bytes[4..])]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn single_bit_frames_hash_canonical_bits_across_byte_boundaries() {
        let (root, spec, staging) = roots("bit1");
        let bytes = vec![0b1010_0101, 0b0110_0011, 0b0000_0011];
        fs::write(spec.join("pixels.raw"), &bytes).unwrap();
        let output = resolve_raw_native_pixels(
            &mut resolver(&spec, &staging),
            "pixels.raw",
            Some(&sha256_hex(&bytes)),
            PixelShape {
                rows: 3,
                columns: 3,
                frames: 2,
                samples_per_pixel: 1,
                photometric_interpretation: PhotometricInterpretation::Monochrome2,
                sample_type: SampleType::Bit1,
                bits_allocated: 1,
                bits_stored: 1,
                high_bit: 0,
                byte_order: ByteOrder::Little,
                planar_configuration: None,
            },
        )
        .unwrap();
        assert_eq!(output.plan.unpadded_value_bytes, 3);
        assert_eq!(
            output.frame_sha256,
            vec![
                sha256_hex(&[0b1010_0101, 0b0000_0001]),
                sha256_hex(&[0b1011_0001, 0b0000_0001]),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inline_and_staged_u1_frames_have_identical_canonical_hashes_and_bytes() {
        let bytes = vec![0b1010_0101, 0b0110_0011, 0b0000_0011];
        let (root, inline, staged) = inline_and_staged(
            "bit1-equivalence",
            &bytes,
            PixelShape {
                rows: 3,
                columns: 3,
                frames: 2,
                samples_per_pixel: 1,
                photometric_interpretation: PhotometricInterpretation::Monochrome2,
                sample_type: SampleType::Bit1,
                bits_allocated: 1,
                bits_stored: 1,
                high_bit: 0,
                byte_order: ByteOrder::Little,
                planar_configuration: None,
            },
        );
        let expected = vec![
            sha256_hex(&[0b1010_0101, 0b0000_0001]),
            sha256_hex(&[0b1011_0001, 0b0000_0001]),
        ];
        assert_eq!(inline.frame_sha256, expected);
        assert_eq!(staged.frame_sha256, expected);
        assert_eq!(read_materialized(&inline.content), bytes);
        assert_eq!(read_materialized(&staged.content), bytes);
        assert_eq!(inline.content.sha256, staged.content.sha256);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inline_and_staged_planar_color_frames_preserve_exact_hashes_and_bytes() {
        let bytes = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 255, 255, 0, 255, 0, 255, 0, 255,
            255, 0, 0,
        ];
        let (root, inline, staged) = inline_and_staged(
            "rgb-planar-equivalence",
            &bytes,
            PixelShape {
                rows: 2,
                columns: 2,
                frames: 2,
                samples_per_pixel: 3,
                photometric_interpretation: PhotometricInterpretation::Rgb,
                sample_type: SampleType::UnsignedInteger,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                byte_order: ByteOrder::Little,
                planar_configuration: Some(PlanarConfiguration::Planar),
            },
        );
        let expected = vec![sha256_hex(&bytes[..12]), sha256_hex(&bytes[12..])];
        assert_eq!(inline.frame_sha256, expected);
        assert_eq!(staged.frame_sha256, expected);
        assert_eq!(read_materialized(&inline.content), bytes);
        assert_eq!(read_materialized(&staged.content), bytes);
        assert_eq!(inline.content.sha256, staged.content.sha256);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rgb_round_trip_contract_rejects_wrong_length() {
        let (root, spec, staging) = roots("rgb");
        fs::write(spec.join("pixels.raw"), [0_u8; 11]).unwrap();
        let error = resolve_raw_native_pixels(
            &mut resolver(&spec, &staging),
            "pixels.raw",
            None,
            PixelShape {
                rows: 2,
                columns: 2,
                frames: 1,
                samples_per_pixel: 3,
                photometric_interpretation: PhotometricInterpretation::Rgb,
                sample_type: SampleType::UnsignedInteger,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                byte_order: ByteOrder::Little,
                planar_configuration: Some(PlanarConfiguration::Interleaved),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RawContentError::Length {
                expected: 12,
                actual: 11,
                ..
            }
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
