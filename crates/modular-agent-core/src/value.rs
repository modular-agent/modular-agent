use std::sync::Arc;

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

use im::{HashMap, Vector};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};

use crate::error::{Error, Result};
#[cfg(feature = "llm")]
use crate::llm::Message;

#[cfg(feature = "image")]
pub(crate) const IMAGE_BASE64_PREFIX: &str = "data:image/png;base64,";

/// Decodes a base64 image string — with or without a `data:<mime>;base64,`
/// prefix — into a `PhotonImage`. Unlike `PhotonImage::new_from_base64`,
/// malformed base64 or non-image bytes are an error, not a panic.
#[cfg(feature = "image")]
pub(crate) fn image_from_base64(s: &str) -> Result<PhotonImage> {
    use base64::Engine as _;
    use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};

    // Padding-indifferent: the previous photon/base64-0.13 path accepted
    // unpadded input, so external payloads must keep decoding.
    const B64: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );

    let payload = match s.split_once(";base64,") {
        Some((prefix, rest)) if prefix.starts_with("data:") => rest,
        _ => s,
    };
    let bytes = B64
        .decode(payload.trim())
        .map_err(|e| Error::InvalidValue(format!("Invalid base64 image data: {e}")))?;
    photon_rs::native::open_image_from_bytes(&bytes)
        .map_err(|e| Error::InvalidValue(format!("Invalid image data: {e}")))
}

/// The value type passed between modules.
///
/// Supports multiple data types with immutable data structures for efficient cloning.
/// Large data (String, Image, Tensor, etc.) is wrapped in `Arc` for reference-counted sharing.
#[derive(Debug, Clone, Default)]
pub enum Value {
    /// Empty value. Used as a trigger signal.
    #[default]
    Unit,

    /// Boolean value.
    Boolean(bool),

    /// 64-bit signed integer.
    Integer(i64),

    /// 64-bit floating point number.
    Number(f64),

    /// UTF-8 string wrapped in `Arc` for efficient cloning.
    String(Arc<String>),

    /// Image data (requires `image` feature).
    #[cfg(feature = "image")]
    Image(Arc<PhotonImage>),

    /// Ordered array of values.
    Array(Vector<Value>),

    /// Key-value map.
    Object(HashMap<String, Value>),

    /// Tensor data for embeddings, etc.
    Tensor(Arc<Vec<f32>>),

    /// LLM chat message (requires `llm` feature).
    #[cfg(feature = "llm")]
    Message(Arc<Message>),

    /// Error value for propagating errors through the workflow.
    Error(Arc<Error>),
}

/// Type alias for key-value maps used in `Value::Object`.
pub type ValueMap<S, T> = HashMap<S, T>;

impl Value {
    /// Creates a `Unit` value.
    pub fn unit() -> Self {
        Value::Unit
    }

    /// Creates a `Boolean` value.
    pub fn boolean(value: bool) -> Self {
        Value::Boolean(value)
    }

    /// Creates an `Integer` value.
    pub fn integer(value: i64) -> Self {
        Value::Integer(value)
    }

    /// Creates a `Number` value.
    pub fn number(value: f64) -> Self {
        Value::Number(value)
    }

    /// Creates a `String` value.
    pub fn string(value: impl Into<String>) -> Self {
        Value::String(Arc::new(value.into()))
    }

    /// Creates an `Image` value from a `PhotonImage`.
    #[cfg(feature = "image")]
    pub fn image(value: PhotonImage) -> Self {
        Value::Image(Arc::new(value))
    }

    /// Creates an `Image` value from an `Arc<PhotonImage>`.
    #[cfg(feature = "image")]
    pub fn image_arc(value: Arc<PhotonImage>) -> Self {
        Value::Image(value)
    }

    /// Creates an `Array` value.
    pub fn array(value: Vector<Value>) -> Self {
        Value::Array(value)
    }

    /// Creates an `Object` value.
    pub fn object(value: ValueMap<String, Value>) -> Self {
        Value::Object(value)
    }

    /// Creates a `Tensor` value from a `Vec<f32>`.
    pub fn tensor(value: Vec<f32>) -> Self {
        Value::Tensor(Arc::new(value))
    }

    /// Creates a `Message` value.
    #[cfg(feature = "llm")]
    pub fn message(value: Message) -> Self {
        Value::Message(Arc::new(value))
    }

    /// Creates a default `Boolean` value (`false`).
    pub fn boolean_default() -> Self {
        Value::Boolean(false)
    }

    /// Creates a default `Integer` value (`0`).
    pub fn integer_default() -> Self {
        Value::Integer(0)
    }

    /// Creates a default `Number` value (`0.0`).
    pub fn number_default() -> Self {
        Value::Number(0.0)
    }

    /// Creates a default `String` value (empty string).
    pub fn string_default() -> Self {
        Value::String(Arc::new(String::new()))
    }

    /// Creates a default `Image` value (1x1 transparent pixel).
    #[cfg(feature = "image")]
    pub fn image_default() -> Self {
        Value::Image(Arc::new(PhotonImage::new(vec![0u8, 0u8, 0u8, 0u8], 1, 1)))
    }

    /// Creates a default `Array` value (empty array).
    pub fn array_default() -> Self {
        Value::Array(Vector::new())
    }

    /// Creates a default `Object` value (empty object).
    pub fn object_default() -> Self {
        Value::Object(HashMap::new())
    }

    /// Creates a default `Tensor` value (empty vector).
    pub fn tensor_default() -> Self {
        Value::Tensor(Arc::new(Vec::new()))
    }

