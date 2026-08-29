use std::fmt;
use std::str::FromStr;

use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
use dicom_core::{Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use serde::{Deserialize, Serialize};

const MAX_SEQUENCE_DEPTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DicomVr {
    AE,
    AS,
    AT,
    CS,
    DA,
    DS,
    DT,
    FD,
    FL,
    IS,
    LO,
    LT,
    OB,
    OD,
    OF,
    OL,
    OV,
    OW,
    PN,
    SH,
    SL,
    SQ,
    SS,
    ST,
    SV,
    TM,
    UC,
    UI,
    UL,
    UN,
    UR,
    US,
    UT,
    UV,
}

impl DicomVr {
    pub const fn as_dicom(self) -> VR {
        match self {
            Self::AE => VR::AE,
            Self::AS => VR::AS,
            Self::AT => VR::AT,
            Self::CS => VR::CS,
            Self::DA => VR::DA,
            Self::DS => VR::DS,
            Self::DT => VR::DT,
            Self::FD => VR::FD,
            Self::FL => VR::FL,
            Self::IS => VR::IS,
            Self::LO => VR::LO,
            Self::LT => VR::LT,
            Self::OB => VR::OB,
            Self::OD => VR::OD,
            Self::OF => VR::OF,
            Self::OL => VR::OL,
            Self::OV => VR::OV,
            Self::OW => VR::OW,
            Self::PN => VR::PN,
            Self::SH => VR::SH,
            Self::SL => VR::SL,
            Self::SQ => VR::SQ,
            Self::SS => VR::SS,
            Self::ST => VR::ST,
            Self::SV => VR::SV,
            Self::TM => VR::TM,
            Self::UC => VR::UC,
            Self::UI => VR::UI,
            Self::UL => VR::UL,
            Self::UN => VR::UN,
            Self::UR => VR::UR,
            Self::US => VR::US,
            Self::UT => VR::UT,
            Self::UV => VR::UV,
        }
    }

    fn permits(self, value: &PrimitiveValue) -> bool {
        match self {
            Self::AE
            | Self::AS
            | Self::CS
            | Self::DA
            | Self::DS
            | Self::DT
            | Self::IS
            | Self::LO
            | Self::LT
            | Self::PN
            | Self::SH
            | Self::ST
            | Self::TM
            | Self::UC
            | Self::UI
            | Self::UR
            | Self::UT => matches!(value, PrimitiveValue::String(_)),
            Self::AT => matches!(value, PrimitiveValue::Tag(_)),
            Self::FL => matches!(value, PrimitiveValue::Float32Bits(_)),
            Self::FD => matches!(value, PrimitiveValue::Float64Bits(_)),
            Self::SS => {
                matches!(value, PrimitiveValue::Signed(value) if i16::try_from(*value).is_ok())
            }
            Self::SL => {
                matches!(value, PrimitiveValue::Signed(value) if i32::try_from(*value).is_ok())
            }
            Self::SV => matches!(value, PrimitiveValue::Signed(_)),
            Self::US => {
                matches!(value, PrimitiveValue::Unsigned(value) if u16::try_from(*value).is_ok())
            }
            Self::UL => {
                matches!(value, PrimitiveValue::Unsigned(value) if u32::try_from(*value).is_ok())
            }
            Self::UV => matches!(value, PrimitiveValue::Unsigned(_)),
            Self::OB | Self::OD | Self::OF | Self::OL | Self::OV | Self::OW | Self::UN => false,
            Self::SQ => false,
        }
    }

    fn validate_primitive(self, value: &PrimitiveValue) -> bool {
        if !self.permits(value) {
            return false;
        }
        let PrimitiveValue::String(value) = value else {
            return true;
        };
        validate_string_value(self, value)
    }
}

fn validate_string_value(vr: DicomVr, value: &str) -> bool {
    if value.contains('\\') || value.as_bytes().contains(&0) {
        return false;
    }
    let length = value.len();
    match vr {
        DicomVr::AE => {
            length <= 16
                && value
                    .bytes()
                    .all(|byte| byte == b' ' || byte.is_ascii_graphic())
        }
        DicomVr::AS => {
            length == 4
                && value.as_bytes()[..3].iter().all(u8::is_ascii_digit)
                && matches!(value.as_bytes()[3], b'D' | b'W' | b'M' | b'Y')
        }
        DicomVr::CS => {
            length <= 16
                && value.bytes().all(|byte| {
                    byte == b' '
                        || byte == b'_'
                        || byte.is_ascii_uppercase()
                        || byte.is_ascii_digit()
                })
        }
        DicomVr::DA => valid_date(value),
        DicomVr::DS => length <= 16 && value.parse::<f64>().is_ok_and(f64::is_finite),
        DicomVr::DT => valid_datetime(value),
        DicomVr::IS => length <= 12 && value.trim().parse::<i64>().is_ok(),
        DicomVr::LO => length <= 64 && valid_text(value, false),
        DicomVr::LT => length <= 10_240 && valid_text(value, true),
        DicomVr::PN => value
            .split('=')
            .all(|group| group.len() <= 64 && valid_text(group, false)),
        DicomVr::SH => length <= 16 && valid_text(value, false),
        DicomVr::ST => length <= 1_024 && valid_text(value, true),
        DicomVr::TM => valid_time(value),
        DicomVr::UI => valid_uid(value),
        DicomVr::UC | DicomVr::UT => valid_text(value, true),
        DicomVr::UR => !value.chars().any(char::is_control),
        _ => true,
    }
}

fn valid_text(value: &str, allow_formatting: bool) -> bool {
    value.chars().all(|character| {
        !character.is_control()
            || (allow_formatting && matches!(character, '\t' | '\n' | '\u{000C}' | '\r'))
    })
}

fn valid_uid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.split('.').all(|component| {
            !component.is_empty()
                && component.bytes().all(|byte| byte.is_ascii_digit())
                && (component == "0" || !component.starts_with('0'))
        })
}

