use std::{collections::BTreeMap, sync::Arc};

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};

use super::error::AgentError;

const IMAGE_BASE64_PREFIX: &str = "data:image/png;base64,";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentData {
    pub kind: String,
    pub value: AgentValue,
}

impl AgentData {
    pub fn unit() -> Self {
        Self {
            kind: "unit".to_string(),
            value: AgentValue::unit(),
        }
    }

    pub fn boolean(value: bool) -> Self {
        AgentData {
            kind: "boolean".to_string(),
            value: AgentValue::boolean(value),
        }
    }

    pub fn integer(value: i64) -> Self {
        AgentData {
            kind: "integer".to_string(),
            value: AgentValue::integer(value),
        }
    }

    pub fn number(value: f64) -> Self {
        AgentData {
            kind: "number".to_string(),
            value: AgentValue::number(value),
        }
    }

    pub fn string(value: impl Into<String>) -> Self {
        AgentData {
            kind: "string".to_string(),
            value: AgentValue::string(value.into()),
        }
    }

    #[cfg(feature = "image")]
    pub fn image(value: PhotonImage) -> Self {
        AgentData {
            kind: "image".to_string(),
            value: AgentValue::image(value),
        }
    }

    pub fn object(value: AgentValueMap<String, AgentValue>) -> Self {
        AgentData {
            kind: "object".to_string(),
            value: AgentValue::object(value),
        }
    }

    pub fn object_with_kind(
        kind: impl Into<String>,
        value: AgentValueMap<String, AgentValue>,
    ) -> Self {
        AgentData {
            kind: kind.into(),
            value: AgentValue::object(value),
        }
    }

    pub fn array(kind: impl Into<String>, value: Vec<AgentValue>) -> Self {
        AgentData {
            kind: kind.into(),
            value: AgentValue::array(value),
        }
    }

    pub fn from_value(value: AgentValue) -> Self {
        let kind = value.kind();
        AgentData { kind, value }
    }

    pub fn from_json_with_kind(
        kind: impl Into<String>,
        value: serde_json::Value,
    ) -> Result<Self, AgentError> {
        let kind: String = kind.into();
        let value = AgentValue::from_kind_json(&kind, value)?;
        Ok(Self { kind, value })
    }

    pub fn from_json(json_value: serde_json::Value) -> Result<Self, AgentError> {
        let value = AgentValue::from_json(json_value)?;
        Ok(AgentData {
            kind: value.kind(),
            value,
        })
    }

    /// Create AgentData from any Serialize with automatic kind inference
    pub fn from_serialize<T: Serialize>(value: &T) -> Result<Self, AgentError> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to serialize: {}", e)))?;
        Self::from_json(json_value)
    }

    /// Create AgentData from any Serialize with custom kind
    pub fn from_serialize_with_kind<T: Serialize>(
        kind: impl Into<String>,
        value: &T,
    ) -> Result<Self, AgentError> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to serialize: {}", e)))?;
        Self::from_json_with_kind(kind, json_value)
    }

    /// Convert AgentData to a Deserialize
    pub fn to_deserialize<T: for<'de> Deserialize<'de>>(&self) -> Result<T, AgentError> {
        let json_value = self.value.to_json();
        serde_json::from_value(json_value)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to deserialize: {}", e)))
    }

    #[allow(unused)]
    pub fn is_unit(&self) -> bool {
        self.kind == "unit"
    }

    #[allow(unused)]
    pub fn is_boolean(&self) -> bool {
        self.kind == "boolean"
    }

    #[allow(unused)]
    pub fn is_integer(&self) -> bool {
        self.kind == "integer"
    }

    #[allow(unused)]
    pub fn is_number(&self) -> bool {
        self.kind == "number"
    }

    #[allow(unused)]
    pub fn is_string(&self) -> bool {
        self.kind == "string"
    }

    #[cfg(feature = "image")]
    #[allow(unused)]
    pub fn is_image(&self) -> bool {
        self.kind == "image"
    }

    #[allow(unused)]
    pub fn is_object(&self) -> bool {
        if let AgentValue::Object(_) = &self.value {
            true
        } else {
            false
        }
    }

    #[allow(unused)]
    pub fn is_array(&self) -> bool {
        if let AgentValue::Array(_) = &self.value {
            true
        } else {
            false
        }
    }

    #[allow(unused)]
    pub fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    #[allow(unused)]
    pub fn as_i64(&self) -> Option<i64> {
        self.value.as_i64()
    }

    #[allow(unused)]
    pub fn as_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }

    pub fn as_str(&self) -> Option<&str> {
        self.value.as_str()
    }

    #[cfg(feature = "image")]
    #[allow(unused)]
    pub fn as_image(&self) -> Option<Arc<PhotonImage>> {
        self.value.as_image()
    }

    pub fn as_object(&self) -> Option<&AgentValueMap<String, AgentValue>> {
        self.value.as_object()
    }

    #[allow(unused)]
    pub fn as_array(&self) -> Option<&Vec<AgentValue>> {
        self.value.as_array()
    }

    #[allow(unused)]
    pub fn get(&self, key: &str) -> Option<&AgentValue> {
        self.value.get(key)
    }

    #[allow(unused)]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.value.get_bool(key)
    }

    #[allow(unused)]
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.value.get_i64(key)
    }

    #[allow(unused)]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.value.get_f64(key)
    }

    #[allow(unused)]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.value.get_str(key)
    }

    #[cfg(feature = "image")]
    #[allow(unused)]
    pub fn get_image(&self, key: &str) -> Option<Arc<PhotonImage>> {
        self.value.get_image(key)
    }

    #[allow(unused)]
    pub fn get_object(&self, key: &str) -> Option<&AgentValueMap<String, AgentValue>> {
        self.value.get_object(key)
    }

    #[allow(unused)]
    pub fn get_array(&self, key: &str) -> Option<&Vec<AgentValue>> {
        self.value.get_array(key)
    }
}

impl<'de> Deserialize<'de> for AgentData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json_value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(obj) = json_value else {
            return Err(serde::de::Error::custom("not a JSON object"));
        };
        let Some(kind) = obj.get("kind").and_then(|k| k.as_str()) else {
            return Err(serde::de::Error::custom("missing kind"));
        };
        let Some(value) = obj.get("value") else {
            return Err(serde::de::Error::custom("Missing value"));
        };
        AgentData::from_json_with_kind(kind, value.to_owned()).map_err(|e| {
            serde::de::Error::custom(format!("Failed to deserialize AgentData: {}", e))
        })
    }
}

#[derive(Debug, Clone)]
pub enum AgentValue {
    // Primitive types stored directly
    Unit,
    Boolean(bool),
    Integer(i64),
    Number(f64),

    // Larger data structures use reference counting
    String(Arc<String>),

    #[cfg(feature = "image")]
    Image(Arc<PhotonImage>),

    // Recursive data structures
    Array(Arc<Vec<AgentValue>>),
    Object(Arc<AgentValueMap<String, AgentValue>>),
}

pub type AgentValueMap<S, T> = BTreeMap<S, T>;

impl AgentValue {
    pub fn unit() -> Self {
        AgentValue::Unit
    }

    pub fn boolean(value: bool) -> Self {
        AgentValue::Boolean(value)
    }

    pub fn integer(value: i64) -> Self {
        AgentValue::Integer(value)
    }

    pub fn number(value: f64) -> Self {
        AgentValue::Number(value)
    }

    pub fn string(value: impl Into<String>) -> Self {
        AgentValue::String(Arc::new(value.into()))
    }

    #[cfg(feature = "image")]
    pub fn image(value: PhotonImage) -> Self {
        AgentValue::Image(Arc::new(value))
    }

    #[cfg(feature = "image")]
    pub fn image_arc(value: Arc<PhotonImage>) -> Self {
        AgentValue::Image(value)
    }

    pub fn object(value: AgentValueMap<String, AgentValue>) -> Self {
        AgentValue::Object(Arc::new(value))
    }

    pub fn array(value: Vec<AgentValue>) -> Self {
        AgentValue::Array(Arc::new(value))
    }

    pub fn boolean_default() -> Self {
        AgentValue::Boolean(false)
    }

    pub fn integer_default() -> Self {
        AgentValue::Integer(0)
    }

    pub fn number_default() -> Self {
        AgentValue::Number(0.0)
    }

