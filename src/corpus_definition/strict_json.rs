use std::collections::BTreeMap;

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Number, Value};

use super::{CorpusDefinitionError, CorpusDefinitionLimits, invalid, limit};

pub(super) fn parse(
    bytes: &[u8],
    path: &str,
    limits: CorpusDefinitionLimits,
) -> Result<Value, CorpusDefinitionError> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(invalid(path, "UTF-8 BOM is forbidden".into()));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|e| invalid(path, format!("invalid UTF-8: {e}")))?;
    let mut de = serde_json::Deserializer::from_str(text);
    let value = Seed {
        path,
        limits,
        depth: 0,
    }
    .deserialize(&mut de)
    .map_err(|e| {
        let message = e.to_string();
        if message.contains("limit exceeded") {
            CorpusDefinitionError::Limit(format!("{path}: {message}"))
        } else {
            invalid(path, message)
        }
    })?;
    de.end().map_err(|e| invalid(path, e.to_string()))?;
    Ok(value)
}

#[derive(Clone, Copy)]
struct Seed<'a> {
    path: &'a str,
    limits: CorpusDefinitionLimits,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if self.depth > self.limits.json_depth {
            return Err(serde::de::Error::custom(
                limit(self.path, self.limits.json_depth as u64).to_string(),
            ));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        self.visit_string(v.to_owned())
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
        if v.len() > self.limits.json_string_bytes {
            Err(E::custom("JSON string limit exceeded"))
        } else {
            Ok(Value::String(v))
        }
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = seq.next_element_seed(Seed {
            depth: self.depth + 1,
            ..self
        })? {
            if out.len() >= self.limits.json_array_entries {
                return Err(serde::de::Error::custom("JSON array limit exceeded"));
            }
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut out = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > self.limits.json_string_bytes {
                return Err(serde::de::Error::custom("JSON key limit exceeded"));
            }
            if out.contains_key(&key) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate object key {key:?}"
                )));
            }
            let value = map.next_value_seed(Seed {
                depth: self.depth + 1,
                ..self
            })?;
            out.insert(key, value);
        }
        Ok(Value::Object(out.into_iter().collect()))
    }
}