    /// Creates a `Value` from a `serde_json::Value`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidValue` if the JSON value cannot be converted.
    pub fn from_json(value: serde_json::Value) -> Result<Self> {
        match value {
            serde_json::Value::Null => Ok(Value::Unit),
            serde_json::Value::Bool(b) => Ok(Value::Boolean(b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Value::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Number(f))
                } else {
                    Err(Error::InvalidValue(
                        "Invalid numeric value for Value".into(),
                    ))
                }
            }
            serde_json::Value::String(s) => {
                #[cfg(feature = "image")]
                if s.starts_with(IMAGE_BASE64_PREFIX) {
                    let img = image_from_base64(&s)?;
                    Ok(Value::Image(Arc::new(img)))
                } else {
                    Ok(Value::String(Arc::new(s)))
                }
                #[cfg(not(feature = "image"))]
                Ok(Value::String(Arc::new(s)))
            }
            serde_json::Value::Array(arr) => {
                let module_arr: Vector<Value> = arr
                    .into_iter()
                    .map(Value::from_json)
                    .collect::<Result<_, _>>()?;
                Ok(Value::Array(module_arr))
            }
            serde_json::Value::Object(obj) => {
                let map: HashMap<String, Value> = obj
                    .into_iter()
                    .map(|(k, v)| Ok((k, Value::from_json(v)?)))
                    .collect::<Result<_>>()?;
                Ok(Value::Object(map))
            }
        }
    }

    /// Converts to a `serde_json::Value`.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Unit => serde_json::Value::Null,
            Value::Boolean(b) => (*b).into(),
            Value::Integer(i) => (*i).into(),
            Value::Number(n) => (*n).into(),
            Value::String(s) => s.as_str().into(),
            #[cfg(feature = "image")]
            Value::Image(img) => img.get_base64().into(),
            Value::Array(a) => {
                let arr: Vec<serde_json::Value> = a.iter().map(|v| v.to_json()).collect();
                serde_json::Value::Array(arr)
            }
            Value::Object(o) => {
                let mut map = serde_json::Map::new();
                let mut entries: Vec<_> = o.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));

                for (k, v) in entries {
                    map.insert(k.clone(), v.to_json());
                }
                serde_json::Value::Object(map)
            }
            Value::Tensor(t) => {
                let arr: Vec<serde_json::Value> = t
                    .iter()
                    .map(|&v| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(v as f64)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        )
                    })
                    .collect();
                serde_json::Value::Array(arr)
            }
            #[cfg(feature = "llm")]
            Value::Message(m) => serde_json::to_value(&**m).unwrap_or(serde_json::Value::Null),
            Value::Error(_) => serde_json::Value::Null, // Errors are not serializable
        }
    }

    /// Create Value from Serialize
    pub fn from_serialize<T: Serialize>(value: &T) -> Result<Self> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| Error::InvalidValue(format!("Failed to serialize: {}", e)))?;
        Self::from_json(json_value)
    }

    /// Convert Value to a Deserialize
    pub fn to_deserialize<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        let json_value = self.to_json();
        serde_json::from_value(json_value)
            .map_err(|e| Error::InvalidValue(format!("Failed to deserialize: {}", e)))
    }

    // Type check helpers

    /// Returns `true` if this is a `Unit` value.
    pub fn is_unit(&self) -> bool {
        matches!(self, Value::Unit)
    }

    /// Returns `true` if this is a `Boolean` value.
    pub fn is_boolean(&self) -> bool {
        matches!(self, Value::Boolean(_))
    }

    /// Returns `true` if this is an `Integer` value.
    pub fn is_integer(&self) -> bool {
        matches!(self, Value::Integer(_))
    }

    /// Returns `true` if this is a `Number` value.
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    /// Returns `true` if this is a `String` value.
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Returns `true` if this is an `Image` value.
    #[cfg(feature = "image")]
    pub fn is_image(&self) -> bool {
        matches!(self, Value::Image(_))
    }

    /// Returns `true` if this is an `Array` value.
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// Returns `true` if this is an `Object` value.
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// Returns `true` if this is a `Tensor` value.
    pub fn is_tensor(&self) -> bool {
        matches!(self, Value::Tensor(_))
    }

    /// Returns `true` if this is a `Message` value.
    #[cfg(feature = "llm")]
    pub fn is_message(&self) -> bool {
        matches!(self, Value::Message(_))
    }

    // Cast helpers

    /// Returns the inner boolean value if this is a `Boolean`, otherwise `None`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the value as `i64` if this is an `Integer` or `Number`, otherwise `None`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    /// Returns the value as `f64` if this is an `Integer` or `Number`, otherwise `None`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Integer(i) => Some(*i as f64),
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns a reference to the inner string if this is a `String`, otherwise `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns a reference to the inner image if this is an `Image`, otherwise `None`.
    #[cfg(feature = "image")]
    pub fn as_image(&self) -> Option<&PhotonImage> {
        match self {
            Value::Image(img) => Some(img),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner image if this is an `Image`, otherwise `None`.
    #[cfg(feature = "image")]
    pub fn as_image_mut(&mut self) -> Option<&mut PhotonImage> {
        match self {
            Value::Image(img) => Some(Arc::make_mut(img)),
            _ => None,
        }
    }

    /// Extracts the inner `Arc<PhotonImage>` if this is an `Image`, consuming self.
    #[cfg(feature = "image")]
    pub fn into_image(self) -> Option<Arc<PhotonImage>> {
        match self {
            Value::Image(img) => Some(img),
            _ => None,
        }
    }

    /// Returns a reference to the inner message if this is a `Message`, otherwise `None`.
    #[cfg(feature = "llm")]
    pub fn as_message(&self) -> Option<&Message> {
        match self {
            Value::Message(m) => Some(m),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner message if this is a `Message`, otherwise `None`.
    #[cfg(feature = "llm")]
    pub fn as_message_mut(&mut self) -> Option<&mut Message> {
        match self {
            Value::Message(m) => Some(Arc::make_mut(m)),
            _ => None,
        }
    }

    /// Extracts the inner `Arc<Message>` if this is a `Message`, consuming self.
    #[cfg(feature = "llm")]
    pub fn into_message(self) -> Option<Arc<Message>> {
        match self {
            Value::Message(m) => Some(m),
            _ => None,
        }
    }

    /// Converts to a boolean with type coercion.
    ///
    /// Conversion rules:
    /// - `Boolean`: returns the value
    /// - `Integer`: `0` → `false`, otherwise `true`
    /// - `Number`: `0.0` → `false`, otherwise `true`
    /// - `String`: parses "true"/"false"
    pub fn to_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            Value::Integer(i) => Some(*i != 0),
            Value::Number(n) => Some(*n != 0.0),
            Value::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Converts to `Value::Boolean` or `Value::Array` of booleans.
    pub fn to_boolean_value(&self) -> Option<Value> {
        match self {
            Value::Boolean(_) => Some(self.clone()),
            Value::Array(arr) => {
                if arr.iter().all(|v| v.is_boolean()) {
                    return Some(self.clone());
                }
                let mut new_arr = Vector::new();
                for item in arr {
                    new_arr.push_back(item.to_boolean_value()?);
                }
                Some(Value::Array(new_arr))
            }
            _ => self.to_boolean().map(Value::boolean),
        }
    }

    /// Converts to an integer (i64) with type coercion.
    pub fn to_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Boolean(b) => Some(if *b { 1 } else { 0 }),
            Value::Number(n) => Some(*n as i64),
            Value::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Converts to `Value::Integer` or `Value::Array` of integers.
    pub fn to_integer_value(&self) -> Option<Value> {
        match self {
            Value::Integer(_) => Some(self.clone()),
            Value::Array(arr) => {
                if arr.iter().all(|v| v.is_integer()) {
                    return Some(self.clone());
                }
                let mut new_arr = Vector::new();
                for item in arr {
                    new_arr.push_back(item.to_integer_value()?);
                }
                Some(Value::Array(new_arr))
            }
            _ => self.to_integer().map(Value::integer),
        }
    }

    /// Converts to a number (f64) with type coercion.
    pub fn to_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Integer(i) => Some(*i as f64),
            Value::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Converts to `Value::Number` or `Value::Array` of numbers.
    pub fn to_number_value(&self) -> Option<Value> {
        match self {
            Value::Number(_) => Some(self.clone()),
            Value::Array(arr) => {
                if arr.iter().all(|v| v.is_number()) {
                    return Some(self.clone());
                }
                let mut new_arr = Vector::new();
                for item in arr {
                    new_arr.push_back(item.to_number_value()?);
                }
                Some(Value::Array(new_arr))
            }
            _ => self.to_number().map(Value::number),
        }
    }

    /// Converts to a string with type coercion.
    pub fn to_string(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.as_ref().clone()),
            Value::Boolean(b) => Some(b.to_string()),
            Value::Integer(i) => Some(i.to_string()),
            Value::Number(n) => Some(n.to_string()),
            #[cfg(feature = "llm")]
            Value::Message(m) => Some(m.text()),
            _ => None,
        }
    }

    /// Converts to `Value::String` or `Value::Array` of strings.
    pub fn to_string_value(&self) -> Option<Value> {
        match self {
            Value::String(_) => Some(self.clone()),
            Value::Array(arr) => {
                if arr.iter().all(|v| v.is_string()) {
                    return Some(self.clone());
                }
                let mut new_arr = Vector::new();
                for item in arr {
                    new_arr.push_back(item.to_string_value()?);
                }
                Some(Value::Array(new_arr))
            }
            _ => self.to_string().map(Value::string),
        }
    }

    /// Converts to a `Message`.
    #[cfg(feature = "llm")]
    pub fn to_message(&self) -> Option<Message> {
        Message::try_from(self.clone()).ok()
    }

    /// Converts to `Value::Message` or `Value::Array` of messages.
    ///
    /// If the value is an array, it recursively converts its elements.
    #[cfg(feature = "llm")]
    pub fn to_message_value(&self) -> Option<Value> {
        match self {
            Value::Message(_) => Some(self.clone()),
            Value::Array(arr) => {
                if arr.iter().all(|v| v.is_message()) {
                    return Some(self.clone());
                }
                let mut new_arr = Vector::new();
                for item in arr {
                    new_arr.push_back(item.to_message_value()?);
                }
                Some(Value::Array(new_arr))
            }
            _ => Message::try_from(self.clone()).ok().map(Value::message),
        }
    }

    /// Returns a reference to the inner object map if this is an `Object`, otherwise `None`.
    pub fn as_object(&self) -> Option<&ValueMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner object map if this is an `Object`, otherwise `None`.
    pub fn as_object_mut(&mut self) -> Option<&mut ValueMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Extracts the inner `HashMap` if this is an `Object`, consuming self.
    pub fn into_object(self) -> Option<ValueMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Returns a reference to the inner array if this is an `Array`, otherwise `None`.
    pub fn as_array(&self) -> Option<&Vector<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner array if this is an `Array`, otherwise `None`.
    pub fn as_array_mut(&mut self) -> Option<&mut Vector<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Extracts the inner `Vector` if this is an `Array`, consuming self.
    pub fn into_array(self) -> Option<Vector<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Returns a reference to the inner tensor if this is a `Tensor`, otherwise `None`.
    pub fn as_tensor(&self) -> Option<&Vec<f32>> {
        match self {
            Value::Tensor(t) => Some(t),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner tensor if this is a `Tensor`, otherwise `None`.
    pub fn as_tensor_mut(&mut self) -> Option<&mut Vec<f32>> {
        match self {
            Value::Tensor(t) => Some(Arc::make_mut(t)),
            _ => None,
        }
    }

    /// Extracts the inner `Arc<Vec<f32>>` if this is a `Tensor`, consuming self.
    pub fn into_tensor(self) -> Option<Arc<Vec<f32>>> {
        match self {
            Value::Tensor(t) => Some(t),
            _ => None,
        }
    }

    /// Extracts the inner `Vec<f32>` if this is a `Tensor`, consuming self.
    /// Possibly O(n) copy.
    pub fn into_tensor_vec(self) -> Option<Vec<f32>> {
        match self {
            Value::Tensor(t) => Some(Arc::unwrap_or_clone(t)),
            _ => None,
        }
    }

    // Getters by key
    // These methods only work on `Object` values.

    /// Gets a value by key from an `Object`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|o| o.get(key))
    }

    /// Looks up a named property, seeing through variants that expose
    /// properties without being an `Object` (currently `Message`, whose
    /// fields resolve as they serialize, `Image`, which exposes `width`
    /// and `height`, and `Array`, which resolves an index key like `"0"`
    /// — see [`parse_index`] — to its element).
    /// Returns an owned value because non-`Object` variants materialize
    /// their properties on demand; `Object` and `Array` lookups are cheap
    /// `Arc`/`im` clones.
    pub fn get_prop(&self, key: &str) -> Option<Value> {
        match self {
            Value::Object(o) => o.get(key).cloned(),
            Value::Array(a) => a.get(parse_index(key)?).cloned(),
            #[cfg(feature = "llm")]
            Value::Message(m) => m.get_prop(key),
            #[cfg(feature = "image")]
            Value::Image(i) => match key {
                "width" => Some(Value::integer(i.get_width() as i64)),
                "height" => Some(Value::integer(i.get_height() as i64)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Sets a named property, seeing through variants that expose
    /// properties without being an `Object`: an `Object` inserts the key,
    /// an `Array` replaces the element at an in-range index key (see
    /// [`parse_index`]). A `Message` and an `Image` are read-only through
    /// this surface — their properties resolve via
    /// [`Value::get_prop`], but writing one is an error. Other
    /// variants — and a non-index or out-of-range key on an `Array` — are
    /// an error rather than a silent drop.
    pub fn set_prop(&mut self, key: &str, value: Value) -> Result<()> {
        match self {
            Value::Object(o) => {
                o.insert(key.to_string(), value);
                Ok(())
            }
            Value::Array(a) => match parse_index(key) {
                Some(i) if i < a.len() => {
                    a.set(i, value);
                    Ok(())
                }
                Some(i) => Err(Error::InvalidValue(format!(
                    "Index {i} is out of range for array of length {}",
                    a.len()
                ))),
                None => Err(Error::InvalidValue(format!(
                    "Cannot set property `{key}` on an array; expected an index"
                ))),
            },
            #[cfg(feature = "llm")]
            Value::Message(_) => Err(Error::InvalidValue(format!(
                "Cannot set property `{key}` on a Message"
            ))),
            #[cfg(feature = "image")]
            Value::Image(_) => Err(Error::InvalidValue(format!(
                "Cannot set property `{key}` on an Image"
            ))),
            _ => Err(Error::InvalidValue(format!(
                "Cannot set property `{key}` on this value"
            ))),
        }
    }

    /// True when the value exposes properties by key (`Object`, `Array`,
    /// `Message`, `Image`).
    pub fn has_props(&self) -> bool {
        match self {
            Value::Object(_) => true,
            Value::Array(_) => true,
            #[cfg(feature = "llm")]
            Value::Message(_) => true,
            #[cfg(feature = "image")]
            Value::Image(_) => true,
            _ => false,
        }
    }

    /// Gets a mutable reference to a value by key from an `Object`.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.as_object_mut().and_then(|o| o.get_mut(key))
    }

    /// Gets a boolean value by key from an `Object`.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Gets an i64 value by key from an `Object`.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    /// Gets an f64 value by key from an `Object`.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    /// Gets a string reference by key from an `Object`.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    /// Gets an image reference by key from an `Object`.
    #[cfg(feature = "image")]
    pub fn get_image(&self, key: &str) -> Option<&PhotonImage> {
        self.get(key).and_then(|v| v.as_image())
    }

    /// Gets a mutable image reference by key from an `Object`.
    #[cfg(feature = "image")]
    pub fn get_image_mut(&mut self, key: &str) -> Option<&mut PhotonImage> {
        self.get_mut(key).and_then(|v| v.as_image_mut())
    }

    /// Gets an object reference by key from an `Object`.
    pub fn get_object(&self, key: &str) -> Option<&ValueMap<String, Value>> {
        self.get(key).and_then(|v| v.as_object())
    }

    /// Gets a mutable object reference by key from an `Object`.
    pub fn get_object_mut(&mut self, key: &str) -> Option<&mut ValueMap<String, Value>> {
        self.get_mut(key).and_then(|v| v.as_object_mut())
    }

    /// Gets an array reference by key from an `Object`.
    pub fn get_array(&self, key: &str) -> Option<&Vector<Value>> {
        self.get(key).and_then(|v| v.as_array())
    }

    /// Gets a mutable array reference by key from an `Object`.
    pub fn get_array_mut(&mut self, key: &str) -> Option<&mut Vector<Value>> {
        self.get_mut(key).and_then(|v| v.as_array_mut())
    }

    /// Gets a tensor reference by key from an `Object`.
    pub fn get_tensor(&self, key: &str) -> Option<&Vec<f32>> {
        self.get(key).and_then(|v| v.as_tensor())
    }

    /// Gets a mutable tensor reference by key from an `Object`.
    pub fn get_tensor_mut(&mut self, key: &str) -> Option<&mut Vec<f32>> {
        self.get_mut(key).and_then(|v| v.as_tensor_mut())
    }

    /// Gets a message reference by key from an `Object`.
    #[cfg(feature = "llm")]
    pub fn get_message(&self, key: &str) -> Option<&Message> {
        self.get(key).and_then(|v| v.as_message())
    }

    /// Gets a mutable message reference by key from an `Object`.
    #[cfg(feature = "llm")]
    pub fn get_message_mut(&mut self, key: &str) -> Option<&mut Message> {
        self.get_mut(key).and_then(|v| v.as_message_mut())
    }

    // Setter by key

    /// Sets a value by key in an `Object`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidValue` if this is not an `Object`.
    pub fn set(&mut self, key: String, value: Value) -> Result<()> {
        if let Some(obj) = self.as_object_mut() {
            obj.insert(key, value);
            Ok(())
        } else {
            Err(Error::InvalidValue(
                "set can only be called on Object Value".into(),
            ))
        }
    }
}