    pub fn string_default() -> Self {
        AgentValue::String(Arc::new(String::new()))
    }

    #[cfg(feature = "image")]
    pub fn image_default() -> Self {
        AgentValue::Image(Arc::new(PhotonImage::new(vec![0u8, 0u8, 0u8, 0u8], 1, 1)))
    }

    pub fn array_default() -> Self {
        AgentValue::Array(Arc::new(Vec::new()))
    }

    pub fn object_default() -> Self {
        AgentValue::Object(Arc::new(AgentValueMap::new()))
    }

    pub fn from_json(value: serde_json::Value) -> Result<Self, AgentError> {
        match value {
            serde_json::Value::Null => Ok(AgentValue::Unit),
            serde_json::Value::Bool(b) => Ok(AgentValue::Boolean(b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(AgentValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(AgentValue::Number(f))
                } else {
                    // This case should not happen, but handle it gracefully
                    Ok(AgentValue::Integer(0))
                }
            }
            serde_json::Value::String(s) => {
                #[cfg(feature = "image")]
                if s.starts_with(IMAGE_BASE64_PREFIX) {
                    let img =
                        PhotonImage::new_from_base64(&s.trim_start_matches(IMAGE_BASE64_PREFIX));
                    Ok(AgentValue::Image(Arc::new(img)))
                } else {
                    Ok(AgentValue::String(Arc::new(s)))
                }
                #[cfg(not(feature = "image"))]
                Ok(AgentValue::String(Arc::new(s)))
            }
            serde_json::Value::Array(arr) => {
                let mut agent_arr = Vec::new();
                for v in arr {
                    agent_arr.push(AgentValue::from_json(v)?);
                }
                Ok(AgentValue::array(agent_arr))
            }
            serde_json::Value::Object(obj) => {
                let mut map = AgentValueMap::new();
                for (k, v) in obj {
                    map.insert(k, AgentValue::from_json(v)?);
                }
                Ok(AgentValue::object(map))
            }
        }
    }

    pub fn from_kind_json(kind: &str, value: serde_json::Value) -> Result<Self, AgentError> {
        match kind {
            "unit" => {
                if let serde_json::Value::Array(a) = value {
                    Ok(AgentValue::Array(Arc::new(
                        a.into_iter().map(|_| AgentValue::Unit).collect(),
                    )))
                } else {
                    Ok(AgentValue::Unit)
                }
            }
            "boolean" => match value {
                serde_json::Value::Bool(b) => Ok(AgentValue::Boolean(b)),
                serde_json::Value::Array(a) => {
                    let mut agent_arr = Vec::new();
                    for v in a {
                        if let serde_json::Value::Bool(b) = v {
                            agent_arr.push(AgentValue::Boolean(b));
                        } else {
                            return Err(AgentError::InvalidArrayValue("boolean".into()));
                        }
                    }
                    Ok(AgentValue::Array(Arc::new(agent_arr)))
                }
                _ => Err(AgentError::InvalidValue("boolean".into())),
            },
            "integer" => match value {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Ok(AgentValue::Integer(i))
                    } else if let Some(f) = n.as_f64() {
                        Ok(AgentValue::Integer(f as i64))
                    } else {
                        Err(AgentError::InvalidValue("integer".into()))
                    }
                }
                serde_json::Value::Array(a) => {
                    let mut agent_arr = Vec::new();
                    for n in a {
                        if let Some(i) = n.as_i64() {
                            agent_arr.push(AgentValue::Integer(i));
                        } else if let Some(f) = n.as_f64() {
                            agent_arr.push(AgentValue::Integer(f as i64));
                        } else {
                            return Err(AgentError::InvalidArrayValue("integer".into()));
                        }
                    }
                    Ok(AgentValue::Array(Arc::new(agent_arr)))
                }
                _ => Err(AgentError::InvalidValue("integer".into())),
            },
            "number" => match value {
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        Ok(AgentValue::Number(f))
                    } else if let Some(i) = n.as_i64() {
                        Ok(AgentValue::Number(i as f64))
                    } else {
                        Err(AgentError::InvalidValue("number".into()))
                    }
                }
                serde_json::Value::Array(a) => {
                    let mut agent_arr = Vec::new();
                    for n in a {
                        if let Some(f) = n.as_f64() {
                            agent_arr.push(AgentValue::Number(f));
                        } else if let Some(i) = n.as_i64() {
                            agent_arr.push(AgentValue::Number(i as f64));
                        } else {
                            return Err(AgentError::InvalidArrayValue("number".into()));
                        }
                    }
                    Ok(AgentValue::Array(Arc::new(agent_arr)))
                }
                _ => Err(AgentError::InvalidValue("number".into())),
            },
            "string" => match value {
                serde_json::Value::String(s) => Ok(AgentValue::string(s)),
                serde_json::Value::Array(a) => {
                    let mut agent_arr = Vec::new();
                    for v in a {
                        if let serde_json::Value::String(s) = v {
                            agent_arr.push(AgentValue::string(s));
                        } else {
                            return Err(AgentError::InvalidArrayValue("string".into()));
                        }
                    }
                    Ok(AgentValue::Array(Arc::new(agent_arr)))
                }
                _ => Err(AgentError::InvalidValue("string".into())),
            },
            #[cfg(feature = "image")]
            "image" => match value {
                serde_json::Value::String(s) => Ok(AgentValue::Image(Arc::new(
                    PhotonImage::new_from_base64(&s.trim_start_matches(IMAGE_BASE64_PREFIX)),
                ))),
                serde_json::Value::Array(a) => {
                    let mut agent_arr = Vec::new();
                    for v in a {
                        if let serde_json::Value::String(s) = v {
                            agent_arr.push(AgentValue::image(PhotonImage::new_from_base64(
                                &s.trim_start_matches(IMAGE_BASE64_PREFIX),
                            )));
                        } else {
                            return Err(AgentError::InvalidArrayValue("image".into()));
                        }
                    }
                    Ok(AgentValue::Array(Arc::new(agent_arr)))
                }
                _ => Err(AgentError::InvalidValue("image".into())),
            },
            _ => match value {
                serde_json::Value::Null => Ok(AgentValue::Unit),
                serde_json::Value::Bool(b) => Ok(AgentValue::Boolean(b)),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Ok(AgentValue::Integer(i))
                    } else if let Some(f) = n.as_f64() {
                        Ok(AgentValue::Number(f))
                    } else {
                        Err(AgentError::InvalidValue("number".into()))
                    }
                }
                serde_json::Value::String(s) => Ok(AgentValue::string(s)),
                serde_json::Value::Array(a) => {
                    let mut agent_arr = Vec::new();
                    for v in a {
                        let agent_v = AgentValue::from_kind_json(kind, v)?;
                        agent_arr.push(agent_v);
                    }
                    Ok(AgentValue::Array(Arc::new(agent_arr)))
                }
                serde_json::Value::Object(obj) => {
                    let mut map = AgentValueMap::new();
                    for (k, v) in obj {
                        map.insert(k.clone(), AgentValue::from_json(v)?);
                    }
                    Ok(AgentValue::object(map))
                }
            },
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            AgentValue::Unit => serde_json::Value::Null,
            AgentValue::Boolean(b) => (*b).into(),
            AgentValue::Integer(i) => (*i).into(),
            AgentValue::Number(n) => (*n).into(),
            AgentValue::String(s) => s.as_str().into(),
            #[cfg(feature = "image")]
            AgentValue::Image(img) => img.get_base64().into(),
            AgentValue::Object(o) => {
                let mut map = serde_json::Map::new();
                for (k, v) in o.iter() {
                    map.insert(k.clone(), v.to_json());
                }
                serde_json::Value::Object(map)
            }
            AgentValue::Array(a) => {
                let arr: Vec<serde_json::Value> = a.iter().map(|v| v.to_json()).collect();
                serde_json::Value::Array(arr)
            }
        }
    }

    /// Create AgentValue from Serialize
    pub fn from_serialize<T: Serialize>(value: &T) -> Result<Self, AgentError> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to serialize: {}", e)))?;
        Self::from_json(json_value)
    }

    /// Convert AgentValue to a Deserialize
    pub fn to_deserialize<T: for<'de> Deserialize<'de>>(&self) -> Result<T, AgentError> {
        let json_value = self.to_json();
        serde_json::from_value(json_value)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to deserialize: {}", e)))
    }

    #[allow(unused)]
    pub fn is_unit(&self) -> bool {
        matches!(self, AgentValue::Unit)
    }

    #[allow(unused)]
    pub fn is_boolean(&self) -> bool {
        matches!(self, AgentValue::Boolean(_))
    }

    #[allow(unused)]
    pub fn is_integer(&self) -> bool {
        matches!(self, AgentValue::Integer(_))
    }

    #[allow(unused)]
    pub fn is_number(&self) -> bool {
        matches!(self, AgentValue::Number(_))
    }

    #[allow(unused)]
    pub fn is_string(&self) -> bool {
        matches!(self, AgentValue::String(_))
    }

    #[cfg(feature = "image")]
    #[allow(unused)]
    pub fn is_image(&self) -> bool {
        matches!(self, AgentValue::Image(_))
    }

    #[allow(unused)]
    pub fn is_array(&self) -> bool {
        matches!(self, AgentValue::Array(_))
    }

    #[allow(unused)]
    pub fn is_object(&self) -> bool {
        matches!(self, AgentValue::Object(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AgentValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            AgentValue::Integer(i) => Some(*i),
            AgentValue::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AgentValue::Integer(i) => Some(*i as f64),
            AgentValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            AgentValue::String(s) => Some(s),
            _ => None,
        }
    }

    #[cfg(feature = "image")]
    pub fn as_image(&self) -> Option<Arc<PhotonImage>> {
        match self {
            AgentValue::Image(img) => Some(img.clone()),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&AgentValueMap<String, AgentValue>> {
        match self {
            AgentValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<AgentValue>> {
        match self {
            AgentValue::Array(a) => Some(a),
            _ => None,
        }
    }

    #[allow(unused)]
    pub fn get(&self, key: &str) -> Option<&AgentValue> {
        self.as_object().and_then(|o| o.get(key))
    }

    #[allow(unused)]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    #[allow(unused)]
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    #[allow(unused)]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    #[allow(unused)]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    #[cfg(feature = "image")]
    #[allow(unused)]
    pub fn get_image(&self, key: &str) -> Option<Arc<PhotonImage>> {
        self.get(key).and_then(|v| v.as_image())
    }

    #[allow(unused)]
    pub fn get_object(&self, key: &str) -> Option<&AgentValueMap<String, AgentValue>> {
        self.get(key).and_then(|v| v.as_object())
    }

    #[allow(unused)]
    pub fn get_array(&self, key: &str) -> Option<&Vec<AgentValue>> {
        self.get(key).and_then(|v| v.as_array())
    }

    pub fn kind(&self) -> String {
        match self {
            AgentValue::Unit => "unit".to_string(),
            AgentValue::Boolean(_) => "boolean".to_string(),
            AgentValue::Integer(_) => "integer".to_string(),
            AgentValue::Number(_) => "number".to_string(),
            AgentValue::String(_) => "string".to_string(),
            #[cfg(feature = "image")]
            AgentValue::Image(_) => "image".to_string(),
            AgentValue::Object(_) => "object".to_string(),
            AgentValue::Array(arr) => {
                if arr.is_empty() {
                    "array".to_string()
                } else {
                    arr[0].kind()
                }
            }
        }
    }
}

impl Default for AgentValue {
    fn default() -> Self {
        AgentValue::Unit
    }
}

impl PartialEq for AgentValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AgentValue::Unit, AgentValue::Unit) => true,
            (AgentValue::Boolean(b1), AgentValue::Boolean(b2)) => b1 == b2,
            (AgentValue::Integer(i1), AgentValue::Integer(i2)) => i1 == i2,
            (AgentValue::Number(n1), AgentValue::Number(n2)) => n1 == n2,
            (AgentValue::String(s1), AgentValue::String(s2)) => s1 == s2,
            #[cfg(feature = "image")]
            (AgentValue::Image(i1), AgentValue::Image(i2)) => {
                i1.get_width() == i2.get_width()
                    && i1.get_height() == i2.get_height()
                    && i1.get_raw_pixels() == i2.get_raw_pixels()
            }
            (AgentValue::Object(o1), AgentValue::Object(o2)) => o1 == o2,
            (AgentValue::Array(a1), AgentValue::Array(a2)) => a1 == a2,
            _ => false,
        }
    }
}

