use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};

use crate::error::AgentError;

#[cfg(feature = "image")]
const IMAGE_BASE64_PREFIX: &str = "data:image/png;base64,";

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
                    Err(AgentError::InvalidValue(
                        "Invalid numeric value for AgentValue".into(),
                    ))
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

    pub fn is_unit(&self) -> bool {
        matches!(self, AgentValue::Unit)
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, AgentValue::Boolean(_))
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, AgentValue::Integer(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, AgentValue::Number(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, AgentValue::String(_))
    }

    #[cfg(feature = "image")]
    pub fn is_image(&self) -> bool {
        matches!(self, AgentValue::Image(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, AgentValue::Array(_))
    }

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

    pub fn as_object_mut(&mut self) -> Option<&mut AgentValueMap<String, AgentValue>> {
        match self {
            AgentValue::Object(o) => Some(Arc::make_mut(o)),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<AgentValue>> {
        match self {
            AgentValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<AgentValue>> {
        match self {
            AgentValue::Array(a) => Some(Arc::make_mut(a)),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&AgentValue> {
        self.as_object().and_then(|o| o.get(key))
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut AgentValue> {
        self.as_object_mut().and_then(|o| o.get_mut(key))
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    #[cfg(feature = "image")]
    pub fn get_image(&self, key: &str) -> Option<Arc<PhotonImage>> {
        self.get(key).and_then(|v| v.as_image())
    }

    pub fn get_object(&self, key: &str) -> Option<&AgentValueMap<String, AgentValue>> {
        self.get(key).and_then(|v| v.as_object())
    }

    pub fn get_object_mut(&mut self, key: &str) -> Option<&mut AgentValueMap<String, AgentValue>> {
        self.get_mut(key).and_then(|v| v.as_object_mut())
    }

    pub fn get_array(&self, key: &str) -> Option<&Vec<AgentValue>> {
        self.get(key).and_then(|v| v.as_array())
    }

    pub fn get_array_mut(&mut self, key: &str) -> Option<&mut Vec<AgentValue>> {
        self.get_mut(key).and_then(|v| v.as_array_mut())
    }

    pub fn set(&mut self, key: String, value: AgentValue) -> Result<(), AgentError> {
        if let Some(obj) = self.as_object_mut() {
            obj.insert(key, value);
            Ok(())
        } else {
            Err(AgentError::InvalidValue(
                "set can only be called on Object AgentValue".into(),
            ))
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
    fn test_partial_eq() {
        // Test PartialEq implementation
        let unit1 = AgentValue::unit();
        let unit2 = AgentValue::unit();
        assert_eq!(unit1, unit2);

        let boolean1 = AgentValue::boolean(true);
        let boolean2 = AgentValue::boolean(true);
        assert_eq!(boolean1, boolean2);

        let integer1 = AgentValue::integer(42);
        let integer2 = AgentValue::integer(42);
        assert_eq!(integer1, integer2);
        let different = AgentValue::integer(100);
        assert_ne!(integer1, different);

        let number1 = AgentValue::number(3.14);
        let number2 = AgentValue::number(3.14);
        assert_eq!(number1, number2);

        let string1 = AgentValue::string("hello");
        let string2 = AgentValue::string("hello");
        assert_eq!(string1, string2);

        #[cfg(feature = "image")]
        {
            let image1 = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            let image2 = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(image1, image2);
        }

        let obj1 = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("value1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        let obj2 = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("value1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        assert_eq!(obj1, obj2);

        let arr1 = AgentValue::array(vec![
            AgentValue::integer(1),
            AgentValue::string("two"),
            AgentValue::boolean(true),
        ]);
        let arr2 = AgentValue::array(vec![
            AgentValue::integer(1),
            AgentValue::string("two"),
            AgentValue::boolean(true),
        ]);
        assert_eq!(arr1, arr2);

        let mixed_types_1 = AgentValue::boolean(true);
        let mixed_types_2 = AgentValue::integer(1);
        assert_ne!(mixed_types_1, mixed_types_2);
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
        #[cfg(feature = "image")]
        assert_eq!(unit.is_image(), false);

        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean.is_unit(), false);
        assert_eq!(boolean.is_boolean(), true);
        assert_eq!(boolean.is_integer(), false);
        assert_eq!(boolean.is_number(), false);
        assert_eq!(boolean.is_string(), false);
        assert_eq!(boolean.is_array(), false);
        assert_eq!(boolean.is_object(), false);
        #[cfg(feature = "image")]
        assert_eq!(boolean.is_image(), false);

        let integer = AgentValue::integer(42);
        assert_eq!(integer.is_unit(), false);
        assert_eq!(integer.is_boolean(), false);
        assert_eq!(integer.is_integer(), true);
        assert_eq!(integer.is_number(), false);
        assert_eq!(integer.is_string(), false);
        assert_eq!(integer.is_array(), false);
        assert_eq!(integer.is_object(), false);
        #[cfg(feature = "image")]
        assert_eq!(integer.is_image(), false);

        let number = AgentValue::number(3.14);
        assert_eq!(number.is_unit(), false);
        assert_eq!(number.is_boolean(), false);
        assert_eq!(number.is_integer(), false);
        assert_eq!(number.is_number(), true);
        assert_eq!(number.is_string(), false);
        assert_eq!(number.is_array(), false);
        assert_eq!(number.is_object(), false);
        #[cfg(feature = "image")]
        assert_eq!(number.is_image(), false);

        let string = AgentValue::string("hello");
        assert_eq!(string.is_unit(), false);
        assert_eq!(string.is_boolean(), false);
        assert_eq!(string.is_integer(), false);
        assert_eq!(string.is_number(), false);
        assert_eq!(string.is_string(), true);
        assert_eq!(string.is_array(), false);
        assert_eq!(string.is_object(), false);
        #[cfg(feature = "image")]
        assert_eq!(string.is_image(), false);

        let array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        assert_eq!(array.is_unit(), false);
        assert_eq!(array.is_boolean(), false);
        assert_eq!(array.is_integer(), false);
        assert_eq!(array.is_number(), false);
        assert_eq!(array.is_string(), false);
        assert_eq!(array.is_array(), true);
        assert_eq!(array.is_object(), false);
        #[cfg(feature = "image")]
        assert_eq!(array.is_image(), false);

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
        #[cfg(feature = "image")]
        assert_eq!(obj.is_image(), false);

        #[cfg(feature = "image")]
        {
            let img = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(img.is_unit(), false);
            assert_eq!(img.is_boolean(), false);
            assert_eq!(img.is_integer(), false);
            assert_eq!(img.is_number(), false);
            assert_eq!(img.is_string(), false);
            assert_eq!(img.is_array(), false);
            assert_eq!(img.is_object(), false);
            assert_eq!(img.is_image(), true);
        }
    }

    #[test]
    fn test_agent_value_as_methods() {
        // Test accessor methods on AgentValue
        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(boolean.as_i64(), None);
        assert_eq!(boolean.as_f64(), None);
        assert_eq!(boolean.as_str(), None);
        assert!(boolean.as_array().is_none());
        assert_eq!(boolean.as_object(), None);
        #[cfg(feature = "image")]
        assert!(boolean.as_image().is_none());

        let integer = AgentValue::integer(42);
        assert_eq!(integer.as_bool(), None);
        assert_eq!(integer.as_i64(), Some(42));
        assert_eq!(integer.as_f64(), Some(42.0));
        assert_eq!(integer.as_str(), None);
        assert!(integer.as_array().is_none());
        assert_eq!(integer.as_object(), None);
        #[cfg(feature = "image")]
        assert!(integer.as_image().is_none());

        let number = AgentValue::number(3.14);
        assert_eq!(number.as_bool(), None);
        assert_eq!(number.as_i64(), Some(3)); // truncated
        assert_eq!(number.as_f64().unwrap(), 3.14);
        assert_eq!(number.as_str(), None);
        assert!(number.as_array().is_none());
        assert_eq!(number.as_object(), None);
        #[cfg(feature = "image")]
        assert!(number.as_image().is_none());

        let string = AgentValue::string("hello");
        assert_eq!(string.as_bool(), None);
        assert_eq!(string.as_i64(), None);
        assert_eq!(string.as_f64(), None);
        assert_eq!(string.as_str(), Some("hello"));
        assert!(string.as_array().is_none());
        assert_eq!(string.as_object(), None);
        #[cfg(feature = "image")]
        assert!(string.as_image().is_none());

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
        #[cfg(feature = "image")]
        assert!(array.as_image().is_none());

        let mut array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        if let Some(arr) = array.as_array_mut() {
            arr.push(AgentValue::integer(3));
        }

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
        #[cfg(feature = "image")]
        assert!(obj.as_image().is_none());

        let mut obj = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("string1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        if let Some(value) = obj.as_object_mut() {
            value.insert("key3".to_string(), AgentValue::boolean(true));
        }

        #[cfg(feature = "image")]
        {
            let img = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(img.as_bool(), None);
            assert_eq!(img.as_i64(), None);
            assert_eq!(img.as_f64(), None);
            assert_eq!(img.as_str(), None);
            assert!(img.as_array().is_none());
            assert_eq!(img.as_object(), None);
            assert!(img.as_image().is_some());
        }
    }

    #[test]
    fn test_agent_value_get_methods() {
        // Test get methods on AgentValue
        const KEY: &str = "key";

        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean.get(KEY), None);

        let integer = AgentValue::integer(42);
        assert_eq!(integer.get(KEY), None);

        let number = AgentValue::number(3.14);
        assert_eq!(number.get(KEY), None);

        let string = AgentValue::string("hello");
        assert_eq!(string.get(KEY), None);

        let array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        assert_eq!(array.get(KEY), None);

        let mut array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::integer(2)]);
        assert_eq!(array.get_mut(KEY), None);

        let mut obj = AgentValue::object(
            [
                ("k_boolean".to_string(), AgentValue::boolean(true)),
                ("k_integer".to_string(), AgentValue::integer(42)),
                ("k_number".to_string(), AgentValue::number(3.14)),
                ("k_string".to_string(), AgentValue::string("string1")),
                (
                    "k_array".to_string(),
                    AgentValue::array(vec![AgentValue::integer(1)]),
                ),
                (
                    "k_object".to_string(),
                    AgentValue::object(
                        [("inner_key".to_string(), AgentValue::integer(100))].into(),
                    ),
                ),
                #[cfg(feature = "image")]
                (
                    "k_image".to_string(),
                    AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1)),
                ),
            ]
            .into(),
        );
        assert_eq!(obj.get(KEY), None);
        assert_eq!(obj.get_bool("k_boolean"), Some(true));
        assert_eq!(obj.get_i64("k_integer"), Some(42));
        assert_eq!(obj.get_f64("k_number"), Some(3.14));
        assert_eq!(obj.get_str("k_string"), Some("string1"));
        assert!(obj.get_array("k_array").is_some());
        assert!(obj.get_array_mut("k_array").is_some());
        assert!(obj.get_object("k_object").is_some());
        assert!(obj.get_object_mut("k_object").is_some());
        #[cfg(feature = "image")]
        assert!(obj.get_image("k_image").is_some());

        #[cfg(feature = "image")]
        {
            let img = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(img.get(KEY), None);
        }
    }

    #[test]
    fn test_agent_value_set() {
        // Test set method on AgentValue
        let mut obj = AgentValue::object(AgentValueMap::new());
        assert!(obj.set("key1".to_string(), AgentValue::integer(42)).is_ok());
        assert_eq!(obj.get_i64("key1"), Some(42));

        let mut not_obj = AgentValue::integer(10);
        assert!(
            not_obj
                .set("key1".to_string(), AgentValue::integer(42))
                .is_err()
        );
    }

    #[test]
    fn test_agent_value_default() {
        assert_eq!(AgentValue::default(), AgentValue::Unit);

        assert_eq!(AgentValue::boolean_default(), AgentValue::Boolean(false));
        assert_eq!(AgentValue::integer_default(), AgentValue::Integer(0));
        assert_eq!(AgentValue::number_default(), AgentValue::Number(0.0));
        assert_eq!(
            AgentValue::string_default(),
            AgentValue::String(Arc::new(String::new()))
        );
        assert_eq!(
            AgentValue::array_default(),
            AgentValue::Array(Arc::new(Vec::new()))
        );
        assert_eq!(
            AgentValue::object_default(),
            AgentValue::Object(Arc::new(AgentValueMap::new()))
        );

        #[cfg(feature = "image")]
        {
            assert_eq!(
                AgentValue::image_default(),
                AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1))
            );
        }
    }

    #[test]
    fn test_to_json() {
        // Test to_json
        let unit = AgentValue::unit();
        assert_eq!(unit.to_json(), json!(null));

        let boolean = AgentValue::boolean(true);
        assert_eq!(boolean.to_json(), json!(true));

        let integer = AgentValue::integer(42);
        assert_eq!(integer.to_json(), json!(42));

        let number = AgentValue::number(3.14);
        assert_eq!(number.to_json(), json!(3.14));

        let string = AgentValue::string("hello");
        assert_eq!(string.to_json(), json!("hello"));

        let array = AgentValue::array(vec![AgentValue::integer(1), AgentValue::string("test")]);
        assert_eq!(array.to_json(), json!([1, "test"]));

        let obj = AgentValue::object(
            [
                ("key1".to_string(), AgentValue::string("string1")),
                ("key2".to_string(), AgentValue::integer(2)),
            ]
            .into(),
        );
        assert_eq!(obj.to_json(), json!({"key1": "string1", "key2": 2}));

        #[cfg(feature = "image")]
        {
            let img = AgentValue::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(
                img.to_json(),
                json!(
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg=="
                )
            );
        }
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

        // Test Arc Image serialization
        #[cfg(feature = "image")]
        {
            let img = AgentValue::image_arc(Arc::new(PhotonImage::new(vec![0u8; 4], 1, 1)));
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
    fn test_agent_value_into() {
        // Test From implementations for AgentValue
        let from_unit: AgentValue = ().into();
        assert_eq!(from_unit, AgentValue::Unit);

        let from_bool: AgentValue = true.into();
        assert_eq!(from_bool, AgentValue::Boolean(true));

        let from_i32: AgentValue = 42i32.into();
        assert_eq!(from_i32, AgentValue::Integer(42));

        let from_i64: AgentValue = 100i64.into();
        assert_eq!(from_i64, AgentValue::Integer(100));

        let from_f64: AgentValue = 3.14f64.into();
        assert_eq!(from_f64, AgentValue::Number(3.14));

        let from_string: AgentValue = "hello".to_string().into();
        assert_eq!(
            from_string,
            AgentValue::String(Arc::new("hello".to_string()))
        );

        let from_str: AgentValue = "world".into();
        assert_eq!(from_str, AgentValue::String(Arc::new("world".to_string())));
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
        let agent_data = AgentValue::from_serialize(&test_data).unwrap();
        assert_eq!(agent_data.get_str("name"), Some("Alice"));
        assert_eq!(agent_data.get_i64("age"), Some(30));
        assert_eq!(agent_data.get_bool("active"), Some(true));

        let restored: TestStruct = agent_data.to_deserialize().unwrap();
        assert_eq!(restored, test_data);
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
        let agent_data = AgentValue::from_serialize(&person).unwrap();
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
