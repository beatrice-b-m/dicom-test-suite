//! Pure execution-local resolution of already encoded frame payloads.
//!
//! This module owns fragmentation, padding, offset-table arithmetic, and the
//! corresponding immutable-plan patch. It performs no filesystem or service
//! invocation, so planning previews and execution materialization can share
//! exactly one transform.

use crate::composition::{
    AttributeAddress, AttributeValue, ContentMaterialization, DicomVr, ResolvedAttribute,
    ResolvedInstancePlan, ValueOrigin,
};
use crate::corpus_plan::{EncodingPlan, FragmentationPolicy, OffsetTablePolicy};
use crate::encapsulation::{
    BasicOffsetTablePolicy, EncapsulatedPixelData, ExtendedOffsetTable, encapsulate_frames,
    serialize_ov_words_little_endian,
};
use crate::sha256_hex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedSlotInput {
    pub slot: String,
    pub ordered_frames: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEncodedSlot {
    pub slot: String,
    pub basic_offset_table: Vec<u32>,
    pub compressed_frame_sha256: Vec<String>,
    pub fragments: Vec<Vec<u8>>,
    pub fragment_count: u64,
    pub compressed_lengths: Vec<u64>,
    pub padded_fragment_lengths: Vec<u64>,
    pub fragments_per_frame: Vec<u64>,
    pub fragment_evidence: Vec<ResolvedFragment>,
    pub extended_offset_table: Vec<u64>,
    pub extended_offset_table_lengths: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFragment {
    pub frame_index: u64,
    pub item_start_offset: u64,
    pub compressed_length: u64,
    pub padded_length: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEncodedContent {
    pub instance: ResolvedInstancePlan,
    pub slots: Vec<ResolvedEncodedSlot>,
}

pub fn resolve_encoded_content(
    instance: &ResolvedInstancePlan,
    encoding: &EncodingPlan,
    inputs: &[EncodedSlotInput],
) -> Result<ResolvedEncodedContent, String> {
    let mut instance = instance.clone();
    let mut slots = Vec::with_capacity(inputs.len());
    let mut extended_values = None;
    for input in inputs {
        let content = instance
            .content
            .iter_mut()
            .find(|content| content.slot == input.slot)
            .ok_or_else(|| format!("encoded slot {} is absent", input.slot))?;
        let encapsulated = resolve_frames(encoding, &input.ordered_frames)?;
        if let Some(table) = &encapsulated.extended_offset_table {
            if extended_values.is_some() {
                return Err("multiple extended-offset-table slots are unsupported".into());
            }
            extended_values = Some((
                table.offset_value_bytes.clone(),
                table.length_value_bytes.clone(),
            ));
        }
        let mut fragments = encapsulated.fragment_payloads.clone();
        for fragment in &mut fragments {
            if fragment.len() % 2 != 0 {
                fragment.push(0);
            }
        }
        let aggregate = fragments.concat();
        content.kind = "encapsulated_pixels".into();
        content.vr = DicomVr::OB;
        content.size_bytes = aggregate.len() as u64;
        content.sha256 = sha256_hex(&aggregate);
        content.properties.insert(
            "compressed_frame_sha256".into(),
            serde_json::to_string(&encapsulated.compressed_frame_hashes)
                .map_err(|error| error.to_string())?,
        );
        content.materialization = Some(ContentMaterialization::Encapsulated {
            basic_offset_table: encapsulated.basic_offset_table.offsets.clone(),
            fragments: fragments.clone(),
        });
        slots.push(ResolvedEncodedSlot {
            slot: input.slot.clone(),
            basic_offset_table: encapsulated.basic_offset_table.offsets.clone(),
            compressed_frame_sha256: encapsulated.compressed_frame_hashes.clone(),
            fragment_count: encapsulated.fragments.len() as u64,
            compressed_lengths: encapsulated
                .fragments
                .iter()
                .map(|fragment| fragment.compressed_length as u64)
                .collect(),
            padded_fragment_lengths: encapsulated
                .fragments
                .iter()
                .map(|fragment| fragment.padded_length as u64)
                .collect(),
            fragments_per_frame: encapsulated
                .fragments_per_frame
                .iter()
                .map(|count| *count as u64)
                .collect(),
            fragment_evidence: encapsulated
                .fragments
                .iter()
                .map(|fragment| ResolvedFragment {
                    frame_index: fragment.frame_index as u64,
                    item_start_offset: fragment.item_start_offset as u64,
                    compressed_length: fragment.compressed_length as u64,
                    padded_length: fragment.padded_length as u64,
                })
                .collect(),
            extended_offset_table: encapsulated
                .extended_offset_table
                .as_ref()
                .map(|table| table.offsets.clone())
                .unwrap_or_default(),
            extended_offset_table_lengths: encapsulated
                .extended_offset_table
                .as_ref()
                .map(|table| table.lengths.clone())
                .unwrap_or_default(),
            fragments,
        });
    }
    if let Some((offsets, lengths)) = extended_values {
        upsert_binary_attribute(&mut instance, "7FE0,0001", DicomVr::OV, offsets)?;
        upsert_binary_attribute(&mut instance, "7FE0,0002", DicomVr::OV, lengths)?;
    }
    Ok(ResolvedEncodedContent { instance, slots })
}

fn resolve_frames(
    encoding: &EncodingPlan,
    encoded_frames: &[Vec<u8>],
) -> Result<EncapsulatedPixelData, String> {
    let policy = match encoding.offset_table {
        OffsetTablePolicy::PopulatedBasic => BasicOffsetTablePolicy::Populated,
        OffsetTablePolicy::EmptyBasic | OffsetTablePolicy::Extended => {
            BasicOffsetTablePolicy::Empty
        }
        OffsetTablePolicy::NotApplicable => {
            return Err("encoded content requires an offset-table policy".into());
        }
    };
    let mut result = match encoding.fragmentation {
        FragmentationPolicy::OneFragmentPerFrame | FragmentationPolicy::PreserveEncodedFrames => {
            if encoding.offset_table == OffsetTablePolicy::Extended {
                EncapsulatedPixelData::one_fragment_per_frame_with_extended_offset_table(
                    encoded_frames,
                )
            } else {
                EncapsulatedPixelData::one_fragment_per_frame(encoded_frames, policy)
            }
        }
        FragmentationPolicy::FixedMaximumBytes { maximum_bytes } => {
            let maximum =
                usize::try_from(maximum_bytes).map_err(|_| "fragment maximum exceeds usize")?;
            let even_maximum = maximum & !1;
            let counts = encoded_frames
                .iter()
                .map(|frame| {
                    if frame.len() <= maximum {
                        return Ok(1);
                    }
                    if even_maximum == 0 {
                        return Err("fragment maximum is too small");
                    }
                    frame
                        .len()
                        .checked_add(even_maximum - 1)
                        .map(|value| value / even_maximum)
                        .ok_or("fragment count overflow")
                })
                .collect::<Result<Vec<_>, _>>()?;
            encapsulate_frames(encoded_frames, &counts, policy)
        }
        FragmentationPolicy::FixedFragmentsPerFrame {
            fragments_per_frame,
        } => {
            if fragments_per_frame == 0 {
                return Err("fragments per frame is zero".into());
            }
            let count =
                usize::try_from(fragments_per_frame).map_err(|_| "fragment count exceeds usize")?;
            encapsulate_frames(encoded_frames, &vec![count; encoded_frames.len()], policy)
        }
        FragmentationPolicy::Native => {
            return Err("encoded frames cannot use native fragmentation".into());
        }
    }
    .map_err(|error| error.to_string())?;
    if encoding.offset_table == OffsetTablePolicy::Extended
        && result.extended_offset_table.is_none()
    {
        let first_start = result
            .fragments
            .first()
            .ok_or("encoded content has no fragments")?
            .item_start_offset;
        let mut offsets = Vec::with_capacity(result.fragments_per_frame.len());
        let mut lengths = Vec::with_capacity(result.fragments_per_frame.len());
        let mut index = 0usize;
        for count in &result.fragments_per_frame {
            let first = result
                .fragments
                .get(index)
                .ok_or("fragment cardinality drift")?;
            offsets.push(u64::from(
                first
                    .item_start_offset
                    .checked_sub(first_start)
                    .ok_or("fragment offset underflow")?,
            ));
            let mut length = 0_u64;
            for _ in 0..*count {
                let fragment = result
                    .fragments
                    .get(index)
                    .ok_or("fragment cardinality drift")?;
                length = length
                    .checked_add(fragment.compressed_length as u64)
                    .ok_or("fragment length overflow")?;
                index += 1;
            }
            lengths.push(length);
        }
        result.extended_offset_table = Some(ExtendedOffsetTable {
            offset_value_bytes: serialize_ov_words_little_endian(&offsets),
            length_value_bytes: serialize_ov_words_little_endian(&lengths),
            offsets,
            lengths,
        });
    }
    Ok(result)
}

fn upsert_binary_attribute(
    instance: &mut ResolvedInstancePlan,
    tag: &str,
    vr: DicomVr,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let address = AttributeAddress::from_normalized_tag(tag).map_err(|error| error.to_string())?;
    if instance
        .content
        .iter()
        .any(|content| content.address == address)
    {
        return Err(format!(
            "encoding-owned attribute {tag} conflicts with canonical content"
        ));
    }
    let attribute = ResolvedAttribute {
        address: address.clone(),
        vr,
        value: Some(AttributeValue::Binary(bytes)),
        origin: ValueOrigin::InstanceOverride,
    };
    if let Some(existing) = instance
        .attributes
        .iter_mut()
        .find(|existing| existing.address == address)
    {
        *existing = attribute;
    } else {
        instance.attributes.push(attribute);
        instance
            .attributes
            .sort_by(|left, right| left.address.cmp(&right.address));
    }
    Ok(())
}