fn valid_date(value: &str) -> bool {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let year = value[0..4].parse::<u32>().unwrap_or(0);
    let month = value[4..6].parse::<u32>().unwrap_or(0);
    let day = value[6..8].parse::<u32>().unwrap_or(0);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=maximum).contains(&day)
}

fn valid_time(value: &str) -> bool {
    let (whole, fraction) = value
        .split_once('.')
        .map_or((value, None), |(whole, fraction)| (whole, Some(fraction)));
    if !matches!(whole.len(), 2 | 4 | 6)
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|fraction| {
            fraction.is_empty()
                || fraction.len() > 6
                || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return false;
    }
    let hour = whole[0..2].parse::<u32>().unwrap_or(24);
    let minute = (whole.len() >= 4).then(|| whole[2..4].parse::<u32>().unwrap_or(60));
    let second = (whole.len() == 6).then(|| whole[4..6].parse::<u32>().unwrap_or(60));
    hour <= 23 && minute.is_none_or(|value| value <= 59) && second.is_none_or(|value| value <= 60)
}

fn valid_datetime(value: &str) -> bool {
    if value.is_empty() || value.len() > 26 {
        return false;
    }
    let timezone_start = value
        .char_indices()
        .skip(8)
        .find_map(|(index, character)| matches!(character, '+' | '-').then_some(index));
    let (local, timezone) = timezone_start.map_or((value, None), |index| {
        (&value[..index], Some(&value[index..]))
    });
    if timezone.is_some_and(|zone| {
        zone.len() != 5
            || !zone[1..].bytes().all(|byte| byte.is_ascii_digit())
            || zone[1..3].parse::<u32>().unwrap_or(24) > 23
            || zone[3..5].parse::<u32>().unwrap_or(60) > 59
    }) {
        return false;
    }
    let (whole, fraction) = local
        .split_once('.')
        .map_or((local, None), |(whole, fraction)| (whole, Some(fraction)));
    if !matches!(whole.len(), 4 | 6 | 8 | 10 | 12 | 14)
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|fraction| {
            fraction.is_empty()
                || fraction.len() > 6
                || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return false;
    }
    if whole.len() >= 8 && !valid_date(&whole[..8]) {
        return false;
    }
    if whole.len() > 8 && !valid_time(&whole[8..]) {
        return false;
    }
    true
}