/// Parses a key-path segment as an array index: ASCII digits only, no
/// sign, no leading zeros (except `"0"` itself). This single definition
/// decides what counts as an index everywhere — property access here and
/// the root-array handling in Get Value must agree, or a key could skip
/// one path and fail the other.
pub fn parse_index(key: &str) -> Option<usize> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    key.parse().ok()
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Unit, Value::Unit) => true,
            (Value::Boolean(b1), Value::Boolean(b2)) => b1 == b2,
            (Value::Integer(i1), Value::Integer(i2)) => i1 == i2,
            (Value::Number(n1), Value::Number(n2)) => n1 == n2,
            (Value::String(s1), Value::String(s2)) => s1 == s2,
            #[cfg(feature = "image")]
            (Value::Image(i1), Value::Image(i2)) => {
                i1.get_width() == i2.get_width()
                    && i1.get_height() == i2.get_height()
                    && i1.get_raw_pixels() == i2.get_raw_pixels()
            }
            (Value::Array(a1), Value::Array(a2)) => a1 == a2,
            (Value::Object(o1), Value::Object(o2)) => o1 == o2,
            (Value::Tensor(t1), Value::Tensor(t2)) => t1 == t2,
            #[cfg(feature = "llm")]
            (Value::Message(m1), Value::Message(m2)) => m1 == m2,
            _ => false,
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Unit => serializer.serialize_none(),
            Value::Boolean(b) => serializer.serialize_bool(*b),
            Value::Integer(i) => serializer.serialize_i64(*i),
            Value::Number(n) => serializer.serialize_f64(*n),
            Value::String(s) => serializer.serialize_str(s),
            #[cfg(feature = "image")]
            Value::Image(img) => serializer.serialize_str(&img.get_base64()),
            Value::Array(a) => {
                let mut seq = serializer.serialize_seq(Some(a.len()))?;
                for e in a.iter() {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
            Value::Object(o) => {
                let mut map = serializer.serialize_map(Some(o.len()))?;
                // Sort the entries to ensure stable JSON output.
                let mut entries: Vec<_> = o.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));

                for (k, v) in entries {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Value::Tensor(t) => {
                let mut seq = serializer.serialize_seq(Some(t.len()))?;
                for e in t.iter() {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
            #[cfg(feature = "llm")]
            Value::Message(m) => m.serialize(serializer),
            Value::Error(_) => serializer.serialize_none(), // Errors are not serializable
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        Value::from_json(value)
            .map_err(|e| serde::de::Error::custom(format!("Failed to deserialize Value: {}", e)))
    }
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Value::unit()
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::boolean(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::integer(value as i64)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::integer(value)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::Integer(value as i64)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::Integer(value as i64)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::Number(value as f64)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::number(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::string(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::string(value)
    }
}

impl From<Vector<Value>> for Value {
    fn from(value: Vector<Value>) -> Self {
        Value::Array(value)
    }
}

impl From<HashMap<String, Value>> for Value {
    fn from(value: HashMap<String, Value>) -> Self {
        Value::Object(value)
    }
}

// Tensor support
impl From<Vec<f32>> for Value {
    fn from(value: Vec<f32>) -> Self {
        Value::Tensor(Arc::new(value))
    }
}
impl From<Arc<Vec<f32>>> for Value {
    fn from(value: Arc<Vec<f32>>) -> Self {
        Value::Tensor(value)
    }
}

// Standard Collections support
impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::Array(Vector::from(value))
    }
}
impl From<std::collections::HashMap<String, Value>> for Value {
    fn from(value: std::collections::HashMap<String, Value>) -> Self {
        Value::Object(HashMap::from(value))
    }
}

// Error support
impl From<Error> for Value {
    fn from(value: Error) -> Self {
        Value::Error(Arc::new(value))
    }
}

// Option support
impl From<Option<Value>> for Value {
    fn from(value: Option<Value>) -> Self {
        value.unwrap_or(Value::Unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use im::{hashmap, vector};
    use serde_json::json;

    #[test]
    fn test_partial_eq() {
        // Test PartialEq implementation
        let unit1 = Value::unit();
        let unit2 = Value::unit();
        assert_eq!(unit1, unit2);

        let boolean1 = Value::boolean(true);
        let boolean2 = Value::boolean(true);
        assert_eq!(boolean1, boolean2);

        let integer1 = Value::integer(42);
        let integer2 = Value::integer(42);
        assert_eq!(integer1, integer2);
        let different = Value::integer(100);
        assert_ne!(integer1, different);

        let number1 = Value::number(2.5);
        let number2 = Value::number(2.5);
        assert_eq!(number1, number2);

        let string1 = Value::string("hello");
        let string2 = Value::string("hello");
        assert_eq!(string1, string2);

        #[cfg(feature = "image")]
        {
            let image1 = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            let image2 = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(image1, image2);
        }

        let obj1 = Value::object(hashmap! {
                "key1".into() => Value::string("value1"),
                "key2".into() => Value::integer(2),
        });
        let obj2 = Value::object(hashmap! {
                "key1".to_string() => Value::string("value1"),
                "key2".to_string() => Value::integer(2),
        });
        assert_eq!(obj1, obj2);

        let arr1 = Value::array(vector![
            Value::integer(1),
            Value::string("two"),
            Value::boolean(true),
        ]);
        let arr2 = Value::array(vector![
            Value::integer(1),
            Value::string("two"),
            Value::boolean(true),
        ]);
        assert_eq!(arr1, arr2);

        let mixed_types_1 = Value::boolean(true);
        let mixed_types_2 = Value::integer(1);
        assert_ne!(mixed_types_1, mixed_types_2);

        #[cfg(feature = "llm")]
        {
            let msg1 = Value::message(Message::user("hello".to_string()));
            let msg2 = Value::message(Message::user("hello".to_string()));
            assert_eq!(msg1, msg2);
        }
    }

    #[test]
    fn test_value_constructors() {
        // Test Value constructors
        let unit = Value::unit();
        assert_eq!(unit, Value::Unit);

        let boolean = Value::boolean(true);
        assert_eq!(boolean, Value::Boolean(true));

        let integer = Value::integer(42);
        assert_eq!(integer, Value::Integer(42));

        let number = Value::number(2.5);
        assert!(matches!(number, Value::Number(_)));
        if let Value::Number(num) = number {
            assert!((num - 2.5).abs() < f64::EPSILON);
        }

        let string = Value::string("hello");
        assert!(matches!(string, Value::String(_)));
        assert_eq!(string.as_str().unwrap(), "hello");

        let text = Value::string("multiline\ntext");
        assert!(matches!(text, Value::String(_)));
        assert_eq!(text.as_str().unwrap(), "multiline\ntext");

        let array = Value::array(vector![Value::integer(1), Value::integer(2)]);
        assert!(matches!(array, Value::Array(_)));
        if let Value::Array(arr) = array {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0].as_i64().unwrap(), 1);
            assert_eq!(arr[1].as_i64().unwrap(), 2);
        }

        let obj = Value::object(hashmap! {
                "key1".to_string() => Value::string("string1"),
                "key2".to_string() => Value::integer(2),
        });
        assert!(matches!(obj, Value::Object(_)));
        if let Value::Object(obj) = obj {
            assert_eq!(obj.get("key1").and_then(|v| v.as_str()), Some("string1"));
            assert_eq!(obj.get("key2").and_then(|v| v.as_i64()), Some(2));
        } else {
            panic!("Object was not deserialized correctly");
        }

        #[cfg(feature = "llm")]
        {
            let msg = Value::message(Message::user("hello".to_string()));
            assert!(matches!(msg, Value::Message(_)));
        }
    }

    #[test]
    fn test_value_from_json_value() {
        // Test converting from JSON value to Value
        let null = Value::from_json(json!(null)).unwrap();
        assert_eq!(null, Value::Unit);

        let boolean = Value::from_json(json!(true)).unwrap();
        assert_eq!(boolean, Value::Boolean(true));

        let integer = Value::from_json(json!(42)).unwrap();
        assert_eq!(integer, Value::Integer(42));

        let number = Value::from_json(json!(2.5)).unwrap();
        assert!(matches!(number, Value::Number(_)));
        if let Value::Number(num) = number {
            assert!((num - 2.5).abs() < f64::EPSILON);
        }

        let string = Value::from_json(json!("hello")).unwrap();
        assert!(matches!(string, Value::String(_)));
        if let Value::String(s) = string {
            assert_eq!(*s, "hello");
        } else {
            panic!("Expected string value");
        }

        let array = Value::from_json(json!([1, "test", true])).unwrap();
        assert!(matches!(array, Value::Array(_)));
        if let Value::Array(arr) = array {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::Integer(1));
            assert!(matches!(&arr[1], Value::String(_)));
            if let Value::String(s) = &arr[1] {
                assert_eq!(**s, "test");
            } else {
                panic!("Expected string value");
            }
            assert_eq!(arr[2], Value::Boolean(true));
        }

        let object = Value::from_json(json!({"key1": "string1", "key2": 2})).unwrap();
        assert!(matches!(object, Value::Object(_)));
        if let Value::Object(obj) = object {
            assert_eq!(obj.get("key1").and_then(|v| v.as_str()), Some("string1"));
            assert_eq!(obj.get("key2").and_then(|v| v.as_i64()), Some(2));
        } else {
            panic!("Object was not deserialized correctly");
        }
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_image_from_base64_errors_and_padding() {
        // 1x1 transparent PNG, standard padded encoding
        const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==";

        // A string claiming the PNG data URI prefix with a corrupt payload is
        // an error, not a panic — through from_json and the serde surface.
        let bad = format!("{IMAGE_BASE64_PREFIX}@@not-base64@@");
        assert!(Value::from_json(serde_json::json!(bad)).is_err());
        assert!(serde_json::from_str::<Value>(&format!("\"{bad}\"")).is_err());

        // Valid data still sniffs into an Image
        let good = format!("{IMAGE_BASE64_PREFIX}{PNG_BASE64}");
        assert!(
            Value::from_json(serde_json::json!(good))
                .unwrap()
                .is_image()
        );

        // Unpadded input keeps decoding (the old base64 0.13 path allowed it)
        let unpadded = PNG_BASE64.trim_end_matches('=');
        assert!(image_from_base64(unpadded).is_ok());
    }

    #[test]
    fn test_value_test_methods() {
        // Test test methods on Value
        let unit = Value::unit();
        assert!(unit.is_unit());
        assert!(!unit.is_boolean());
        assert!(!unit.is_integer());
        assert!(!unit.is_number());
        assert!(!unit.is_string());
        assert!(!unit.is_array());
        assert!(!unit.is_object());
        #[cfg(feature = "image")]
        assert!(!unit.is_image());

        let boolean = Value::boolean(true);
        assert!(!boolean.is_unit());
        assert!(boolean.is_boolean());
        assert!(!boolean.is_integer());
        assert!(!boolean.is_number());
        assert!(!boolean.is_string());
        assert!(!boolean.is_array());
        assert!(!boolean.is_object());
        #[cfg(feature = "image")]
        assert!(!boolean.is_image());

        let integer = Value::integer(42);
        assert!(!integer.is_unit());
        assert!(!integer.is_boolean());
        assert!(integer.is_integer());
        assert!(!integer.is_number());
        assert!(!integer.is_string());
        assert!(!integer.is_array());
        assert!(!integer.is_object());
        #[cfg(feature = "image")]
        assert!(!integer.is_image());

        let number = Value::number(2.5);
        assert!(!number.is_unit());
        assert!(!number.is_boolean());
        assert!(!number.is_integer());
        assert!(number.is_number());
        assert!(!number.is_string());
        assert!(!number.is_array());
        assert!(!number.is_object());
        #[cfg(feature = "image")]
        assert!(!number.is_image());

        let string = Value::string("hello");
        assert!(!string.is_unit());
        assert!(!string.is_boolean());
        assert!(!string.is_integer());
        assert!(!string.is_number());
        assert!(string.is_string());
        assert!(!string.is_array());
        assert!(!string.is_object());
        #[cfg(feature = "image")]
        assert!(!string.is_image());

        let array = Value::array(vector![Value::integer(1), Value::integer(2)]);
        assert!(!array.is_unit());
        assert!(!array.is_boolean());
        assert!(!array.is_integer());
        assert!(!array.is_number());
        assert!(!array.is_string());
        assert!(array.is_array());
        assert!(!array.is_object());
        #[cfg(feature = "image")]
        assert!(!array.is_image());

        let obj = Value::object(hashmap! {
                "key1".to_string() => Value::string("string1"),
                "key2".to_string() => Value::integer(2),
        });
        assert!(!obj.is_unit());
        assert!(!obj.is_boolean());
        assert!(!obj.is_integer());
        assert!(!obj.is_number());
        assert!(!obj.is_string());
        assert!(!obj.is_array());
        assert!(obj.is_object());
        #[cfg(feature = "image")]
        assert!(!obj.is_image());

        #[cfg(feature = "image")]
        {
            let img = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert!(!img.is_unit());
            assert!(!img.is_boolean());
            assert!(!img.is_integer());
            assert!(!img.is_number());
            assert!(!img.is_string());
            assert!(!img.is_array());
            assert!(!img.is_object());
            assert!(img.is_image());
        }

        #[cfg(feature = "llm")]
        {
            let msg = Value::message(Message::user("hello".to_string()));
            assert!(!msg.is_unit());
            assert!(!msg.is_boolean());
            assert!(!msg.is_integer());
            assert!(!msg.is_number());
            assert!(!msg.is_string());
            assert!(!msg.is_array());
            assert!(!msg.is_object());
            #[cfg(feature = "image")]
            assert!(!msg.is_image());
            assert!(msg.is_message());
        }
    }

    #[test]
    fn test_value_as_methods() {
        // Test accessor methods on Value
        let boolean = Value::boolean(true);
        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(boolean.as_i64(), None);
        assert_eq!(boolean.as_f64(), None);
        assert_eq!(boolean.as_str(), None);
        assert!(boolean.as_array().is_none());
        assert_eq!(boolean.as_object(), None);
        #[cfg(feature = "image")]
        assert!(boolean.as_image().is_none());

        let integer = Value::integer(42);
        assert_eq!(integer.as_bool(), None);
        assert_eq!(integer.as_i64(), Some(42));
        assert_eq!(integer.as_f64(), Some(42.0));
        assert_eq!(integer.as_str(), None);
        assert!(integer.as_array().is_none());
        assert_eq!(integer.as_object(), None);
        #[cfg(feature = "image")]
        assert!(integer.as_image().is_none());

        let number = Value::number(2.5);
        assert_eq!(number.as_bool(), None);
        assert_eq!(number.as_i64(), Some(2)); // truncated
        assert_eq!(number.as_f64().unwrap(), 2.5);
        assert_eq!(number.as_str(), None);
        assert!(number.as_array().is_none());
        assert_eq!(number.as_object(), None);
        #[cfg(feature = "image")]
        assert!(number.as_image().is_none());

        let string = Value::string("hello");
        assert_eq!(string.as_bool(), None);
        assert_eq!(string.as_i64(), None);
        assert_eq!(string.as_f64(), None);
        assert_eq!(string.as_str(), Some("hello"));
        assert!(string.as_array().is_none());
        assert_eq!(string.as_object(), None);
        #[cfg(feature = "image")]
        assert!(string.as_image().is_none());

        let array = Value::array(vector![Value::integer(1), Value::integer(2)]);
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

        let mut array = Value::array(vector![Value::integer(1), Value::integer(2)]);
        if let Some(arr) = array.as_array_mut() {
            arr.push_back(Value::integer(3));
        }

        let obj = Value::object(hashmap! {
                "key1".to_string() => Value::string("string1"),
                "key2".to_string() => Value::integer(2),
        });
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

        let mut obj = Value::object(hashmap! {
                "key1".to_string() => Value::string("string1"),
                "key2".to_string() => Value::integer(2),
        });
        if let Some(value) = obj.as_object_mut() {
            value.insert("key3".to_string(), Value::boolean(true));
        }

        #[cfg(feature = "image")]
        {
            let img = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(img.as_bool(), None);
            assert_eq!(img.as_i64(), None);
            assert_eq!(img.as_f64(), None);
            assert_eq!(img.as_str(), None);
            assert!(img.as_array().is_none());
            assert_eq!(img.as_object(), None);
            assert!(img.as_image().is_some());
        }

        #[cfg(feature = "llm")]
        {
            let mut msg = Value::message(Message::user("hello".to_string()));
            assert!(msg.as_message().is_some());
            assert_eq!(msg.as_message().unwrap().text(), "hello");
            assert!(msg.as_message_mut().is_some());
            if let Some(m) = msg.as_message_mut() {
                m.content = "world".into();
            }
            assert_eq!(msg.as_message().unwrap().text(), "world");
            assert!(msg.into_message().is_some());
        }
    }

    #[test]
    fn test_value_get_methods() {
        // Test get methods on Value
        const KEY: &str = "key";

        let boolean = Value::boolean(true);
        assert_eq!(boolean.get(KEY), None);

        let integer = Value::integer(42);
        assert_eq!(integer.get(KEY), None);

        let number = Value::number(2.5);
        assert_eq!(number.get(KEY), None);

        let string = Value::string("hello");
        assert_eq!(string.get(KEY), None);

        let array = Value::array(vector![Value::integer(1), Value::integer(2)]);
        assert_eq!(array.get(KEY), None);

        let mut array = Value::array(vector![Value::integer(1), Value::integer(2)]);
        assert_eq!(array.get_mut(KEY), None);

        let mut obj = Value::object(hashmap! {
                "k_boolean".to_string() => Value::boolean(true),
                "k_integer".to_string() => Value::integer(42),
                "k_number".to_string() => Value::number(2.5),
                "k_string".to_string() => Value::string("string1"),
                "k_array".to_string() => Value::array(vector![Value::integer(1)]),
                "k_object".to_string() => Value::object(hashmap! {
                        "inner_key".to_string() => Value::integer(100),
                }),
        });
        #[cfg(feature = "image")]
        obj.set(
            "k_image".to_string(),
            Value::image(PhotonImage::new(vec![0u8; 4], 1, 1)),
        )
        .unwrap();
        #[cfg(feature = "llm")]
        obj.set(
            "k_message".to_string(),
            Value::message(Message::user("hello".to_string())),
        )
        .unwrap();
        assert_eq!(obj.get(KEY), None);
        assert_eq!(obj.get_bool("k_boolean"), Some(true));
        assert_eq!(obj.get_i64("k_integer"), Some(42));
        assert_eq!(obj.get_f64("k_number"), Some(2.5));
        assert_eq!(obj.get_str("k_string"), Some("string1"));
        assert!(obj.get_array("k_array").is_some());
        assert!(obj.get_array_mut("k_array").is_some());
        assert!(obj.get_object("k_object").is_some());
        assert!(obj.get_object_mut("k_object").is_some());
        #[cfg(feature = "image")]
        assert!(obj.get_image("k_image").is_some());
        #[cfg(feature = "llm")]
        assert!(obj.get_message("k_message").is_some());
        #[cfg(feature = "llm")]
        assert!(obj.get_message_mut("k_message").is_some());

        #[cfg(feature = "image")]
        {
            let img = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(img.get(KEY), None);
        }
    }

    #[test]
    fn test_value_get_prop() {
        // Object: same lookup as `get`, but owned.
        let obj = Value::object(hashmap! {
            "k".to_string() => Value::integer(1),
        });
        assert_eq!(obj.get_prop("k"), Some(Value::integer(1)));
        assert_eq!(obj.get_prop("missing"), None);

        // Message: fields resolve as they serialize.
        #[cfg(feature = "llm")]
        {
            let msg = Value::message(Message::user("hello".to_string()));
            assert_eq!(msg.get_prop("role"), Some(Value::string("user")));
            assert_eq!(msg.get_prop("content"), Some(Value::string("hello")));
            assert_eq!(msg.get_prop("missing"), None);
        }

        // Image: width and height resolve as integers.
        #[cfg(feature = "image")]
        {
            let img = Value::image(PhotonImage::new(vec![0u8; 8], 2, 1));
            assert_eq!(img.get_prop("width"), Some(Value::integer(2)));
            assert_eq!(img.get_prop("height"), Some(Value::integer(1)));
            assert_eq!(img.get_prop("missing"), None);
        }

        // Array: index keys resolve to elements.
        let arr = Value::array(vector![Value::integer(10), Value::integer(20)]);
        assert_eq!(arr.get_prop("0"), Some(Value::integer(10)));
        assert_eq!(arr.get_prop("1"), Some(Value::integer(20)));
        assert_eq!(arr.get_prop("2"), None);
        assert_eq!(arr.get_prop("name"), None);

        // Scalars have no properties.
        assert_eq!(Value::integer(42).get_prop("k"), None);
        assert_eq!(Value::string("s").get_prop("k"), None);
        assert_eq!(Value::unit().get_prop("k"), None);
    }

    #[test]
    fn test_parse_index() {
        assert_eq!(parse_index("0"), Some(0));
        assert_eq!(parse_index("12"), Some(12));

        // ASCII digits only, no sign, no leading zeros, no whitespace.
        assert_eq!(parse_index(""), None);
        assert_eq!(parse_index("+1"), None);
        assert_eq!(parse_index("-1"), None);
        assert_eq!(parse_index("00"), None);
        assert_eq!(parse_index("01"), None);
        assert_eq!(parse_index(" 0"), None);
        assert_eq!(parse_index("1a"), None);
        assert_eq!(parse_index("١"), None); // non-ASCII digit
    }

    #[test]
    fn test_value_set_prop() {
        // Object: plain insert
        let mut obj = Value::object_default();
        obj.set_prop("k", Value::integer(1)).unwrap();
        assert_eq!(obj.get_prop("k"), Some(Value::integer(1)));
        assert!(obj.has_props());

        // Message: exposes properties (so key-path writes don't clobber it
        // with an empty Object) but rejects writes, leaving it untouched
        #[cfg(feature = "llm")]
        {
            let mut msg = Value::message(Message::user("hello".to_string()));
            assert!(msg.has_props());
            assert!(msg.set_prop("content", Value::string("edited")).is_err());
            assert!(msg.is_message());
            assert_eq!(msg.get_prop("content"), Some(Value::string("hello")));
        }

        // Image: same read-only contract as Message
        #[cfg(feature = "image")]
        {
            let mut img = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert!(img.has_props());
            assert!(img.set_prop("width", Value::integer(2)).is_err());
            assert!(img.is_image());
            assert_eq!(img.get_prop("width"), Some(Value::integer(1)));
        }

        // Array: in-range index replaces the element; a shared clone is
        // untouched (im::Vector copy-on-write). Out-of-range or non-index
        // keys error with the array intact.
        let mut arr = Value::array(vector![Value::integer(10), Value::integer(20)]);
        let shared_arr = arr.clone();
        arr.set_prop("1", Value::integer(99)).unwrap();
        assert_eq!(arr.get_prop("1"), Some(Value::integer(99)));
        assert_eq!(shared_arr.get_prop("1"), Some(Value::integer(20)));
        assert!(arr.has_props());
        assert!(arr.set_prop("2", Value::integer(1)).is_err());
        assert!(arr.set_prop("name", Value::integer(1)).is_err());
        assert_eq!(arr.as_array().unwrap().len(), 2);

        // Scalars reject property writes
        let mut scalar = Value::integer(42);
        assert!(scalar.set_prop("k", Value::integer(1)).is_err());
        assert!(!scalar.has_props());
        assert!(!Value::unit().has_props());
    }

    #[test]
    fn test_value_set() {
        // Test set method on Value
        let mut obj = Value::object(ValueMap::new());
        assert!(obj.set("key1".to_string(), Value::integer(42)).is_ok());
        assert_eq!(obj.get_i64("key1"), Some(42));

        let mut not_obj = Value::integer(10);
        assert!(not_obj.set("key1".to_string(), Value::integer(42)).is_err());
    }

    #[test]
    fn test_value_default() {
        assert_eq!(Value::default(), Value::Unit);

        assert_eq!(Value::boolean_default(), Value::Boolean(false));
        assert_eq!(Value::integer_default(), Value::Integer(0));
        assert_eq!(Value::number_default(), Value::Number(0.0));
        assert_eq!(
            Value::string_default(),
            Value::String(Arc::new(String::new()))
        );
        assert_eq!(Value::array_default(), Value::Array(Vector::new()));
        assert_eq!(Value::object_default(), Value::Object(ValueMap::new()));

        #[cfg(feature = "image")]
        {
            assert_eq!(
                Value::image_default(),
                Value::image(PhotonImage::new(vec![0u8; 4], 1, 1))
            );
        }
    }

    #[test]
    fn test_to_json() {
        // Test to_json
        let unit = Value::unit();
        assert_eq!(unit.to_json(), json!(null));

        let boolean = Value::boolean(true);
        assert_eq!(boolean.to_json(), json!(true));

        let integer = Value::integer(42);
        assert_eq!(integer.to_json(), json!(42));

        let number = Value::number(2.5);
        assert_eq!(number.to_json(), json!(2.5));

        let string = Value::string("hello");
        assert_eq!(string.to_json(), json!("hello"));

        let array = Value::array(vector![Value::integer(1), Value::string("test")]);
        assert_eq!(array.to_json(), json!([1, "test"]));

        let obj = Value::object(hashmap! {
                "key1".to_string() => Value::string("string1"),
                "key2".to_string() => Value::integer(2),
        });
        assert_eq!(obj.to_json(), json!({"key1": "string1", "key2": 2}));

        #[cfg(feature = "image")]
        {
            let img = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(
                img.to_json(),
                json!(
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg=="
                )
            );
        }

        #[cfg(feature = "llm")]
        {
            let msg = Value::message(Message::user("hello".to_string()));
            assert_eq!(
                msg.to_json(),
                json!({
                    "role": "user",
                    "content": "hello",
                })
            );
        }
    }

    #[test]
    fn test_value_serialization() {
        // Test Null serialization
        {
            let null = Value::Unit;
            assert_eq!(serde_json::to_string(&null).unwrap(), "null");
        }

        // Test Boolean serialization
        {
            let boolean_t = Value::boolean(true);
            assert_eq!(serde_json::to_string(&boolean_t).unwrap(), "true");

            let boolean_f = Value::boolean(false);
            assert_eq!(serde_json::to_string(&boolean_f).unwrap(), "false");
        }

        // Test Integer serialization
        {
            let integer = Value::integer(42);
            assert_eq!(serde_json::to_string(&integer).unwrap(), "42");
        }

        // Test Number serialization
        {
            let num = Value::number(2.5);
            assert_eq!(serde_json::to_string(&num).unwrap(), "2.5");

            let num = Value::number(3.0);
            assert_eq!(serde_json::to_string(&num).unwrap(), "3.0");
        }

        // Test String serialization
        {
            let s = Value::string("Hello, world!");
            assert_eq!(serde_json::to_string(&s).unwrap(), "\"Hello, world!\"");

            let s = Value::string("hello\nworld\n\n");
            assert_eq!(serde_json::to_string(&s).unwrap(), r#""hello\nworld\n\n""#);
        }

        // Test Image serialization
        #[cfg(feature = "image")]
        {
            let img = Value::image(PhotonImage::new(vec![0u8; 4], 1, 1));
            assert_eq!(
                serde_json::to_string(&img).unwrap(),
                r#""data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==""#
            );
        }

        // Test Arc Image serialization
        #[cfg(feature = "image")]
        {
            let img = Value::image_arc(Arc::new(PhotonImage::new(vec![0u8; 4], 1, 1)));
            assert_eq!(
                serde_json::to_string(&img).unwrap(),
                r#""data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==""#
            );
        }

        // Test Array serialization
        {
            let array = Value::array(vector![
                Value::integer(1),
                Value::string("test"),
                Value::object(hashmap! {
                        "key1".to_string() => Value::string("test"),
                        "key2".to_string() => Value::integer(2),
                }),
            ]);
            assert_eq!(
                serde_json::to_string(&array).unwrap(),
                r#"[1,"test",{"key1":"test","key2":2}]"#
            );
        }

        // Test Object serialization
        {
            let obj = Value::object(hashmap! {
                    "key1".to_string() => Value::string("test"),
                    "key2".to_string() => Value::integer(3),
            });
            assert_eq!(
                serde_json::to_string(&obj).unwrap(),
                r#"{"key1":"test","key2":3}"#
            );
        }
    }

    #[test]
    fn test_value_deserialization() {
        // Test Null deserialization
        {
            let deserialized: Value = serde_json::from_str("null").unwrap();
            assert_eq!(deserialized, Value::Unit);
        }

        // Test Boolean deserialization
        {
            let deserialized: Value = serde_json::from_str("false").unwrap();
            assert_eq!(deserialized, Value::boolean(false));

            let deserialized: Value = serde_json::from_str("true").unwrap();
            assert_eq!(deserialized, Value::boolean(true));
        }

        // Test Integer deserialization
        {
            let deserialized: Value = serde_json::from_str("123").unwrap();
            assert_eq!(deserialized, Value::integer(123));
        }

        // Test Number deserialization
        {
            let deserialized: Value = serde_json::from_str("2.5").unwrap();
            assert_eq!(deserialized, Value::number(2.5));

            let deserialized: Value = serde_json::from_str("3.0").unwrap();
            assert_eq!(deserialized, Value::number(3.0));
        }

        // Test String deserialization
        {
            let deserialized: Value = serde_json::from_str("\"Hello, world!\"").unwrap();
            assert_eq!(deserialized, Value::string("Hello, world!"));

            let deserialized: Value = serde_json::from_str(r#""hello\nworld\n\n""#).unwrap();
            assert_eq!(deserialized, Value::string("hello\nworld\n\n"));
        }

        // Test Image deserialization
        #[cfg(feature = "image")]
        {
            let deserialized: Value = serde_json::from_str(
                r#""data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==""#,
            )
            .unwrap();
            assert!(matches!(deserialized, Value::Image(_)));
        }

        // Test Array deserialization
        {
            let deserialized: Value =
                serde_json::from_str(r#"[1,"test",{"key1":"test","key2":2}]"#).unwrap();
            assert!(matches!(deserialized, Value::Array(_)));
            if let Value::Array(arr) = deserialized {
                assert_eq!(arr.len(), 3, "Array length mismatch after serialization");
                assert_eq!(arr[0], Value::integer(1));
                assert_eq!(arr[1], Value::string("test"));
                assert_eq!(
                    arr[2],
                    Value::object(hashmap! {
                            "key1".to_string() => Value::string("test"),
                            "key2".to_string() => Value::integer(2),
                    })
                );
            }
        }

        // Test Object deserialization
        {
            let deserialized: Value = serde_json::from_str(r#"{"key1":"test","key2":3}"#).unwrap();
            assert_eq!(
                deserialized,
                Value::object(hashmap! {
                        "key1".to_string() => Value::string("test"),
                        "key2".to_string() => Value::integer(3),
                })
            );
        }
    }

    #[test]
    fn test_value_into() {
        // Test From implementations for Value
        let from_unit: Value = ().into();
        assert_eq!(from_unit, Value::Unit);

        let from_bool: Value = true.into();
        assert_eq!(from_bool, Value::Boolean(true));

        let from_i32: Value = 42i32.into();
        assert_eq!(from_i32, Value::Integer(42));

        let from_i64: Value = 100i64.into();
        assert_eq!(from_i64, Value::Integer(100));

        let from_f64: Value = 2.5f64.into();
        assert_eq!(from_f64, Value::Number(2.5));

        let from_string: Value = "hello".to_string().into();
        assert_eq!(from_string, Value::String(Arc::new("hello".to_string())));

        let from_str: Value = "world".into();
        assert_eq!(from_str, Value::String(Arc::new("world".to_string())));
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

        // Test ModuleData roundtrip
        let module_data = Value::from_serialize(&test_data).unwrap();
        assert_eq!(module_data.get_str("name"), Some("Alice"));
        assert_eq!(module_data.get_i64("age"), Some(30));
        assert_eq!(module_data.get_bool("active"), Some(true));

        let restored: TestStruct = module_data.to_deserialize().unwrap();
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

        // Test ModuleData roundtrip with nested structures
        let module_data = Value::from_serialize(&person).unwrap();
        assert_eq!(module_data.get_str("name"), Some("Bob"));

        let address = module_data.get_object("address").unwrap();
        assert_eq!(
            address.get("city").and_then(|v| v.as_str()),
            Some("Springfield")
        );

        let tags = module_data.get_array("tags").unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str(), Some("developer"));

        let restored: Person = module_data.to_deserialize().unwrap();
        assert_eq!(restored, person);
    }

    #[test]
    fn test_value_conversions() {
        // Boolean
        assert_eq!(Value::boolean(true).to_boolean(), Some(true));
        assert_eq!(Value::integer(1).to_boolean(), Some(true));
        assert_eq!(Value::integer(0).to_boolean(), Some(false));
        assert_eq!(Value::number(1.0).to_boolean(), Some(true));
        assert_eq!(Value::number(0.0).to_boolean(), Some(false));
        assert_eq!(Value::string("true").to_boolean(), Some(true));
        assert_eq!(Value::string("false").to_boolean(), Some(false));
        assert_eq!(Value::unit().to_boolean(), None);

        let bool_arr = Value::array(vector![Value::integer(1), Value::integer(0)]);
        let converted = bool_arr.to_boolean_value().unwrap();
        assert!(converted.is_array());
        let arr = converted.as_array().unwrap();
        assert_eq!(arr[0], Value::boolean(true));
        assert_eq!(arr[1], Value::boolean(false));

        // Integer
        assert_eq!(Value::integer(42).to_integer(), Some(42));
        assert_eq!(Value::boolean(true).to_integer(), Some(1));
        assert_eq!(Value::boolean(false).to_integer(), Some(0));
        assert_eq!(Value::number(42.9).to_integer(), Some(42));
        assert_eq!(Value::string("42").to_integer(), Some(42));
        assert_eq!(Value::unit().to_integer(), None);

        let int_arr = Value::array(vector![Value::string("10"), Value::boolean(true)]);
        let converted = int_arr.to_integer_value().unwrap();
        assert!(converted.is_array());
        let arr = converted.as_array().unwrap();
        assert_eq!(arr[0], Value::integer(10));
        assert_eq!(arr[1], Value::integer(1));

        // Number
        assert_eq!(Value::number(2.5).to_number(), Some(2.5));
        assert_eq!(Value::integer(42).to_number(), Some(42.0));
        assert_eq!(Value::boolean(true).to_number(), Some(1.0));
        assert_eq!(Value::string("2.5").to_number(), Some(2.5));
        assert_eq!(Value::unit().to_number(), None);

        let num_arr = Value::array(vector![Value::integer(10), Value::string("0.5")]);
        let converted = num_arr.to_number_value().unwrap();
        assert!(converted.is_array());
        let arr = converted.as_array().unwrap();
        assert_eq!(arr[0], Value::number(10.0));
        assert_eq!(arr[1], Value::number(0.5));

        // String
        assert_eq!(
            Value::string("hello").to_string(),
            Some("hello".to_string())
        );
        assert_eq!(Value::integer(42).to_string(), Some("42".to_string()));
        assert_eq!(Value::boolean(true).to_string(), Some("true".to_string()));
        assert_eq!(Value::number(2.5).to_string(), Some("2.5".to_string()));
        #[cfg(feature = "llm")]
        assert_eq!(
            Value::message(Message::user("content".to_string())).to_string(),
            Some("content".to_string())
        );
        assert_eq!(Value::unit().to_string(), None);

        let str_arr = Value::array(vector![Value::integer(42), Value::boolean(false)]);
        let converted = str_arr.to_string_value().unwrap();
        assert!(converted.is_array());
        let arr = converted.as_array().unwrap();
        assert_eq!(arr[0], Value::string("42"));
        assert_eq!(arr[1], Value::string("false"));
    }
}