impl Serialize for AgentValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            AgentValue::Unit => serializer.serialize_none(),
            AgentValue::Boolean(b) => serializer.serialize_bool(*b),
            AgentValue::Integer(i) => serializer.serialize_i64(*i),
            AgentValue::Number(n) => serializer.serialize_f64(*n),
            AgentValue::String(s) => serializer.serialize_str(s),
            #[cfg(feature = "image")]
            AgentValue::Image(img) => serializer.serialize_str(&img.get_base64()),
            AgentValue::Object(o) => {
                let mut map = serializer.serialize_map(Some(o.len()))?;
                for (k, v) in o.iter() {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            AgentValue::Array(a) => {
                let mut seq = serializer.serialize_seq(Some(a.len()))?;
                for e in a.iter() {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for AgentValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        AgentValue::from_json(value).map_err(|e| {
            serde::de::Error::custom(format!("Failed to deserialize AgentValue: {}", e))
        })
    }
}

impl From<()> for AgentValue {
    fn from(_: ()) -> Self {
        AgentValue::unit()
    }
}

impl From<bool> for AgentValue {
    fn from(value: bool) -> Self {
        AgentValue::boolean(value)
    }
}

impl From<i32> for AgentValue {
    fn from(value: i32) -> Self {
        AgentValue::integer(value as i64)
    }
}

impl From<i64> for AgentValue {
    fn from(value: i64) -> Self {
        AgentValue::integer(value)
    }
}

impl From<f64> for AgentValue {
    fn from(value: f64) -> Self {
        AgentValue::number(value)
    }
}

impl From<String> for AgentValue {
    fn from(value: String) -> Self {
        AgentValue::string(value)
    }
}

impl From<&str> for AgentValue {
    fn from(value: &str) -> Self {
        AgentValue::string(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_agent_data_new_constructors() {
        // Test all the constructor methods
        let unit_data = AgentData::unit();
        assert_eq!(unit_data.kind, "unit");
        assert_eq!(unit_data.value, AgentValue::Unit);

        let bool_data = AgentData::boolean(true);
        assert_eq!(bool_data.kind, "boolean");
        assert_eq!(bool_data.value, AgentValue::Boolean(true));

        let int_data = AgentData::integer(42);
        assert_eq!(int_data.kind, "integer");
        assert_eq!(int_data.value, AgentValue::Integer(42));

        let num_data = AgentData::number(3.14);
        assert_eq!(num_data.kind, "number");
        assert!(matches!(num_data.value, AgentValue::Number(_)));
        if let AgentValue::Number(num) = num_data.value {
            assert!((num - 3.14).abs() < f64::EPSILON);
        }

        let str_data = AgentData::string("hello".to_string());
        assert_eq!(str_data.kind, "string");
        assert!(matches!(str_data.value, AgentValue::String(_)));
        assert_eq!(str_data.as_str().unwrap(), "hello");

        let text_data = AgentData::string("multiline\ntext\n\n".to_string());
        assert_eq!(text_data.kind, "string");
        assert!(matches!(text_data.value, AgentValue::String(_)));
        assert_eq!(text_data.as_str().unwrap(), "multiline\ntext\n\n");

        #[cfg(feature = "image")]
        {
            let img_data = AgentData::image(PhotonImage::new(vec![0u8, 0u8, 0u8, 0u8], 1, 1));
            assert_eq!(img_data.kind, "image");
            assert!(matches!(img_data.value, AgentValue::Image(_)));
            assert_eq!(img_data.as_image().unwrap().get_width(), 1);
            assert_eq!(img_data.as_image().unwrap().get_height(), 1);
            assert_eq!(
                img_data.as_image().unwrap().get_raw_pixels(),
                vec![0u8, 0u8, 0u8, 0u8]
            );
        }

        let obj_val = [
            ("key1".to_string(), AgentValue::string("string1")),
            ("key2".to_string(), AgentValue::integer(2)),
        ];
        let obj_data = AgentData::object(obj_val.clone().into());
        assert_eq!(obj_data.kind, "object");
        assert!(matches!(obj_data.value, AgentValue::Object(_)));
        assert_eq!(obj_data.as_object().unwrap(), &obj_val.into());
    }

    #[test]
    fn test_agent_data_from_kind_value() {
        // Test creating AgentData from kind and JSON value
        let unit_data = AgentData::from_json_with_kind("unit", json!(null)).unwrap();
        assert_eq!(unit_data.kind, "unit");
        assert_eq!(unit_data.value, AgentValue::Unit);

        let bool_data = AgentData::from_json_with_kind("boolean", json!(true)).unwrap();
        assert_eq!(bool_data.kind, "boolean");
        assert_eq!(bool_data.value, AgentValue::Boolean(true));

        let int_data = AgentData::from_json_with_kind("integer", json!(42)).unwrap();
        assert_eq!(int_data.kind, "integer");
        assert_eq!(int_data.value, AgentValue::Integer(42));

        let int_data = AgentData::from_json_with_kind("integer", json!(3.14)).unwrap();
        assert_eq!(int_data.kind, "integer");
        assert_eq!(int_data.value, AgentValue::Integer(3));

        let num_data = AgentData::from_json_with_kind("number", json!(3.14)).unwrap();
        assert_eq!(num_data.kind, "number");
        assert_eq!(num_data.value, AgentValue::number(3.14));

        let num_data = AgentData::from_json_with_kind("number", json!(3)).unwrap();
        assert_eq!(num_data.kind, "number");
        assert_eq!(num_data.value, AgentValue::number(3.0));

        let str_data = AgentData::from_json_with_kind("string", json!("hello")).unwrap();
        assert_eq!(str_data.kind, "string");
        assert_eq!(str_data.value, AgentValue::string("hello"));

        let str_data = AgentData::from_json_with_kind("string", json!("hello\nworld\n\n")).unwrap();
        assert_eq!(str_data.kind, "string");
        assert_eq!(str_data.value, AgentValue::string("hello\nworld\n\n"));

        let text_data =
            AgentData::from_json_with_kind("string", json!("hello\nworld\n\n")).unwrap();
        assert_eq!(text_data.kind, "string");
        assert_eq!(text_data.value, AgentValue::string("hello\nworld\n\n"));

        #[cfg(feature = "image")]
        {
            let img_data = AgentData::from_json_with_kind("image",
        json!("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==")).unwrap();
            assert_eq!(img_data.kind, "image");
            assert!(matches!(img_data.value, AgentValue::Image(_)));
            assert_eq!(img_data.as_image().unwrap().get_width(), 1);
            assert_eq!(img_data.as_image().unwrap().get_height(), 1);
            assert_eq!(
                img_data.as_image().unwrap().get_raw_pixels(),
                vec![0u8, 0u8, 0u8, 0u8]
            );
        }

        let obj_data =
            AgentData::from_json_with_kind("object", json!({"key1": "string1", "key2": 2}))
                .unwrap();
        assert_eq!(obj_data.kind, "object");
        assert_eq!(
            obj_data.value,
            AgentValue::object(
                [
                    ("key1".to_string(), AgentValue::string("string1")),
                    ("key2".to_string(), AgentValue::integer(2)),
                ]
                .into()
            )
        );

        // Test custom object kind
        let obj_data = AgentData::from_json_with_kind(
            "custom_type".to_string(),
            json!({"foo": "hi", "bar": 3}),
        )
        .unwrap();
        assert_eq!(obj_data.kind, "custom_type");
        assert_eq!(
            obj_data.value,
            AgentValue::object(
                [
                    ("foo".to_string(), AgentValue::string("hi")),
                    ("bar".to_string(), AgentValue::integer(3)),
                ]
                .into()
            )
        );

        // Test array values
        let array_data = AgentData::from_json_with_kind("unit", json!([null, null])).unwrap();
        assert_eq!(array_data.kind, "unit");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![AgentValue::unit(), AgentValue::unit(),])
        );

        let array_data = AgentData::from_json_with_kind("boolean", json!([true, false])).unwrap();
        assert_eq!(array_data.kind, "boolean");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![AgentValue::boolean(true), AgentValue::boolean(false),])
        );

        let array_data = AgentData::from_json_with_kind("integer", json!([1, 2.1, 3.0])).unwrap();
        assert_eq!(array_data.kind, "integer");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::integer(1),
                AgentValue::integer(2),
                AgentValue::integer(3),
            ])
        );

        let array_data = AgentData::from_json_with_kind("number", json!([1.0, 2.1, 3])).unwrap();
        assert_eq!(array_data.kind, "number");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::number(1.0),
                AgentValue::number(2.1),
                AgentValue::number(3.0),
            ])
        );

        let array_data =
            AgentData::from_json_with_kind("string", json!(["test", "hello\nworld\n", ""]))
                .unwrap();
        assert_eq!(array_data.kind, "string");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::string("test"),
                AgentValue::string("hello\nworld\n"),
                AgentValue::string(""),
            ])
        );

        let array_data =
            AgentData::from_json_with_kind("string", json!(["test", "hello\nworld\n", ""]))
                .unwrap();
        assert_eq!(array_data.kind, "string");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::string("test"),
                AgentValue::string("hello\nworld\n"),
                AgentValue::string(""),
            ])
        );

        let array_data = AgentData::from_json_with_kind(
            "object",
            json!([{"key1":"test","key2":1}, {"key1":"test2","key2":"hi"}, {}]),
        )
        .unwrap();
        assert_eq!(array_data.kind, "object");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test")),
                        ("key2".to_string(), AgentValue::integer(1)),
                    ]
                    .into()
                ),
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test2")),
                        ("key2".to_string(), AgentValue::string("hi")),
                    ]
                    .into()
                ),
                AgentValue::object(AgentValueMap::default()),
            ])
        );

        let array_data = AgentData::from_json_with_kind(
            "custom",
            json!([{"key1":"test","key2":1}, {"key1":"test2","key2":"hi"}, {}]),
        )
        .unwrap();
        assert_eq!(array_data.kind, "custom");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test")),
                        ("key2".to_string(), AgentValue::integer(1)),
                    ]
                    .into()
                ),
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test2")),
                        ("key2".to_string(), AgentValue::string("hi")),
                    ]
                    .into()
                ),
                AgentValue::object(AgentValueMap::default()),
            ])
        );
    }

    #[test]
    fn test_agent_data_from_json_value() {
        // Test automatic kind inference from JSON values
        let unit_data = AgentData::from_json(json!(null)).unwrap();
        assert_eq!(unit_data.kind, "unit");
        assert_eq!(unit_data.value, AgentValue::Unit);

        let bool_data = AgentData::from_json(json!(true)).unwrap();
        assert_eq!(bool_data.kind, "boolean");
        assert_eq!(bool_data.value, AgentValue::Boolean(true));

        let int_data = AgentData::from_json(json!(42)).unwrap();
        assert_eq!(int_data.kind, "integer");
        assert_eq!(int_data.value, AgentValue::Integer(42));

        let num_data = AgentData::from_json(json!(3.14)).unwrap();
        assert_eq!(num_data.kind, "number");
        assert_eq!(num_data.value, AgentValue::number(3.14));

        let str_data = AgentData::from_json(json!("hello")).unwrap();
        assert_eq!(str_data.kind, "string");
        assert_eq!(str_data.value, AgentValue::string("hello"));

        let str_data = AgentData::from_json(json!("hello\nworld\n\n")).unwrap();
        assert_eq!(str_data.kind, "string");
        assert_eq!(str_data.value, AgentValue::string("hello\nworld\n\n"));

        #[cfg(feature = "image")]
        {
            let image_data = AgentData::from_json(json!(
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg=="
        ))
        .unwrap();
            assert_eq!(image_data.kind, "image");
            assert!(matches!(image_data.value, AgentValue::Image(_)));
            assert_eq!(image_data.as_image().unwrap().get_width(), 1);
            assert_eq!(image_data.as_image().unwrap().get_height(), 1);
            assert_eq!(
                image_data.as_image().unwrap().get_raw_pixels(),
                vec![0u8, 0u8, 0u8, 0u8]
            );
        }

        let obj_data = AgentData::from_json(json!({"key1": "string1", "key2": 2})).unwrap();
        assert_eq!(obj_data.kind, "object");
        assert_eq!(
            obj_data.value,
            AgentValue::object(
                [
                    ("key1".to_string(), AgentValue::string("string1")),
                    ("key2".to_string(), AgentValue::integer(2)),
                ]
                .into()
            )
        );

        // Test array values
        let array_data = AgentData::from_json(json!([null, null])).unwrap();
        assert_eq!(array_data.kind, "unit");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![AgentValue::unit(), AgentValue::unit(),])
        );

        let array_data = AgentData::from_json(json!([true, false])).unwrap();
        assert_eq!(array_data.kind, "boolean");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![AgentValue::boolean(true), AgentValue::boolean(false),])
        );

        let array_data = AgentData::from_json(json!([1, 2, 3])).unwrap();
        assert_eq!(array_data.kind, "integer");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::integer(1),
                AgentValue::integer(2),
                AgentValue::integer(3),
            ])
        );

        let array_data = AgentData::from_json(json!([1.0, 2.1, 3.2])).unwrap();
        assert_eq!(array_data.kind, "number");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::number(1.0),
                AgentValue::number(2.1),
                AgentValue::number(3.2),
            ])
        );

        let array_data = AgentData::from_json(json!(["test", "hello\nworld\n", ""])).unwrap();
        assert_eq!(array_data.kind, "string");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::string("test"),
                AgentValue::string("hello\nworld\n"),
                AgentValue::string(""),
            ])
        );

        let array_data = AgentData::from_json(
            json!([{"key1":"test","key2":1}, {"key1":"test2","key2":"hi"}, {}]),
        )
        .unwrap();
        assert_eq!(array_data.kind, "object");
        assert_eq!(
            array_data.value,
            AgentValue::array(vec![
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test")),
                        ("key2".to_string(), AgentValue::integer(1)),
                    ]
                    .into()
                ),
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test2")),
                        ("key2".to_string(), AgentValue::string("hi")),
                    ]
                    .into()
                ),
                AgentValue::object(AgentValueMap::default()),
            ])
        );
    }

    #[test]
    fn test_agent_data_accessor_methods() {
        // Test accessor methods
        let str_data = AgentData::string("hello".to_string());
        assert_eq!(str_data.as_str().unwrap(), "hello");
        assert!(str_data.as_object().is_none());

        let obj_val = [
            ("key1".to_string(), AgentValue::string("string1")),
            ("key2".to_string(), AgentValue::integer(2)),
        ];
        let obj_data = AgentData::object(obj_val.clone().into());
        assert!(obj_data.as_str().is_none());
        assert_eq!(obj_data.as_object().unwrap(), &obj_val.into());
    }

    #[test]
    fn test_agent_data_serialization() {
        // Test unit serialization
        {
            let data = AgentData::unit();
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"unit","value":null}"#
            );
        }

        // Test Boolean serialization
        {
            let data = AgentData::boolean(true);
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"boolean","value":true}"#
            );

            let data = AgentData::boolean(false);
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"boolean","value":false}"#
            );
        }

        // Test Integer serialization
        {
            let data = AgentData::integer(42);
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"integer","value":42}"#
            );
        }

        // Test Number serialization
        {
            let data = AgentData::number(3.14);
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"number","value":3.14}"#
            );

            let data = AgentData::number(3.0);
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"number","value":3.0}"#
            );
        }

        // Test String serialization
        {
            let data = AgentData::string("Hello, world!");
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"string","value":"Hello, world!"}"#
            );

            let data = AgentData::string("hello\nworld\n\n");
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"string","value":"hello\nworld\n\n"}"#
            );
        }

        // Test Image serialization
        #[cfg(feature = "image")]
        {
            let data = AgentData::image(PhotonImage::new(vec![0u8, 0u8, 0u8, 0u8], 1, 1));
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"image","value":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg=="}"#
            );
        }

        // Test Object serialization
        {
            let data = AgentData::object(
                [
                    ("key1".to_string(), AgentValue::string("string1")),
                    ("key2".to_string(), AgentValue::integer(2)),
                ]
                .into(),
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"object","value":{"key1":"string1","key2":2}}"#
            );
        }

        // Test custom object serialization
        {
            let data = AgentData::object_with_kind(
                "custom",
                [
                    ("key1".to_string(), AgentValue::string("test")),
                    ("key2".to_string(), AgentValue::integer(3)),
                ]
                .into(),
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"custom","value":{"key1":"test","key2":3}}"#
            );
        }

        // Test Array serialization
        {
            let data = AgentData::array("unit", vec![AgentValue::unit(), AgentValue::unit()]);
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"unit","value":[null,null]}"#
            );

            let data = AgentData::array(
                "boolean",
                vec![AgentValue::boolean(false), AgentValue::boolean(true)],
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"boolean","value":[false,true]}"#
            );

            let data = AgentData::array(
                "integer",
                vec![
                    AgentValue::integer(1),
                    AgentValue::integer(2),
                    AgentValue::integer(3),
                ],
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"integer","value":[1,2,3]}"#
            );

            let data = AgentData::array(
                "number",
                vec![
                    AgentValue::number(1.0),
                    AgentValue::number(2.1),
                    AgentValue::number(3.2),
                ],
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"number","value":[1.0,2.1,3.2]}"#
            );

            let data = AgentData::array(
                "string",
                vec![
                    AgentValue::string("test"),
                    AgentValue::string("hello\nworld\n"),
                    AgentValue::string(""),
                ],
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"string","value":["test","hello\nworld\n",""]}"#
            );

            let data = AgentData::array(
                "object",
                vec![
                    AgentValue::object(
                        [
                            ("key1".to_string(), AgentValue::string("test")),
                            ("key2".to_string(), AgentValue::integer(1)),
                        ]
                        .into(),
                    ),
                    AgentValue::object(
                        [
                            ("key1".to_string(), AgentValue::string("test2")),
                            ("key2".to_string(), AgentValue::string("hi")),
                        ]
                        .into(),
                    ),
                    AgentValue::object(AgentValueMap::default()),
                ],
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"object","value":[{"key1":"test","key2":1},{"key1":"test2","key2":"hi"},{}]}"#
            );

            let data = AgentData::array(
                "custom",
                vec![
                    AgentValue::object(
                        [
                            ("key1".to_string(), AgentValue::string("test")),
                            ("key2".to_string(), AgentValue::integer(1)),
                        ]
                        .into(),
                    ),
                    AgentValue::object(
                        [
                            ("key1".to_string(), AgentValue::string("test2")),
                            ("key2".to_string(), AgentValue::string("hi")),
                        ]
                        .into(),
                    ),
                    AgentValue::object(AgentValueMap::default()),
                ],
            );
            assert_eq!(
                serde_json::to_string(&data).unwrap(),
                r#"{"kind":"custom","value":[{"key1":"test","key2":1},{"key1":"test2","key2":"hi"},{}]}"#
            );
        }
    }

    #[test]
    fn test_agent_data_deserialization() {
        // Test unit deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"unit","value":null}"#).unwrap();
            assert_eq!(deserialized, AgentData::unit());
        }

        // Test Boolean deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"boolean","value":false}"#).unwrap();
            assert_eq!(deserialized, AgentData::boolean(false));

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"boolean","value":true}"#).unwrap();
            assert_eq!(deserialized, AgentData::boolean(true));
        }

        // Test Integer deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"integer","value":123}"#).unwrap();
            assert_eq!(deserialized, AgentData::integer(123));
        }

        // Test Number deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"number","value":3.14}"#).unwrap();
            assert_eq!(deserialized, AgentData::number(3.14));

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"number","value":3.0}"#).unwrap();
            assert_eq!(deserialized, AgentData::number(3.0));
        }

        // Test String deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"string","value":"Hello, world!"}"#).unwrap();
            assert_eq!(deserialized, AgentData::string("Hello, world!"));

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"string","value":"hello\nworld\n\n"}"#).unwrap();
            assert_eq!(deserialized, AgentData::string("hello\nworld\n\n"));
        }

        // Test Image deserialization
        #[cfg(feature = "image")]
        {
            let deserialized: AgentData = serde_json::from_str(
                r#"{"kind":"image","value":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg=="}"#,
            )
            .unwrap();
            assert_eq!(deserialized.kind, "image");
            assert!(matches!(deserialized.value, AgentValue::Image(_)));
        }

        // Test Object deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"object","value":{"key1":"test","key2":3}}"#)
                    .unwrap();
            assert_eq!(
                deserialized,
                AgentData::object(
                    [
                        ("key1".to_string(), AgentValue::string("test")),
                        ("key2".to_string(), AgentValue::integer(3))
                    ]
                    .into()
                )
            );
        }

        // Test custom object deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"custom","value":{"name":"test","value":3}}"#)
                    .unwrap();
            assert_eq!(
                deserialized,
                AgentData::object_with_kind(
                    "custom",
                    [
                        ("name".to_string(), AgentValue::string("test")),
                        ("value".to_string(), AgentValue::integer(3))
                    ]
                    .into()
                )
            );
        }

        // Test Array deserialization
        {
            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"unit","value":[null,null]}"#).unwrap();
            assert_eq!(
                deserialized,
                AgentData::array("unit", vec![AgentValue::unit(), AgentValue::unit(),])
            );

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"boolean","value":[true,false]}"#).unwrap();
            assert_eq!(
                deserialized,
                AgentData::array(
                    "boolean",
                    vec![AgentValue::boolean(true), AgentValue::boolean(false),]
                )
            );

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"integer","value":[1,2,3]}"#).unwrap();
            assert_eq!(
                deserialized,
                AgentData::array(
                    "integer",
                    vec![
                        AgentValue::integer(1),
                        AgentValue::integer(2),
                        AgentValue::integer(3),
                    ]
                )
            );

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"number","value":[1.0,2.1,3]}"#).unwrap();
            assert_eq!(
                deserialized,
                AgentData::array(
                    "number",
                    vec![
                        AgentValue::number(1.0),
                        AgentValue::number(2.1),
                        AgentValue::number(3.0),
                    ]
                )
            );

            let deserialized: AgentData =
                serde_json::from_str(r#"{"kind":"string","value":["test","hello\nworld\n",""]}"#)
                    .unwrap();
            assert_eq!(
                deserialized,
                AgentData::array(
                    "string",
                    vec![
                        AgentValue::string("test"),
                        AgentValue::string("hello\nworld\n"),
                        AgentValue::string(""),
                    ]
                )
            );

            let deserialized: AgentData =
                    serde_json::from_str(r#"{"kind":"object","value":[{"key1":"test","key2":1},{"key1":"test2","key2":"hi"},{}]}"#)
                        .unwrap();
            assert_eq!(
                deserialized,
                AgentData::array(
                    "object",
                    vec![
                        AgentValue::object(
                            [
                                ("key1".to_string(), AgentValue::string("test")),
                                ("key2".to_string(), AgentValue::integer(1)),
                            ]
                            .into()
                        ),
                        AgentValue::object(
                            [
                                ("key1".to_string(), AgentValue::string("test2")),
                                ("key2".to_string(), AgentValue::string("hi")),
                            ]
                            .into()
                        ),
                        AgentValue::object(AgentValueMap::default()),
                    ]
                )
            );

            let deserialized: AgentData =
                    serde_json::from_str(r#"{"kind":"custom","value":[{"key1":"test","key2":1},{"key1":"test2","key2":"hi"},{}]}"#)
                        .unwrap();
            assert_eq!(
                deserialized,
                AgentData::array(
                    "custom",
                    vec![
                        AgentValue::object(
                            [
                                ("key1".to_string(), AgentValue::string("test")),
                                ("key2".to_string(), AgentValue::integer(1)),
                            ]
                            .into()
                        ),
                        AgentValue::object(
                            [
                                ("key1".to_string(), AgentValue::string("test2")),
                                ("key2".to_string(), AgentValue::string("hi")),
                            ]
                            .into()
                        ),
                        AgentValue::object(AgentValueMap::default()),
                    ]
                )
            );
        }
    }

    #[test]
    fn test_agent_value_constructors() {
        // Test AgentValue constructors
        let unit = AgentValue::unit();
        assert_eq!(unit, AgentValue::Unit);

        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean, AgentValue::Boolean(true));

        let integer = AgentValue::integer(42);
        assert_eq!(integer, AgentValue::Integer(42));

        let number = AgentValue::number(3.14);
        assert!(matches!(number, AgentValue::Number(_)));
        if let AgentValue::Number(num) = number {
            assert!((num - 3.14).abs() < f64::EPSILON);
        }

        let string = AgentValue::string("hello");
        assert!(matches!(string, AgentValue::String(_)));
        assert_eq!(string.as_str().unwrap(), "hello");

        let text = AgentValue::string("multiline\ntext");
        assert!(matches!(text, AgentValue::String(_)));
        assert_eq!(text.as_str().unwrap(), "multiline\ntext");

        let array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        assert!(matches!(array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = array {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0].as_i64().unwrap(), 1);
            assert_eq!(arr[1].as_i64().unwrap(), 2);
        }

        let obj = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("string1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        assert!(matches!(obj, AgentValue::Object(_)));
        if let AgentValue::Object(obj) = obj {
            assert_eq!(obj.get("key1").and_then(|v| v.as_str()), Some("string1"));
            assert_eq!(obj.get("key2").and_then(|v| v.as_i64()), Some(2));
        } else {
            panic!("Object was not deserialized correctly");
        }
    }

    #[test]
    fn test_agent_value_from_json_value() {
        // Test converting from JSON value to AgentValue
        let null = AgentValue::from_json(json!(null)).unwrap();
        assert_eq!(null, AgentValue::Unit);

        let boolean = AgentValue::from_json(json!(true)).unwrap();
        assert_eq!(boolean, AgentValue::Boolean(true));

        let integer = AgentValue::from_json(json!(42)).unwrap();
        assert_eq!(integer, AgentValue::Integer(42));

        let number = AgentValue::from_json(json!(3.14)).unwrap();
        assert!(matches!(number, AgentValue::Number(_)));
        if let AgentValue::Number(num) = number {
            assert!((num - 3.14).abs() < f64::EPSILON);
        }

        let string = AgentValue::from_json(json!("hello")).unwrap();
        assert!(matches!(string, AgentValue::String(_)));
        if let AgentValue::String(s) = string {
            assert_eq!(*s, "hello");
        } else {
            panic!("Expected string value");
        }

        let array = AgentValue::from_json(json!([1, "test", true])).unwrap();
        assert!(matches!(array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = array {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], AgentValue::Integer(1));
            assert!(matches!(&arr[1], AgentValue::String(_)));
            if let AgentValue::String(s) = &arr[1] {
                assert_eq!(**s, "test");
            } else {
                panic!("Expected string value");
            }
            assert_eq!(arr[2], AgentValue::Boolean(true));
        }

        let object = AgentValue::from_json(json!({"key1": "string1", "key2": 2})).unwrap();
        assert!(matches!(object, AgentValue::Object(_)));
        if let AgentValue::Object(obj) = object {
            assert_eq!(obj.get("key1").and_then(|v| v.as_str()), Some("string1"));
            assert_eq!(obj.get("key2").and_then(|v| v.as_i64()), Some(2));
        } else {
            panic!("Object was not deserialized correctly");
        }
    }

    #[test]
    fn test_agent_value_from_kind_value() {
        // Test AgentValue::from_kind_value with different kinds and values
        let unit = AgentValue::from_kind_json("unit", json!(null)).unwrap();
        assert_eq!(unit, AgentValue::Unit);

        let boolean = AgentValue::from_kind_json("boolean", json!(true)).unwrap();
        assert_eq!(boolean, AgentValue::Boolean(true));

        let integer = AgentValue::from_kind_json("integer", json!(42)).unwrap();
        assert_eq!(integer, AgentValue::Integer(42));

        let integer = AgentValue::from_kind_json("integer", json!(42.0)).unwrap();
        assert_eq!(integer, AgentValue::Integer(42));

        let number = AgentValue::from_kind_json("number", json!(3.14)).unwrap();
        assert!(matches!(number, AgentValue::Number(_)));
        if let AgentValue::Number(num) = number {
            assert!((num - 3.14).abs() < f64::EPSILON);
        }

        let number = AgentValue::from_kind_json("number", json!(3)).unwrap();
        assert!(matches!(number, AgentValue::Number(_)));
        if let AgentValue::Number(num) = number {
            assert!((num - 3.0).abs() < f64::EPSILON);
        }

        let string = AgentValue::from_kind_json("string", json!("hello")).unwrap();
        assert!(matches!(string, AgentValue::String(_)));
        if let AgentValue::String(s) = string {
            assert_eq!(*s, "hello");
        } else {
            panic!("Expected string value");
        }

        let text = AgentValue::from_kind_json("string", json!("multiline\ntext")).unwrap();
        assert!(matches!(text, AgentValue::String(_)));
        if let AgentValue::String(t) = text {
            assert_eq!(*t, "multiline\ntext");
        } else {
            panic!("Expected text value");
        }

        let array = AgentValue::from_kind_json("array", json!([1, "test", true])).unwrap();
        assert!(matches!(array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = array {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], AgentValue::Integer(1));
            assert!(matches!(&arr[1], AgentValue::String(_)));
            if let AgentValue::String(s) = &arr[1] {
                assert_eq!(**s, "test");
            } else {
                panic!("Expected string value");
            }
            assert_eq!(arr[2], AgentValue::Boolean(true));
        }

        let obj = AgentValue::from_kind_json("object", json!({"key1": "test", "key2": 2})).unwrap();
        assert!(matches!(obj, AgentValue::Object(_)));
        if let AgentValue::Object(obj) = obj {
            assert_eq!(obj.get("key1").and_then(|v| v.as_str()), Some("test"));
            assert_eq!(obj.get("key2").and_then(|v| v.as_i64()), Some(2));
        } else {
            panic!("Object was not deserialized correctly");
        }

        // Test arrays
        let unit_array = AgentValue::from_kind_json("unit", json!([null, null])).unwrap();
        assert!(matches!(unit_array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = unit_array {
            assert_eq!(arr.len(), 2);
            for val in arr.iter() {
                assert_eq!(*val, AgentValue::Unit);
            }
        }

        let bool_array = AgentValue::from_kind_json("boolean", json!([true, false])).unwrap();
        assert!(matches!(bool_array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = bool_array {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], AgentValue::Boolean(true));
            assert_eq!(arr[1], AgentValue::Boolean(false));
        }

        let int_array = AgentValue::from_kind_json("integer", json!([1, 2, 3])).unwrap();
        assert!(matches!(int_array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = int_array {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], AgentValue::Integer(1));
            assert_eq!(arr[1], AgentValue::Integer(2));
            assert_eq!(arr[2], AgentValue::Integer(3));
        }

        let num_array = AgentValue::from_kind_json("number", json!([1.1, 2.2, 3.3])).unwrap();
        assert!(matches!(num_array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = num_array {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], AgentValue::Number(1.1));
            assert_eq!(arr[1], AgentValue::Number(2.2));
            assert_eq!(arr[2], AgentValue::Number(3.3));
        }

        let string_array = AgentValue::from_kind_json("string", json!(["hello", "world"])).unwrap();
        assert!(matches!(string_array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = string_array {
            assert_eq!(arr.len(), 2);
            assert!(matches!(&arr[0], AgentValue::String(_)));
            if let AgentValue::String(s) = &arr[0] {
                assert_eq!(**s, "hello".to_string());
            }
            assert!(matches!(&arr[1], AgentValue::String(_)));
            if let AgentValue::String(s) = &arr[1] {
                assert_eq!(**s, "world".to_string());
            }
        }

        let text_array =
            AgentValue::from_kind_json("string", json!(["hello", "world!\n"])).unwrap();
        assert!(matches!(text_array, AgentValue::Array(_)));
        if let AgentValue::Array(arr) = text_array {
            assert_eq!(arr.len(), 2);
            assert!(matches!(&arr[0], AgentValue::String(_)));
            if let AgentValue::String(s) = &arr[0] {
                assert_eq!(**s, "hello".to_string());
            }
            assert!(matches!(&arr[1], AgentValue::String(_)));
            if let AgentValue::String(s) = &arr[1] {
                assert_eq!(**s, "world!\n".to_string());
            }
        }

        // array_array

        // object_array
    }

    #[test]
    fn test_agent_value_test_methods() {
        // Test test methods on AgentValue
        let unit = AgentValue::unit();
        assert_eq!(unit.is_unit(), true);
        assert_eq!(unit.is_boolean(), false);
        assert_eq!(unit.is_integer(), false);
        assert_eq!(unit.is_number(), false);
        assert_eq!(unit.is_string(), false);
        assert_eq!(unit.is_array(), false);
        assert_eq!(unit.is_object(), false);

        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean.is_unit(), false);
        assert_eq!(boolean.is_boolean(), true);
        assert_eq!(boolean.is_integer(), false);
        assert_eq!(boolean.is_number(), false);
        assert_eq!(boolean.is_string(), false);
        assert_eq!(boolean.is_array(), false);
        assert_eq!(boolean.is_object(), false);

        let integer = AgentValue::integer(42);
        assert_eq!(integer.is_unit(), false);
        assert_eq!(integer.is_boolean(), false);
        assert_eq!(integer.is_integer(), true);
        assert_eq!(integer.is_number(), false);
        assert_eq!(integer.is_string(), false);
        assert_eq!(integer.is_array(), false);
        assert_eq!(integer.is_object(), false);

        let number = AgentValue::number(3.14);
        assert_eq!(number.is_unit(), false);
        assert_eq!(number.is_boolean(), false);
        assert_eq!(number.is_integer(), false);
        assert_eq!(number.is_number(), true);
        assert_eq!(number.is_string(), false);
        assert_eq!(number.is_array(), false);
        assert_eq!(number.is_object(), false);

        let string = AgentValue::string("hello");
        assert_eq!(string.is_unit(), false);
        assert_eq!(string.is_boolean(), false);
        assert_eq!(string.is_integer(), false);
        assert_eq!(string.is_number(), false);
        assert_eq!(string.is_string(), true);
        assert_eq!(string.is_array(), false);
        assert_eq!(string.is_object(), false);

        let array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        assert_eq!(array.is_unit(), false);
        assert_eq!(array.is_boolean(), false);
        assert_eq!(array.is_integer(), false);
        assert_eq!(array.is_number(), false);
        assert_eq!(array.is_string(), false);
        assert_eq!(array.is_array(), true);
        assert_eq!(array.is_object(), false);

        let obj = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("string1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        assert_eq!(obj.is_unit(), false);
        assert_eq!(obj.is_boolean(), false);
        assert_eq!(obj.is_integer(), false);
        assert_eq!(obj.is_number(), false);
        assert_eq!(obj.is_string(), false);
        assert_eq!(obj.is_array(), false);
        assert_eq!(obj.is_object(), true);
    }

    #[test]
    fn test_agent_value_accessor_methods() {
        // Test accessor methods on AgentValue
        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(boolean.as_i64(), None);
        assert_eq!(boolean.as_f64(), None);
        assert_eq!(boolean.as_str(), None);
        assert!(boolean.as_array().is_none());
        assert_eq!(boolean.as_object(), None);

        let integer = AgentValue::integer(42);
        assert_eq!(integer.as_bool(), None);
        assert_eq!(integer.as_i64(), Some(42));
        assert_eq!(integer.as_f64(), Some(42.0));
        assert_eq!(integer.as_str(), None);
        assert!(integer.as_array().is_none());
        assert_eq!(integer.as_object(), None);

        let number = AgentValue::number(3.14);
        assert_eq!(number.as_bool(), None);
        assert_eq!(number.as_i64(), Some(3)); // truncated
        assert_eq!(number.as_f64().unwrap(), 3.14);
        assert_eq!(number.as_str(), None);
        assert!(number.as_array().is_none());
        assert_eq!(number.as_object(), None);

        let string = AgentValue::string("hello");
        assert_eq!(string.as_bool(), None);
        assert_eq!(string.as_i64(), None);
        assert_eq!(string.as_f64(), None);
        assert_eq!(string.as_str(), Some("hello"));
        assert!(string.as_array().is_none());
        assert_eq!(string.as_object(), None);

        let array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        assert_eq!(array.as_bool(), None);
        assert_eq!(array.as_i64(), None);
        assert_eq!(array.as_f64(), None);
        assert_eq!(array.as_str(), None);
        assert!(array.as_array().is_some());
        if let Some(arr) = array.as_array() {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0].as_i64().unwrap(), 1);
            assert_eq!(arr[1].as_i64().unwrap(), 2);
        }
        assert_eq!(array.as_object(), None);

        let obj = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("string1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        assert_eq!(obj.as_bool(), None);
        assert_eq!(obj.as_i64(), None);
        assert_eq!(obj.as_f64(), None);
        assert_eq!(obj.as_str(), None);
        assert!(obj.as_array().is_none());
        assert!(obj.as_object().is_some());
        if let Some(value) = obj.as_object() {
            assert_eq!(value.get("key1").and_then(|v| v.as_str()), Some("string1"));
            assert_eq!(value.get("key2").and_then(|v| v.as_i64()), Some(2));
        }
    }

    #[test]
    fn test_agent_value_default() {
        assert_eq!(AgentValue::default(), AgentValue::Unit);
    }

    #[test]
    fn test_agent_value_serialization() {
        // Test Null serialization
        {
            let null = AgentValue::Unit;
            assert_eq!(serde_json::to_string(&null).unwrap(), "null");
        }

        // Test Boolean serialization
        {
            let boolean_t = AgentValue::boolean(true);
            assert_eq!(serde_json::to_string(&boolean_t).unwrap(), "true");

            let boolean_f = AgentValue::boolean(false);
            assert_eq!(serde_json::to_string(&boolean_f).unwrap(), "false");
        }

        // Test Integer serialization
        {
            let integer = AgentValue::integer(42);
            assert_eq!(serde_json::to_string(&integer).unwrap(), "42");
        }

        // Test Number serialization
        {
            let num = AgentValue::number(3.14);
            assert_eq!(serde_json::to_string(&num).unwrap(), "3.14");

            let num = AgentValue::number(3.0);
            assert_eq!(serde_json::to_string(&num).unwrap(), "3.0");
        }

        // Test String serialization
        {
            let s = AgentValue::string("Hello, world!");
            assert_eq!(serde_json::to_string(&s).unwrap(), "\"Hello, world!\"");

            let s = AgentValue::string("hello\nworld\n\n");
            assert_eq!(serde_json::to_string(&s).unwrap(), r#""hello\nworld\n\n""#);
        }

        // Test Image serialization
        #[cfg(feature = "image")]
        {
            let img = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(
                serde_json::to_string(&img).unwrap(),
                r#""data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==""#
            );
        }

        // Test Array serialization
        {
            let array = AgentValue::array(vec![
                AgentValue::integer(1),
                AgentValue::string("test"),
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test")),
                        ("key2".to_string(), AgentValue::integer(2)),
                    ]
                    .into(),
                ),
            ]);
            assert_eq!(
                serde_json::to_string(&array).unwrap(),
                r#"[1,"test",{"key1":"test","key2":2}]"#
            );
        }

        // Test Object serialization
        {
            let obj = AgentValue::object(
                [
                    ("key1".to_string(), AgentValue::string("test")),
                    ("key2".to_string(), AgentValue::integer(3)),
                ]
                .into(),
            );
            assert_eq!(
                serde_json::to_string(&obj).unwrap(),
                r#"{"key1":"test","key2":3}"#
            );
        }
    }

    #[test]
    fn test_agent_value_deserialization() {
        // Test Null deserialization
        {
            let deserialized: AgentValue = serde_json::from_str("null").unwrap();
            assert_eq!(deserialized, AgentValue::Unit);
        }

        // Test Boolean deserialization
        {
            let deserialized: AgentValue = serde_json::from_str("false").unwrap();
            assert_eq!(deserialized, AgentValue::boolean(false));

            let deserialized: AgentValue = serde_json::from_str("true").unwrap();
            assert_eq!(deserialized, AgentValue::boolean(true));
        }

        // Test Integer deserialization
        {
            let deserialized: AgentValue = serde_json::from_str("123").unwrap();
            assert_eq!(deserialized, AgentValue::integer(123));
        }

        // Test Number deserialization
        {
            let deserialized: AgentValue = serde_json::from_str("3.14").unwrap();
            assert_eq!(deserialized, AgentValue::number(3.14));

            let deserialized: AgentValue = serde_json::from_str("3.0").unwrap();
            assert_eq!(deserialized, AgentValue::number(3.0));
        }

        // Test String deserialization
        {
            let deserialized: AgentValue = serde_json::from_str("\"Hello, world!\"").unwrap();
            assert_eq!(deserialized, AgentValue::string("Hello, world!"));

            let deserialized: AgentValue = serde_json::from_str(r#""hello\nworld\n\n""#).unwrap();
            assert_eq!(deserialized, AgentValue::string("hello\nworld\n\n"));
        }

        // Test Image deserialization
        #[cfg(feature = "image")]
        {
            let deserialized: AgentValue = serde_json::from_str(
                r#""data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==""#,
            )
            .unwrap();
            assert!(matches!(deserialized, AgentValue::Image(_)));
        }

        // Test Array deserialization
        {
            let deserialized: AgentValue =
                serde_json::from_str(r#"[1,"test",{"key1":"test","key2":2}]"#).unwrap();
            assert!(matches!(deserialized, AgentValue::Array(_)));
            if let AgentValue::Array(arr) = deserialized {
                assert_eq!(arr.len(), 3, "Array length mismatch after serialization");
                assert_eq!(arr[0], AgentValue::integer(1));
                assert_eq!(arr[1], AgentValue::string("test"));
                assert_eq!(
                    arr[2],
                    AgentValue::object(
                        [
                            ("key1".to_string(), AgentValue::string("test")),
                            ("key2".to_string(), AgentValue::integer(2)),
                        ]
                        .into()
                    )
                );
            }
        }

        // Test Object deserialization
        {
            let deserialized: AgentValue =
                serde_json::from_str(r#"{"key1":"test","key2":3}"#).unwrap();
            assert_eq!(
                deserialized,
                AgentValue::object(
                    [
                        ("key1".to_string(), AgentValue::string("test")),
                        ("key2".to_string(), AgentValue::integer(3)),
                    ]
                    .into()
                )
            );
        }
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct TestStruct {
            name: String,
            age: i64,
            active: bool,
        }

        let test_data = TestStruct {
            name: "Alice".to_string(),
            age: 30,
            active: true,
        };

        // Test AgentData roundtrip
        let agent_data = AgentData::from_serialize(&test_data).unwrap();
        assert_eq!(agent_data.kind, "object");
        assert_eq!(agent_data.get_str("name"), Some("Alice"));
        assert_eq!(agent_data.get_i64("age"), Some(30));
        assert_eq!(agent_data.get_bool("active"), Some(true));

        let restored: TestStruct = agent_data.to_deserialize().unwrap();
        assert_eq!(restored, test_data);

        // Test AgentData with custom kind
        let agent_data_custom = AgentData::from_serialize_with_kind("person", &test_data).unwrap();
        assert_eq!(agent_data_custom.kind, "person");
        let restored_custom: TestStruct = agent_data_custom.to_deserialize().unwrap();
        assert_eq!(restored_custom, test_data);

        // Test AgentValue roundtrip
        let agent_value = AgentValue::from_serialize(&test_data).unwrap();
        assert!(agent_value.is_object());
        assert_eq!(agent_value.get_str("name"), Some("Alice"));

        let restored_value: TestStruct = agent_value.to_deserialize().unwrap();
        assert_eq!(restored_value, test_data);
    }

    #[test]
    fn test_serialize_deserialize_nested() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct Address {
            street: String,
            city: String,
            zip: String,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct Person {
            name: String,
            age: i64,
            address: Address,
            tags: Vec<String>,
        }

        let person = Person {
            name: "Bob".to_string(),
            age: 25,
            address: Address {
                street: "123 Main St".to_string(),
                city: "Springfield".to_string(),
                zip: "12345".to_string(),
            },
            tags: vec!["developer".to_string(), "rust".to_string()],
        };

        // Test AgentData roundtrip with nested structures
        let agent_data = AgentData::from_serialize(&person).unwrap();
        assert_eq!(agent_data.kind, "object");
        assert_eq!(agent_data.get_str("name"), Some("Bob"));

        let address = agent_data.get_object("address").unwrap();
        assert_eq!(
            address.get("city").and_then(|v| v.as_str()),
            Some("Springfield")
        );

        let tags = agent_data.get_array("tags").unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str(), Some("developer"));

        let restored: Person = agent_data.to_deserialize().unwrap();
        assert_eq!(restored, person);
    }
}
