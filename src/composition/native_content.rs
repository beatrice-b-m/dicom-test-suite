use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
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
    spans
        .iter()
        .map(|span| {
            let start = usize::try_from(span.bit_offset / 8).map_err(|_| RawContentError::Range)?;
            let length =
                usize::try_from(span.bit_length / 8).map_err(|_| RawContentError::Range)?;
            let end = start.checked_add(length).ok_or(RawContentError::Range)?;
            let frame = bytes.get(start..end).ok_or(RawContentError::Range)?;
            Ok(crate::sha256_hex(frame))
        })
        .collect()
}

const HASH_BUFFER_BYTES: usize = 64 * 1024;

fn hash_staged_frames(
    path: &Path,
    frames: &[super::FrameSpan],
) -> Result<Vec<String>, RawContentError> {
    let mut file = File::open(path).map_err(|source| RawContentError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    frames
        .iter()
        .map(|frame| hash_staged_frame(&mut file, path, frame))
        .collect()
}

fn hash_staged_frame(
    file: &mut File,
    path: &Path,
    frame: &super::FrameSpan,
) -> Result<String, RawContentError> {
    let mut hasher = super::content::StreamingSha256::new();
    if frame.bit_offset % 8 == 0 && frame.bit_length % 8 == 0 {
        file.seek(SeekFrom::Start(frame.first_byte_offset))
            .map_err(|source| RawContentError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let mut remaining = frame.bit_length / 8;
        let mut buffer = [0_u8; HASH_BUFFER_BYTES];
        while remaining > 0 {
            let take = usize::try_from(remaining.min(HASH_BUFFER_BYTES as u64))
                .map_err(|_| RawContentError::Range)?;
            file.read_exact(&mut buffer[..take])
                .map_err(|source| RawContentError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            hasher.update(&buffer[..take]);
            remaining -= take as u64;
        }
        return Ok(hasher.finish_hex());
    }

    let first_source_byte = frame.bit_offset / 8;
    file.seek(SeekFrom::Start(first_source_byte))
        .map_err(|source| RawContentError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut source = [0_u8; 1];
    file.read_exact(&mut source)
        .map_err(|source| RawContentError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut source_byte_index = first_source_byte;
    let mut output_byte = 0_u8;
    for destination_bit in 0..frame.bit_length {
        let source_bit = frame
            .bit_offset
            .checked_add(destination_bit)
            .ok_or(RawContentError::Range)?;
        let wanted_source_byte = source_bit / 8;
        if wanted_source_byte != source_byte_index {
            file.read_exact(&mut source)
                .map_err(|source| RawContentError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            source_byte_index = wanted_source_byte;
        }
        let bit = source[0] >> (source_bit % 8) & 1;
        output_byte |= bit << (destination_bit % 8);
        if destination_bit % 8 == 7 {
            hasher.update(&[output_byte]);
            output_byte = 0;
        }
    }
    if frame.bit_length % 8 != 0 {
        hasher.update(&[output_byte]);
    }
    Ok(hasher.finish_hex())
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
