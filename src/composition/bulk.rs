use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{AttributeAddress, CanonicalContent, ContentMaterialization, DicomVr, StagedAsset};
use crate::sha256_hex;

/// The semantic class of a non-sequence DICOM value carried outside the
/// ordinary typed-attribute layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulkDataKind {
    Pixels,
    WaveformSamples,
    EncapsulatedDocument,
    Mesh,
    BackendProduced,
}

impl BulkDataKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pixels => "pixels",
            Self::WaveformSamples => "waveform_samples",
            Self::EncapsulatedDocument => "encapsulated_document",
            Self::Mesh => "mesh",
            Self::BackendProduced => "backend_produced",
        }
    }
}

/// Provenance of the exact bytes represented by a [`BulkDataPlan`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BulkDataSource {
    DefaultSynthetic,
    LocalFile {
        spec_relative_path: String,
    },
    InlineSmallFixture,
    Backend {
        provider_id: String,
        provider_version: String,
    },
}

/// Bounds established before a payload can be accepted for publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkDataBounds {
    pub minimum_bytes: u64,
    pub maximum_bytes: u64,
    pub byte_multiple: u64,
}

impl BulkDataBounds {
    pub const fn exact(bytes: u64) -> Self {
        Self {
            minimum_bytes: bytes,
            maximum_bytes: bytes,
            byte_multiple: 1,
        }
    }

    pub const fn bounded(minimum_bytes: u64, maximum_bytes: u64, byte_multiple: u64) -> Self {
        Self {
            minimum_bytes,
            maximum_bytes,
            byte_multiple,
        }
    }

    fn validate(self, actual: u64) -> Result<(), BulkDataError> {
        if self.minimum_bytes > self.maximum_bytes || self.byte_multiple == 0 {
            return Err(BulkDataError::InvalidBounds);
        }
        if actual < self.minimum_bytes || actual > self.maximum_bytes {
            return Err(BulkDataError::SizeOutOfBounds {
                actual,
                minimum: self.minimum_bytes,
                maximum: self.maximum_bytes,
            });
        }
        if actual % self.byte_multiple != 0 {
            return Err(BulkDataError::SizeAlignment {
                actual,
                multiple: self.byte_multiple,
            });
        }
        Ok(())
    }
}

/// Canonical, hash-addressed plan for one DICOM bulk value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkDataPlan {
    pub slot: String,
    pub kind: BulkDataKind,
    pub address: AttributeAddress,
    pub vr: DicomVr,
    pub size_bytes: u64,
    pub sha256: String,
    pub bounds: BulkDataBounds,
    pub source: BulkDataSource,
    pub properties: BTreeMap<String, String>,
    #[serde(skip)]
    pub materialization: Option<ContentMaterialization>,
}

impl BulkDataPlan {
    pub fn from_bytes<S: TypedBulkDataSlot>(
        bytes: Vec<u8>,
        vr: DicomVr,
        bounds: BulkDataBounds,
        source: BulkDataSource,
        properties: BTreeMap<String, String>,
    ) -> Result<Self, BulkDataError> {
        let size_bytes = u64::try_from(bytes.len()).map_err(|_| BulkDataError::SizeOverflow)?;
        let plan = Self {
            slot: S::SLOT.to_string(),
            kind: S::KIND,
            address: AttributeAddress::from_normalized_tag(S::TAG)
                .map_err(|_| BulkDataError::InvalidSlotContract(S::SLOT))?,
            vr,
            size_bytes,
            sha256: sha256_hex(&bytes),
            bounds,
            source,
            properties,
            materialization: Some(ContentMaterialization::Inline(bytes)),
        };
        plan.validate_for::<S>()?;
        Ok(plan)
    }