impl fmt::Display for DicomVr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_dicom().to_string())
    }
}

impl FromStr for DicomVr {
    type Err = AttributeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let vr = VR::from_str(value).map_err(|_| AttributeError::MalformedVr(value.to_string()))?;
        Ok(match vr {
            VR::AE => Self::AE,
            VR::AS => Self::AS,
            VR::AT => Self::AT,
            VR::CS => Self::CS,
            VR::DA => Self::DA,
            VR::DS => Self::DS,
            VR::DT => Self::DT,
            VR::FD => Self::FD,
            VR::FL => Self::FL,
            VR::IS => Self::IS,
            VR::LO => Self::LO,
            VR::LT => Self::LT,
            VR::OB => Self::OB,
            VR::OD => Self::OD,
            VR::OF => Self::OF,
            VR::OL => Self::OL,
            VR::OV => Self::OV,
            VR::OW => Self::OW,
            VR::PN => Self::PN,
            VR::SH => Self::SH,
            VR::SL => Self::SL,
            VR::SQ => Self::SQ,
            VR::SS => Self::SS,
            VR::ST => Self::ST,
            VR::SV => Self::SV,
            VR::TM => Self::TM,
            VR::UC => Self::UC,
            VR::UI => Self::UI,
            VR::UL => Self::UL,
            VR::UN => Self::UN,
            VR::UR => Self::UR,
            VR::US => Self::US,
            VR::UT => Self::UT,
            VR::UV => Self::UV,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AttributeAddress {
    pub group: u16,
    pub element: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_creator: Option<String>,
}

impl AttributeAddress {
    pub fn standard(tag: Tag) -> Result<Self, AttributeError> {
        let address = Self {
            group: tag.group(),
            element: tag.element(),
            private_creator: None,
        };
        address.validate_shape()?;
        Ok(address)
    }

    pub fn private(tag: Tag, private_creator: impl Into<String>) -> Result<Self, AttributeError> {
        let address = Self {
            group: tag.group(),
            element: tag.element(),
            private_creator: Some(private_creator.into()),
        };
        address.validate_shape()?;
        Ok(address)
    }

    pub fn from_keyword(keyword: &str) -> Result<Self, AttributeError> {
        let dictionary = StandardDataDictionary;
        let entry = dictionary
            .by_name(keyword)
            .ok_or_else(|| AttributeError::UnknownKeyword(keyword.to_string()))?;
        Self::standard(entry.tag())
    }

    pub fn from_normalized_tag(value: &str) -> Result<Self, AttributeError> {
        if value.len() != 9 || value.as_bytes()[4] != b',' {
            return Err(AttributeError::MalformedTag(value.to_string()));
        }
        let group = u16::from_str_radix(&value[..4], 16)
            .map_err(|_| AttributeError::MalformedTag(value.to_string()))?;
        let element = u16::from_str_radix(&value[5..], 16)
            .map_err(|_| AttributeError::MalformedTag(value.to_string()))?;
        if value != format!("{group:04X},{element:04X}") {
            return Err(AttributeError::MalformedTag(value.to_string()));
        }
        Self::standard(Tag(group, element))
    }

    pub const fn tag(&self) -> Tag {
        Tag(self.group, self.element)
    }

    pub fn normalized_tag(&self) -> String {
        format!("{:04X},{:04X}", self.group, self.element)
    }

    fn validate_shape(&self) -> Result<(), AttributeError> {
        let is_private_group = self.group & 1 == 1;
        match (&self.private_creator, is_private_group, self.element) {
            (Some(creator), true, 0x1000..=0xFFFF)
                if !creator.is_empty() && creator.len() <= 64 =>
            {
                Ok(())
            }
            (Some(_), true, 0x1000..=0xFFFF) => Err(AttributeError::InvalidPrivateCreator {
                tag: self.normalized_tag(),
            }),
            (Some(_), _, _) => Err(AttributeError::InvalidPrivateTag {
                tag: self.normalized_tag(),
            }),
            (None, true, 0x1000..=0xFFFF) => Err(AttributeError::MissingPrivateCreator {
                tag: self.normalized_tag(),
            }),
            (None, _, _) => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PrimitiveValue {
    String(String),
    Signed(i64),
    Unsigned(u64),
    Float32Bits(u32),
    Float64Bits(u64),
    Tag(AttributeAddress),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AttributeValue {
    Primitive(PrimitiveValue),
    Multi(Vec<PrimitiveValue>),
    Binary(Vec<u8>),
    Sequence(Vec<AttributeItem>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeItem {
    pub attributes: Vec<AttributeOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum AttributeOperation {
    Set {
        address: AttributeAddress,
        vr: DicomVr,
        value: AttributeValue,
    },
    Empty {
        address: AttributeAddress,
    },
    Remove {
        address: AttributeAddress,
    },
}

impl AttributeOperation {
    pub fn address(&self) -> &AttributeAddress {
        match self {
            Self::Set { address, .. } | Self::Empty { address } | Self::Remove { address } => {
                address
            }
        }
    }

    pub fn validate(&self) -> Result<(), AttributeError> {
        self.validate_at_depth(0)
    }

    fn validate_at_depth(&self, depth: usize) -> Result<(), AttributeError> {
        if depth > MAX_SEQUENCE_DEPTH {
            return Err(AttributeError::SequenceDepthExceeded {
                maximum: MAX_SEQUENCE_DEPTH,
            });
        }
        self.address().validate_shape()?;
        let Self::Set { address, vr, value } = self else {
            return Ok(());
        };
        validate_dictionary_vr(address, *vr)?;
        match value {
            AttributeValue::Primitive(value) => {
                if !vr.validate_primitive(value) {
                    return Err(AttributeError::ValueVrMismatch {
                        tag: address.normalized_tag(),
                        vr: *vr,
                    });
                }
            }
            AttributeValue::Multi(values) => {
                if values.is_empty()
                    || values.iter().any(|value| !vr.validate_primitive(value))
                    || !valid_multi_value_count(address, values.len())
                {
                    return Err(AttributeError::ValueVrMismatch {
                        tag: address.normalized_tag(),
                        vr: *vr,
                    });
                }
            }
            AttributeValue::Binary(_) => {
                if !matches!(
                    vr,
                    DicomVr::OB
                        | DicomVr::OD
                        | DicomVr::OF
                        | DicomVr::OL
                        | DicomVr::OV
                        | DicomVr::OW
                        | DicomVr::UN
                ) {
                    return Err(AttributeError::ValueVrMismatch {
                        tag: address.normalized_tag(),
                        vr: *vr,
                    });
                }
            }
            AttributeValue::Sequence(items) => {
                if *vr != DicomVr::SQ {
                    return Err(AttributeError::ValueVrMismatch {
                        tag: address.normalized_tag(),
                        vr: *vr,
                    });
                }
                for item in items {
                    for operation in &item.attributes {
                        operation.validate_at_depth(depth + 1)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn valid_multi_value_count(address: &AttributeAddress, count: usize) -> bool {
    if address.private_creator.is_some() {
        return count > 0;
    }
    let tag = (address.group, address.element);
    match tag {
        (0x0008, 0x0005)
        | (0x0008, 0x0054)
        | (0x0008, 0x0058)
        | (0x0008, 0x1160)
        | (0x0010, 0x1000)
        | (0x0018, 0x0086)
        | (0x0028, 0x1050)
        | (0x0028, 0x1051) => count >= 1,
        (0x0008, 0x0008) => count >= 2,
        (0x0018, 0x1310) => count == 4,
        (0x0020, 0x0032) | (0x0020, 0x9157) | (0x0028, 0x3002) => count == 3,
        (0x0020, 0x0037) => count == 6,
        (0x0020, 0x0020) | (0x0028, 0x0030) | (0x0028, 0x0034) => count == 2,
        _ => false,
    }
}

fn validate_dictionary_vr(address: &AttributeAddress, vr: DicomVr) -> Result<(), AttributeError> {
    if address.private_creator.is_some() {
        return Ok(());
    }
    let dictionary = StandardDataDictionary;
    let Some(entry) = dictionary.by_tag(address.tag()) else {
        return Err(AttributeError::UnknownStandardTag(address.normalized_tag()));
    };
    let supplied = vr.as_dicom();
    let matches = match entry.vr() {
        VirtualVr::Exact(expected) => supplied == expected,
        VirtualVr::Xs => matches!(supplied, VR::US | VR::SS),
        VirtualVr::Ox | VirtualVr::Px => matches!(supplied, VR::OB | VR::OW),
        VirtualVr::Lt => matches!(supplied, VR::US | VR::OW),
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(AttributeError::DictionaryVrMismatch {
            tag: address.normalized_tag(),
            supplied: vr,
            expected: format!("{:?}", entry.vr()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeError {
    MalformedVr(String),
    MalformedTag(String),
    UnknownKeyword(String),
    UnknownStandardTag(String),
    MissingPrivateCreator {
        tag: String,
    },
    InvalidPrivateCreator {
        tag: String,
    },
    InvalidPrivateTag {
        tag: String,
    },
    DictionaryVrMismatch {
        tag: String,
        supplied: DicomVr,
        expected: String,
    },
    ValueVrMismatch {
        tag: String,
        vr: DicomVr,
    },
    SequenceDepthExceeded {
        maximum: usize,
    },
}

impl fmt::Display for AttributeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedVr(vr) => write!(formatter, "malformed DICOM VR {vr:?}"),
            Self::MalformedTag(tag) => write!(formatter, "malformed normalized DICOM tag {tag:?}"),
            Self::UnknownKeyword(keyword) => write!(formatter, "unknown DICOM keyword {keyword:?}"),
            Self::UnknownStandardTag(tag) => write!(formatter, "unknown standard DICOM tag {tag}"),
            Self::MissingPrivateCreator { tag } => {
                write!(
                    formatter,
                    "private element {tag} requires a private creator"
                )
            }
            Self::InvalidPrivateCreator { tag } => {
                write!(
                    formatter,
                    "private element {tag} has an invalid private creator"
                )
            }
            Self::InvalidPrivateTag { tag } => {
                write!(
                    formatter,
                    "private creator was supplied for non-private data tag {tag}"
                )
            }
            Self::DictionaryVrMismatch {
                tag,
                supplied,
                expected,
            } => write!(
                formatter,
                "DICOM tag {tag} uses VR {supplied}, but the pinned dictionary requires {expected}"
            ),
            Self::ValueVrMismatch { tag, vr } => {
                write!(
                    formatter,
                    "DICOM tag {tag} has a value incompatible with VR {vr}"
                )
            }
            Self::SequenceDepthExceeded { maximum } => {
                write!(formatter, "attribute Sequence depth exceeds {maximum}")
            }
        }
    }
}

impl std::error::Error for AttributeError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(address: AttributeAddress, vr: DicomVr, value: AttributeValue) -> AttributeOperation {
        AttributeOperation::Set { address, vr, value }
    }

    #[test]
    fn keywords_normalize_to_stable_tags() {
        let address = AttributeAddress::from_keyword("PatientName").unwrap();
        assert_eq!(address.normalized_tag(), "0010,0010");
        assert_eq!(address.private_creator, None);
    }

    #[test]
    fn known_standard_tags_require_dictionary_vr() {
        let address = AttributeAddress::from_keyword("PatientName").unwrap();
        assert!(
            set(
                address.clone(),
                DicomVr::PN,
                AttributeValue::Primitive(PrimitiveValue::String("DTS^Synthetic".into()))
            )
            .validate()
            .is_ok()
        );
        assert!(matches!(
            set(
                address,
                DicomVr::LO,
                AttributeValue::Primitive(PrimitiveValue::String("wrong".into()))
            )
            .validate(),
            Err(AttributeError::DictionaryVrMismatch { .. })
        ));
    }

    #[test]
    fn virtual_dictionary_vrs_accept_only_their_contextual_forms() {
        let pixel_data = AttributeAddress::standard(Tag(0x7FE0, 0x0010)).unwrap();
        for vr in [DicomVr::OB, DicomVr::OW] {
            assert!(
                set(pixel_data.clone(), vr, AttributeValue::Binary(vec![0, 1]))
                    .validate()
                    .is_ok()
            );
        }
        assert!(
            set(pixel_data, DicomVr::UN, AttributeValue::Binary(vec![0, 1]))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn unknown_private_elements_require_creator_and_explicit_vr() {
        assert!(matches!(
            AttributeAddress::standard(Tag(0x0011, 0x1010)),
            Err(AttributeError::MissingPrivateCreator { .. })
        ));
        let address = AttributeAddress::private(Tag(0x0011, 0x1010), "DTS_COMPOSE").unwrap();
        assert!(
            set(address, DicomVr::OB, AttributeValue::Binary(vec![1, 2, 3]))
                .validate()
                .is_ok()
        );
        assert!(AttributeAddress::private(Tag(0x0010, 0x1010), "DTS_COMPOSE").is_err());
    }

    #[test]
    fn primitive_and_multi_values_are_range_and_vr_checked() {
        let rows = AttributeAddress::from_keyword("Rows").unwrap();
        assert!(
            set(
                rows.clone(),
                DicomVr::US,
                AttributeValue::Primitive(PrimitiveValue::Unsigned(65535))
            )
            .validate()
            .is_ok()
        );
        assert!(
            set(
                rows,
                DicomVr::US,
                AttributeValue::Primitive(PrimitiveValue::Unsigned(65536))
            )
            .validate()
            .is_err()
        );

        let image_type = AttributeAddress::from_keyword("ImageType").unwrap();
        assert!(
            set(
                image_type,
                DicomVr::CS,
                AttributeValue::Multi(vec![
                    PrimitiveValue::String("DERIVED".into()),
                    PrimitiveValue::String("SECONDARY".into()),
                ])
            )
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn string_lexemes_and_standard_value_multiplicity_are_checked() {
        let birth_date = AttributeAddress::from_keyword("PatientBirthDate").unwrap();
        assert!(
            set(
                birth_date,
                DicomVr::DA,
                AttributeValue::Primitive(PrimitiveValue::String("20260230".into()))
            )
            .validate()
            .is_err()
        );

        let study_uid = AttributeAddress::from_keyword("StudyInstanceUID").unwrap();
        assert!(
            set(
                study_uid,
                DicomVr::UI,
                AttributeValue::Primitive(PrimitiveValue::String("1.02.3".into()))
            )
            .validate()
            .is_err()
        );

        let series_description = AttributeAddress::from_keyword("SeriesDescription").unwrap();
        assert!(
            set(
                series_description,
                DicomVr::LO,
                AttributeValue::Multi(vec![
                    PrimitiveValue::String("ONE".into()),
                    PrimitiveValue::String("TWO".into()),
                ])
            )
            .validate()
            .is_err()
        );

        let image_type = AttributeAddress::from_keyword("ImageType").unwrap();
        assert!(
            set(
                image_type,
                DicomVr::CS,
                AttributeValue::Multi(vec![
                    PrimitiveValue::String("DERIVED".into()),
                    PrimitiveValue::String("SECONDARY".into()),
                ])
            )
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn nested_sequences_are_recursively_typed() {
        let sequence = AttributeAddress::from_keyword("ReferencedSeriesSequence").unwrap();
        let series_uid = AttributeAddress::from_keyword("SeriesInstanceUID").unwrap();
        let operation = set(
            sequence,
            DicomVr::SQ,
            AttributeValue::Sequence(vec![AttributeItem {
                attributes: vec![set(
                    series_uid,
                    DicomVr::UI,
                    AttributeValue::Primitive(PrimitiveValue::String("2.25.123".into())),
                )],
            }]),
        );
        assert!(operation.validate().is_ok());
    }

    #[test]
    fn empty_and_remove_operations_retain_normalized_addresses() {
        let patient_birth_date = AttributeAddress::from_keyword("PatientBirthDate").unwrap();
        assert!(
            AttributeOperation::Empty {
                address: patient_birth_date.clone()
            }
            .validate()
            .is_ok()
        );
        assert!(
            AttributeOperation::Remove {
                address: patient_birth_date
            }
            .validate()
            .is_ok()
        );
    }
}
