//! Bounded byte locator for known-good Explicit VR Little Endian Part 10 files.
//!
//! This is intentionally not a general DICOM parser. It supports the native
//! Explicit VR Little Endian and RLE Lossless transfer syntaxes used as Phase 7
//! mutation sources, and reports source-coordinate ranges without interpreting
//! element values or allocating from declared lengths.

use std::error::Error;
use std::fmt;

pub const EXPLICIT_VR_LITTLE_ENDIAN_UID: &str = "1.2.840.10008.1.2.1";
pub const RLE_LOSSLESS_UID: &str = "1.2.840.10008.1.2.5";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(pub u16, pub u16);

pub const TRANSFER_SYNTAX_UID: Tag = Tag(0x0002, 0x0010);
pub const SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
pub const SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
pub const SPECIFIC_CHARACTER_SET: Tag = Tag(0x0008, 0x0005);
pub const BITS_STORED: Tag = Tag(0x0028, 0x0101);
pub const HIGH_BIT: Tag = Tag(0x0028, 0x0102);
pub const EXTENDED_OFFSET_TABLE: Tag = Tag(0x7fe0, 0x0001);
pub const EXTENDED_OFFSET_TABLE_LENGTHS: Tag = Tag(0x7fe0, 0x0002);
pub const PIXEL_DATA: Tag = Tag(0x7fe0, 0x0010);

const ITEM: Tag = Tag(0xfffe, 0xe000);
const ITEM_DELIMITATION: Tag = Tag(0xfffe, 0xe00d);
const SEQUENCE_DELIMITATION: Tag = Tag(0xfffe, 0xe0dd);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocatorLimits {
    pub max_elements: usize,
    pub max_depth: usize,
    pub max_items: usize,
    pub max_fragments: usize,
}

