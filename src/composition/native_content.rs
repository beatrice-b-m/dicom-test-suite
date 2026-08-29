use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    AttributeAddress, CanonicalContent, ContentError, DicomVr, LocalContentResolver,
    NativePixelPlan, PixelError, PixelShape,
};
use crate::sha256_hex;

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
    let plan = NativePixelPlan::plan(shape)?;
    if plan.shape.bits_allocated % 8 != 0 {
        return Err(RawContentError::NonByteAligned);
    }
    let asset = resolver.resolve(
        "pixels",
        "native_pixels",
        relative_path.as_ref(),
        expected_sha256,
    )?;
    if asset.size_bytes != plan.unpadded_value_bytes {
        return Err(RawContentError::Length {
            path: asset.spec_relative_path,
            expected: plan.unpadded_value_bytes,
            actual: asset.size_bytes,
        });
    }
    let bytes = fs::read(&asset.staged_path).map_err(|source| RawContentError::Io {
        path: asset.staged_path.clone(),
        source,
    })?;
    let mut frame_sha256 = Vec::with_capacity(plan.frame_spans.len());
    for frame in &plan.frame_spans {
        let start = usize::try_from(frame.first_byte_offset).map_err(|_| RawContentError::Range)?;
        let length = usize::try_from(frame.bit_length / 8).map_err(|_| RawContentError::Range)?;
        let end = start.checked_add(length).ok_or(RawContentError::Range)?;
        frame_sha256.push(sha256_hex(
            bytes.get(start..end).ok_or(RawContentError::Range)?,
        ));
    }
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

#[derive(Debug)]
pub enum RawContentError {
    Content(ContentError),
    Pixel(PixelError),
    NonByteAligned,
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