    pub fn from_staged<S: TypedBulkDataSlot>(
        asset: StagedAsset,
        vr: DicomVr,
        bounds: BulkDataBounds,
        mut properties: BTreeMap<String, String>,
    ) -> Result<Self, BulkDataError> {
        let source_path = asset.spec_relative_path.clone();
        properties.insert("spec_relative_path".into(), source_path.clone());
        let plan = Self {
            slot: S::SLOT.to_string(),
            kind: S::KIND,
            address: AttributeAddress::from_normalized_tag(S::TAG)
                .map_err(|_| BulkDataError::InvalidSlotContract(S::SLOT))?,
            vr,
            size_bytes: asset.size_bytes,
            sha256: asset.sha256,
            bounds,
            source: BulkDataSource::LocalFile {
                spec_relative_path: source_path,
            },
            properties,
            materialization: Some(ContentMaterialization::StagedFile(asset.staged_path)),
        };
        plan.validate_for::<S>()?;
        Ok(plan)
    }

    /// Construct a provider declaration before bytes are materialized. The
    /// writer will reject it until the provider response supplies matching
    /// materialization.
    pub fn backend_declaration<S: TypedBulkDataSlot>(
        vr: DicomVr,
        size_bytes: u64,
        sha256: String,
        bounds: BulkDataBounds,
        provider_id: String,
        provider_version: String,
        properties: BTreeMap<String, String>,
    ) -> Result<Self, BulkDataError> {
        if provider_id.is_empty() || provider_version.is_empty() {
            return Err(BulkDataError::InvalidProviderProvenance);
        }
        let plan = Self {
            slot: S::SLOT.to_string(),
            kind: BulkDataKind::BackendProduced,
            address: AttributeAddress::from_normalized_tag(S::TAG)
                .map_err(|_| BulkDataError::InvalidSlotContract(S::SLOT))?,
            vr,
            size_bytes,
            sha256,
            bounds,
            source: BulkDataSource::Backend {
                provider_id,
                provider_version,
            },
            properties,
            materialization: None,
        };
        plan.validate_common()?;
        S::validate_destination(&plan)?;
        Ok(plan)
    }

    pub fn validate_for<S: TypedBulkDataSlot>(&self) -> Result<(), BulkDataError> {
        self.validate_common()?;
        if self.slot != S::SLOT || self.address.normalized_tag() != S::TAG {
            return Err(BulkDataError::SlotMismatch {
                expected: S::SLOT,
                actual: self.slot.clone(),
            });
        }
        if self.kind != S::KIND && self.kind != BulkDataKind::BackendProduced {
            return Err(BulkDataError::KindMismatch {
                expected: S::KIND,
                actual: self.kind,
            });
        }
        S::validate_destination(self)
    }

    fn validate_common(&self) -> Result<(), BulkDataError> {
        if self.slot.is_empty() {
            return Err(BulkDataError::EmptySlot);
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(BulkDataError::InvalidSha256(self.sha256.clone()));
        }
        self.bounds.validate(self.size_bytes)
    }

    pub fn into_canonical_content(self) -> CanonicalContent {
        let mut properties = self.properties;
        properties.insert(
            "bulk_bounds".into(),
            serde_json::to_string(&self.bounds).expect("bulk bounds serialize"),
        );
        properties.insert(
            "bulk_source".into(),
            serde_json::to_string(&self.source).expect("bulk source serializes"),
        );
        CanonicalContent {
            slot: self.slot,
            kind: self.kind.as_str().to_string(),
            address: self.address,
            vr: self.vr,
            size_bytes: self.size_bytes,
            sha256: self.sha256,
            properties,
            placement: super::ContentPlacement::TopLevel,
            materialization: self.materialization,
        }
    }
}

/// A sealed-by-convention semantic contract between a template content slot
/// and the DICOM element/VRs that may carry it.
pub trait TypedBulkDataSlot {
    const SLOT: &'static str;
    const KIND: BulkDataKind;
    const TAG: &'static str;
    const ALLOWED_VRS: &'static [DicomVr];

    fn validate_destination(plan: &BulkDataPlan) -> Result<(), BulkDataError> {
        if Self::ALLOWED_VRS.contains(&plan.vr) {
            Ok(())
        } else {
            Err(BulkDataError::InvalidVr {
                slot: Self::SLOT,
                vr: plan.vr,
            })
        }
    }
}