impl Default for LocatorLimits {
    fn default() -> Self {
        Self {
            max_elements: 100_000,
            max_depth: 32,
            max_items: 100_000,
            max_fragments: 100_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementLocation {
    pub tag: Tag,
    pub vr: [u8; 2],
    pub header: ByteRange,
    pub length_field: ByteRange,
    pub value: ByteRange,
    pub declared_length: Option<u32>,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceLocation {
    pub element_index: usize,
    pub value: ByteRange,
    pub delimitation: Option<ByteRange>,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemLocation {
    pub header: ByteRange,
    pub value: ByteRange,
    pub declared_length: Option<u32>,
    pub delimitation: Option<ByteRange>,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimitationKind {
    Item,
    Sequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelimitationLocation {
    pub kind: DelimitationKind,
    pub bytes: ByteRange,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncapsulatedPixelDataLocation {
    pub pixel_data_element_index: usize,
    pub basic_offset_table_item: ItemLocation,
    pub basic_offset_table_entries: Vec<ByteRange>,
    pub fragment_items: Vec<ItemLocation>,
    pub sequence_delimitation: ByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedPart10 {
    pub file_meta: ByteRange,
    pub file_meta_end: usize,
    pub transfer_syntax_uid: ElementLocation,
    pub dataset: ByteRange,
    pub elements: Vec<ElementLocation>,
    pub sequences: Vec<SequenceLocation>,
    pub items: Vec<ItemLocation>,
    pub delimitations: Vec<DelimitationLocation>,
    pub encapsulated_pixel_data: Option<EncapsulatedPixelDataLocation>,
    pub extended_offset_table_entries: Vec<ByteRange>,
    pub extended_offset_table_length_entries: Vec<ByteRange>,
}

impl LocatedPart10 {
    pub fn first(&self, tag: Tag) -> Option<&ElementLocation> {
        self.elements.iter().find(|element| element.tag == tag)
    }

    pub fn all(&self, tag: Tag) -> impl Iterator<Item = &ElementLocation> {
        self.elements
            .iter()
            .filter(move |element| element.tag == tag)
    }

    pub fn dataset_sop_class_uid(&self) -> Option<&ElementLocation> {
        self.first(SOP_CLASS_UID)
    }

    pub fn dataset_sop_instance_uid(&self) -> Option<&ElementLocation> {
        self.first(SOP_INSTANCE_UID)
    }

    pub fn specific_character_set(&self) -> Option<&ElementLocation> {
        self.first(SPECIFIC_CHARACTER_SET)
    }

    pub fn bits_stored(&self) -> Option<&ElementLocation> {
        self.first(BITS_STORED)
    }

    pub fn high_bit(&self) -> Option<&ElementLocation> {
        self.first(HIGH_BIT)
    }

    pub fn pixel_data(&self) -> Option<&ElementLocation> {
        self.first(PIXEL_DATA)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocatorError {
    NotPart10,
    Truncated {
        offset: usize,
        needed: usize,
        end: usize,
    },
    MissingTransferSyntaxUid,
    UnsupportedTransferSyntax {
        uid: String,
    },
    InvalidVr {
        offset: usize,
        vr: [u8; 2],
    },
    InvalidMetaGroup {
        offset: usize,
        tag: Tag,
    },
    UndefinedLengthNotSupported {
        offset: usize,
        tag: Tag,
    },
    DeclaredLengthExceedsContainer {
        offset: usize,
        declared_end: usize,
        container_end: usize,
    },
    UnexpectedControlTag {
        offset: usize,
        tag: Tag,
    },
    ExpectedItem {
        offset: usize,
        tag: Tag,
    },
    MissingDelimitation {
        offset: usize,
        kind: DelimitationKind,
    },
    InvalidOffsetTableLength {
        tag: Tag,
        length: usize,
    },
    LimitExceeded {
        kind: &'static str,
        limit: usize,
    },
    OffsetOverflow {
        offset: usize,
    },
}

impl fmt::Display for LocatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPart10 => write!(f, "source is not a DICOM Part 10 byte stream"),
            Self::Truncated {
                offset,
                needed,
                end,
            } => write!(
                f,
                "need {needed} bytes at offset {offset}, container ends at {end}"
            ),
            Self::MissingTransferSyntaxUid => write!(f, "file meta has no Transfer Syntax UID"),
            Self::UnsupportedTransferSyntax { uid } => {
                write!(f, "unsupported mutation-source transfer syntax {uid}")
            }
            Self::InvalidVr { offset, vr } => write!(
                f,
                "invalid explicit VR {:?} at offset {offset}",
                String::from_utf8_lossy(vr)
            ),
            Self::InvalidMetaGroup { offset, tag } => write!(
                f,
                "non-file-meta tag ({:04x},{:04x}) at offset {offset}",
                tag.0, tag.1
            ),
            Self::UndefinedLengthNotSupported { offset, tag } => write!(
                f,
                "undefined length is unsupported for ({:04x},{:04x}) at offset {offset}",
                tag.0, tag.1
            ),
            Self::DeclaredLengthExceedsContainer {
                offset,
                declared_end,
                container_end,
            } => write!(
                f,
                "value at {offset} declares end {declared_end} beyond container {container_end}"
            ),
            Self::UnexpectedControlTag { offset, tag } => write!(
                f,
                "unexpected control tag ({:04x},{:04x}) at offset {offset}",
                tag.0, tag.1
            ),
            Self::ExpectedItem { offset, tag } => write!(
                f,
                "expected item at {offset}, found ({:04x},{:04x})",
                tag.0, tag.1
            ),
            Self::MissingDelimitation { offset, kind } => {
                write!(f, "missing {kind:?} delimitation after offset {offset}")
            }
            Self::InvalidOffsetTableLength { tag, length } => write!(
                f,
                "offset table ({:04x},{:04x}) length {length} is not a multiple of its entry width",
                tag.0, tag.1
            ),
            Self::LimitExceeded { kind, limit } => write!(f, "{kind} limit {limit} exceeded"),
            Self::OffsetOverflow { offset } => write!(f, "offset arithmetic overflow at {offset}"),
        }
    }
}

impl Error for LocatorError {}

pub fn locate_explicit_vr_le_part10(
    source: &[u8],
    limits: LocatorLimits,
) -> Result<LocatedPart10, LocatorError> {
    if source.len() < 132 || &source[128..132] != b"DICM" {
        return Err(LocatorError::NotPart10);
    }
    let mut parser = Parser::new(source, limits);
    let file_meta_start = 132;
    let mut cursor = file_meta_start;
    let mut transfer_syntax_index = None;
    while cursor < source.len() {
        parser.require(cursor, 4, source.len())?;
        let tag = parser.tag(cursor);
        if tag.0 != 0x0002 {
            break;
        }
        let (element, next) = parser.parse_element_header(cursor, source.len(), 0)?;
        if element.declared_length.is_none() {
            return Err(LocatorError::UndefinedLengthNotSupported {
                offset: cursor,
                tag,
            });
        }
        let index = parser.push_element(element)?;
        if tag == TRANSFER_SYNTAX_UID {
            transfer_syntax_index = Some(index);
        }
        cursor = next;
    }
    let file_meta_end = cursor;
    let transfer_syntax_index =
        transfer_syntax_index.ok_or(LocatorError::MissingTransferSyntaxUid)?;
    let transfer_syntax_uid = parser.elements[transfer_syntax_index];
    let uid = trim_text(&source[transfer_syntax_uid.value.start..transfer_syntax_uid.value.end]);
    if uid != EXPLICIT_VR_LITTLE_ENDIAN_UID && uid != RLE_LOSSLESS_UID {
        return Err(LocatorError::UnsupportedTransferSyntax {
            uid: uid.to_string(),
        });
    }

    parser.parse_dataset(cursor, source.len(), 0, Stop::ContainerEnd)?;
    parser.collect_extended_offset_entries()?;
    Ok(LocatedPart10 {
        file_meta: ByteRange::new(file_meta_start, file_meta_end),
        file_meta_end,
        transfer_syntax_uid,
        dataset: ByteRange::new(file_meta_end, source.len()),
        elements: parser.elements,
        sequences: parser.sequences,
        items: parser.items,
        delimitations: parser.delimitations,
        encapsulated_pixel_data: parser.encapsulated_pixel_data,
        extended_offset_table_entries: parser.extended_offset_table_entries,
        extended_offset_table_length_entries: parser.extended_offset_table_length_entries,
    })
}

#[derive(Debug, Clone, Copy)]
enum Stop {
    ContainerEnd,
    ItemDelimitation,
}

struct Parser<'a> {
    source: &'a [u8],
    limits: LocatorLimits,
    elements: Vec<ElementLocation>,
    sequences: Vec<SequenceLocation>,
    items: Vec<ItemLocation>,
    delimitations: Vec<DelimitationLocation>,
    encapsulated_pixel_data: Option<EncapsulatedPixelDataLocation>,
    extended_offset_table_entries: Vec<ByteRange>,
    extended_offset_table_length_entries: Vec<ByteRange>,
    items_seen: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a [u8], limits: LocatorLimits) -> Self {
        Self {
            source,
            limits,
            elements: Vec::new(),
            sequences: Vec::new(),
            items: Vec::new(),
            delimitations: Vec::new(),
            encapsulated_pixel_data: None,
            extended_offset_table_entries: Vec::new(),
            extended_offset_table_length_entries: Vec::new(),
            items_seen: 0,
        }
    }

    fn parse_dataset(
        &mut self,
        mut cursor: usize,
        end: usize,
        depth: usize,
        stop: Stop,
    ) -> Result<usize, LocatorError> {
        self.check_depth(depth)?;
        while cursor < end {
            self.require(cursor, 4, end)?;
            let tag = self.tag(cursor);
            if tag == ITEM_DELIMITATION {
                return match stop {
                    Stop::ItemDelimitation => Ok(cursor),
                    Stop::ContainerEnd => Err(LocatorError::UnexpectedControlTag {
                        offset: cursor,
                        tag,
                    }),
                };
            }
            if tag == SEQUENCE_DELIMITATION || tag == ITEM {
                return Err(LocatorError::UnexpectedControlTag {
                    offset: cursor,
                    tag,
                });
            }
            let (element, next) = self.parse_element_header(cursor, end, depth)?;
            let element_index = self.push_element(element)?;
            if element.vr == *b"SQ" {
                cursor = self.parse_sequence(element_index, element.value.start, end, depth)?;
            } else if element.tag == PIXEL_DATA && element.declared_length.is_none() {
                cursor = self.parse_encapsulated_pixel_data(
                    element_index,
                    element.value.start,
                    end,
                    depth,
                )?;
            } else if element.declared_length.is_none() {
                return Err(LocatorError::UndefinedLengthNotSupported {
                    offset: cursor,
                    tag: element.tag,
                });
            } else {
                cursor = next;
            }
        }
        match stop {
            Stop::ContainerEnd => Ok(cursor),
            Stop::ItemDelimitation => Err(LocatorError::MissingDelimitation {
                offset: cursor,
                kind: DelimitationKind::Item,
            }),
        }
    }

    fn parse_sequence(
        &mut self,
        element_index: usize,
        value_start: usize,
        container_end: usize,
        depth: usize,
    ) -> Result<usize, LocatorError> {
        self.check_depth(depth + 1)?;
        let element = self.elements[element_index];
        let sequence_end = element
            .declared_length
            .map_or(container_end, |_| element.value.end);
        let mut cursor = value_start;
        let mut delimitation = None;
        while cursor < sequence_end {
            self.require(cursor, 8, sequence_end)?;
            let tag = self.tag(cursor);
            if tag == SEQUENCE_DELIMITATION {
                if element.declared_length.is_some() {
                    return Err(LocatorError::UnexpectedControlTag {
                        offset: cursor,
                        tag,
                    });
                }
                self.require_zero_control_length(cursor, sequence_end)?;
                let bytes = ByteRange::new(cursor, cursor + 8);
                self.delimitations.push(DelimitationLocation {
                    kind: DelimitationKind::Sequence,
                    bytes,
                    depth,
                });
                delimitation = Some(bytes);
                cursor += 8;
                break;
            }
            if tag != ITEM {
                return Err(LocatorError::ExpectedItem {
                    offset: cursor,
                    tag,
                });
            }
            cursor = self.parse_item(cursor, sequence_end, depth + 1)?;
        }
        if element.declared_length.is_none() && delimitation.is_none() {
            return Err(LocatorError::MissingDelimitation {
                offset: cursor,
                kind: DelimitationKind::Sequence,
            });
        }
        self.sequences.push(SequenceLocation {
            element_index,
            value: ByteRange::new(
                value_start,
                delimitation.map_or(sequence_end, |range| range.start),
            ),
            delimitation,
            depth,
        });
        Ok(cursor)
    }

    fn parse_item(
        &mut self,
        cursor: usize,
        sequence_end: usize,
        depth: usize,
    ) -> Result<usize, LocatorError> {
        self.bump_items()?;
        let raw_length = self.u32(cursor + 4, sequence_end)?;
        let value_start = cursor + 8;
        if raw_length == u32::MAX {
            let delimiter_start =
                self.parse_dataset(value_start, sequence_end, depth, Stop::ItemDelimitation)?;
            self.require_zero_control_length(delimiter_start, sequence_end)?;
            let delimiter = ByteRange::new(delimiter_start, delimiter_start + 8);
            self.delimitations.push(DelimitationLocation {
                kind: DelimitationKind::Item,
                bytes: delimiter,
                depth,
            });
            self.items.push(ItemLocation {
                header: ByteRange::new(cursor, value_start),
                value: ByteRange::new(value_start, delimiter_start),
                declared_length: None,
                delimitation: Some(delimiter),
                depth,
            });
            Ok(delimiter.end)
        } else {
            let value_end = self.checked_end(value_start, raw_length as usize, sequence_end)?;
            self.parse_dataset(value_start, value_end, depth, Stop::ContainerEnd)?;
            self.items.push(ItemLocation {
                header: ByteRange::new(cursor, value_start),
                value: ByteRange::new(value_start, value_end),
                declared_length: Some(raw_length),
                delimitation: None,
                depth,
            });
            Ok(value_end)
        }
    }

    fn parse_encapsulated_pixel_data(
        &mut self,
        element_index: usize,
        mut cursor: usize,
        container_end: usize,
        depth: usize,
    ) -> Result<usize, LocatorError> {
        if self.encapsulated_pixel_data.is_some() {
            return Err(LocatorError::LimitExceeded {
                kind: "encapsulated Pixel Data",
                limit: 1,
            });
        }
        let mut item_locations = Vec::new();
        let sequence_delimitation;
        loop {
            self.require(cursor, 8, container_end)?;
            let tag = self.tag(cursor);
            if tag == SEQUENCE_DELIMITATION {
                self.require_zero_control_length(cursor, container_end)?;
                sequence_delimitation = ByteRange::new(cursor, cursor + 8);
                self.delimitations.push(DelimitationLocation {
                    kind: DelimitationKind::Sequence,
                    bytes: sequence_delimitation,
                    depth,
                });
                cursor += 8;
                break;
            }
            if tag != ITEM {
                return Err(LocatorError::ExpectedItem {
                    offset: cursor,
                    tag,
                });
            }
            self.bump_items()?;
            if !item_locations.is_empty() && item_locations.len() - 1 >= self.limits.max_fragments {
                return Err(LocatorError::LimitExceeded {
                    kind: "fragment",
                    limit: self.limits.max_fragments,
                });
            }
            let length = self.u32(cursor + 4, container_end)?;
            if length == u32::MAX {
                return Err(LocatorError::UndefinedLengthNotSupported {
                    offset: cursor,
                    tag,
                });
            }
            let value_start = cursor + 8;
            let value_end = self.checked_end(value_start, length as usize, container_end)?;
            item_locations.push(ItemLocation {
                header: ByteRange::new(cursor, value_start),
                value: ByteRange::new(value_start, value_end),
                declared_length: Some(length),
                delimitation: None,
                depth: depth + 1,
            });
            cursor = value_end;
        }
        let basic_offset_table_item =
            item_locations
                .first()
                .copied()
                .ok_or(LocatorError::MissingDelimitation {
                    offset: cursor,
                    kind: DelimitationKind::Sequence,
                })?;
        if basic_offset_table_item.value.len() % 4 != 0 {
            return Err(LocatorError::InvalidOffsetTableLength {
                tag: PIXEL_DATA,
                length: basic_offset_table_item.value.len(),
            });
        }
        if basic_offset_table_item.value.len() / 4 > self.limits.max_fragments {
            return Err(LocatorError::LimitExceeded {
                kind: "Basic Offset Table entry",
                limit: self.limits.max_fragments,
            });
        }
        let basic_offset_table_entries = entry_ranges(basic_offset_table_item.value, 4);
        let fragment_items = item_locations[1..].to_vec();
        self.items.extend(item_locations);
        self.encapsulated_pixel_data = Some(EncapsulatedPixelDataLocation {
            pixel_data_element_index: element_index,
            basic_offset_table_item,
            basic_offset_table_entries,
            fragment_items,
            sequence_delimitation,
        });
        Ok(cursor)
    }

    fn parse_element_header(
        &self,
        offset: usize,
        container_end: usize,
        depth: usize,
    ) -> Result<(ElementLocation, usize), LocatorError> {
        self.require(offset, 8, container_end)?;
        let tag = self.tag(offset);
        if tag.0 == 0xfffe {
            return Err(LocatorError::UnexpectedControlTag { offset, tag });
        }
        let vr = [self.source[offset + 4], self.source[offset + 5]];
        if !is_known_vr(vr) {
            return Err(LocatorError::InvalidVr {
                offset: offset + 4,
                vr,
            });
        }
        let long = is_long_vr(vr);
        let (header_end, length_field, raw_length) = if long {
            self.require(offset, 12, container_end)?;
            (
                offset + 12,
                ByteRange::new(offset + 8, offset + 12),
                self.u32(offset + 8, container_end)?,
            )
        } else {
            (
                offset + 8,
                ByteRange::new(offset + 6, offset + 8),
                self.u16(offset + 6, container_end)? as u32,
            )
        };
        let declared_length = (raw_length != u32::MAX).then_some(raw_length);
        let value_end = if let Some(length) = declared_length {
            self.checked_end(header_end, length as usize, container_end)?
        } else {
            header_end
        };
        Ok((
            ElementLocation {
                tag,
                vr,
                header: ByteRange::new(offset, header_end),
                length_field,
                value: ByteRange::new(header_end, value_end),
                declared_length,
                depth,
            },
            if declared_length.is_some() {
                value_end
            } else {
                header_end
            },
        ))
    }

    fn collect_extended_offset_entries(&mut self) -> Result<(), LocatorError> {
        for element in &self.elements {
            let target = match element.tag {
                EXTENDED_OFFSET_TABLE => &mut self.extended_offset_table_entries,
                EXTENDED_OFFSET_TABLE_LENGTHS => &mut self.extended_offset_table_length_entries,
                _ => continue,
            };
            if element.value.len() % 8 != 0 {
                return Err(LocatorError::InvalidOffsetTableLength {
                    tag: element.tag,
                    length: element.value.len(),
                });
            }
            if element.value.len() / 8 > self.limits.max_fragments {
                return Err(LocatorError::LimitExceeded {
                    kind: "Extended Offset Table entry",
                    limit: self.limits.max_fragments,
                });
            }
            target.extend(entry_ranges(element.value, 8));
        }
        Ok(())
    }

    fn push_element(&mut self, element: ElementLocation) -> Result<usize, LocatorError> {
        if self.elements.len() >= self.limits.max_elements {
            return Err(LocatorError::LimitExceeded {
                kind: "element",
                limit: self.limits.max_elements,
            });
        }
        self.elements.push(element);
        Ok(self.elements.len() - 1)
    }

    fn bump_items(&mut self) -> Result<(), LocatorError> {
        if self.items_seen >= self.limits.max_items {
            return Err(LocatorError::LimitExceeded {
                kind: "item",
                limit: self.limits.max_items,
            });
        }
        self.items_seen += 1;
        Ok(())
    }

    fn check_depth(&self, depth: usize) -> Result<(), LocatorError> {
        if depth > self.limits.max_depth {
            return Err(LocatorError::LimitExceeded {
                kind: "nesting depth",
                limit: self.limits.max_depth,
            });
        }
        Ok(())
    }

    fn require(&self, offset: usize, needed: usize, end: usize) -> Result<(), LocatorError> {
        let needed_end = offset
            .checked_add(needed)
            .ok_or(LocatorError::OffsetOverflow { offset })?;
        if needed_end > end || needed_end > self.source.len() {
            return Err(LocatorError::Truncated {
                offset,
                needed,
                end: end.min(self.source.len()),
            });
        }
        Ok(())
    }

    fn checked_end(&self, start: usize, length: usize, end: usize) -> Result<usize, LocatorError> {
        let declared_end = start
            .checked_add(length)
            .ok_or(LocatorError::OffsetOverflow { offset: start })?;
        if declared_end > end || declared_end > self.source.len() {
            return Err(LocatorError::DeclaredLengthExceedsContainer {
                offset: start,
                declared_end,
                container_end: end.min(self.source.len()),
            });
        }
        Ok(declared_end)
    }

    fn require_zero_control_length(&self, offset: usize, end: usize) -> Result<(), LocatorError> {
        self.require(offset, 8, end)?;
        let tag = self.tag(offset);
        if self.u32(offset + 4, end)? != 0 {
            return Err(LocatorError::UnexpectedControlTag { offset, tag });
        }
        Ok(())
    }

    fn tag(&self, offset: usize) -> Tag {
        Tag(
            u16::from_le_bytes([self.source[offset], self.source[offset + 1]]),
            u16::from_le_bytes([self.source[offset + 2], self.source[offset + 3]]),
        )
    }

    fn u16(&self, offset: usize, end: usize) -> Result<u16, LocatorError> {
        self.require(offset, 2, end)?;
        Ok(u16::from_le_bytes(
            self.source[offset..offset + 2].try_into().unwrap(),
        ))
    }

    fn u32(&self, offset: usize, end: usize) -> Result<u32, LocatorError> {
        self.require(offset, 4, end)?;
        Ok(u32::from_le_bytes(
            self.source[offset..offset + 4].try_into().unwrap(),
        ))
    }
}

fn entry_ranges(value: ByteRange, width: usize) -> Vec<ByteRange> {
    (value.start..value.end)
        .step_by(width)
        .map(|start| ByteRange::new(start, start + width))
        .collect()
}

fn trim_text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes)
        .unwrap_or("")
        .trim_end_matches(['\0', ' '])
}

fn is_long_vr(vr: [u8; 2]) -> bool {
    matches!(
        &vr,
        b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
    )
}

fn is_known_vr(vr: [u8; 2]) -> bool {
    matches!(
        &vr,
        b"AE"
            | b"AS"
            | b"AT"
            | b"CS"
            | b"DA"
            | b"DS"
            | b"DT"
            | b"FD"
            | b"FL"
            | b"IS"
            | b"LO"
            | b"LT"
            | b"OB"
            | b"OD"
            | b"OF"
            | b"OL"
            | b"OV"
            | b"OW"
            | b"PN"
            | b"SH"
            | b"SL"
            | b"SQ"
            | b"SS"
            | b"ST"
            | b"SV"
            | b"TM"
            | b"UC"
            | b"UI"
            | b"UL"
            | b"UN"
            | b"UR"
            | b"US"
            | b"UT"
            | b"UV"
    )
}