/// Marker contract used by the external-provider layer. A backend may only
/// answer a request for a slot that already has a typed DICOM destination.
pub trait BackendProducedBulkDataSlot: TypedBulkDataSlot {}

impl<T: TypedBulkDataSlot> BackendProducedBulkDataSlot for T {}

macro_rules! bulk_slot {
    ($name:ident, $slot:literal, $kind:ident, $tag:literal, [$($vr:ident),+ $(,)?]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;
        impl TypedBulkDataSlot for $name {
            const SLOT: &'static str = $slot;
            const KIND: BulkDataKind = BulkDataKind::$kind;
            const TAG: &'static str = $tag;
            const ALLOWED_VRS: &'static [DicomVr] = &[$(DicomVr::$vr),+];
        }
    };
}

bulk_slot!(PixelDataSlot, "pixels", Pixels, "7FE0,0010", [OB, OW]);
bulk_slot!(FloatPixelDataSlot, "pixels", Pixels, "7FE0,0008", [OF]);
bulk_slot!(
    DoubleFloatPixelDataSlot,
    "pixels",
    Pixels,
    "7FE0,0009",
    [OD]
);
bulk_slot!(
    WaveformSamplesSlot,
    "waveform_samples",
    WaveformSamples,
    "5400,1010",
    [OB, OW]
);
bulk_slot!(
    EncapsulatedDocumentSlot,
    "document",
    EncapsulatedDocument,
    "0042,0011",
    [OB]
);
bulk_slot!(MeshSlot, "mesh", Mesh, "0042,0011", [OB]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkDataError {
    EmptySlot,
    InvalidBounds,
    SizeOverflow,
    SizeOutOfBounds {
        actual: u64,
        minimum: u64,
        maximum: u64,
    },
    SizeAlignment {
        actual: u64,
        multiple: u64,
    },
    InvalidSha256(String),
    InvalidProviderProvenance,
    InvalidSlotContract(&'static str),
    SlotMismatch {
        expected: &'static str,
        actual: String,
    },
    KindMismatch {
        expected: BulkDataKind,
        actual: BulkDataKind,
    },
    InvalidVr {
        slot: &'static str,
        vr: DicomVr,
    },
}

impl fmt::Display for BulkDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BulkDataError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_slots_bind_payloads_to_safe_dicom_destinations() {
        let waveform = BulkDataPlan::from_bytes::<WaveformSamplesSlot>(
            vec![0, 1, 2, 3],
            DicomVr::OW,
            BulkDataBounds::bounded(4, 4, 2),
            BulkDataSource::DefaultSynthetic,
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(waveform.address.normalized_tag(), "5400,1010");
        assert_eq!(waveform.size_bytes, 4);
        assert_eq!(waveform.sha256.len(), 64);

        assert!(matches!(
            BulkDataPlan::from_bytes::<WaveformSamplesSlot>(
                vec![0, 1, 2, 3],
                DicomVr::OB,
                BulkDataBounds::bounded(4, 4, 3),
                BulkDataSource::DefaultSynthetic,
                BTreeMap::new(),
            ),
            Err(BulkDataError::SizeAlignment { .. })
        ));
        assert!(matches!(
            BulkDataPlan::from_bytes::<MeshSlot>(
                b"solid mesh".to_vec(),
                DicomVr::OW,
                BulkDataBounds::exact(10),
                BulkDataSource::InlineSmallFixture,
                BTreeMap::new(),
            ),
            Err(BulkDataError::InvalidVr { .. })
        ));
    }

    #[test]
    fn backend_declarations_require_hash_size_and_provider_provenance() {
        let declaration = BulkDataPlan::backend_declaration::<PixelDataSlot>(
            DicomVr::OB,
            8,
            "a".repeat(64),
            BulkDataBounds::exact(8),
            "fixture-provider".into(),
            "1".into(),
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(declaration.kind, BulkDataKind::BackendProduced);
        assert!(declaration.materialization.is_none());
        assert!(matches!(declaration.source, BulkDataSource::Backend { .. }));
    }
}
