//! Core types and traits for the AI Gateway translation layer.
//!
//! This crate defines:
//! - **Canonical model** (`model`): provider-neutral request/response types based on
//!   the OpenAI Chat Completions format. Clients speak this format; providers translate
//!   to/from it.
//! - **Translator traits** (`translate`): `RequestTranslator`, `ResponseTranslator`, and
//!   `StreamParser` — pure data-mapping interfaces with no IO or HTTP client dependency.
//! - **Error types** (`error`): `TranslateError` for translation failures,
//!   `ProviderError` for upstream API errors.

#![forbid(unsafe_code)]

pub mod error;
pub mod model;
pub mod translate;

/// Alias for JSON object pass-through fields.
///
/// Uses `serde_json::Map` for zero-cost interop with `Value::Object` and
/// zero-overhead `#[serde(flatten)]` deserialization.
pub type JsonObject = serde_json::Map<String, serde_json::Value>;

/// Returns `true` if the JSON object is empty (for `skip_serializing_if`).
pub fn json_object_is_empty(value: &JsonObject) -> bool {
    value.is_empty()
}

/// Current Unix time in seconds: the `created` stamp for a response that is
/// synthesised by a translator, or received from an upstream that omits it.
#[must_use]
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─── Generic utility types ─────────────────────────────────────────────────

/// Forward-compatible wrapper for tagged enums.
///
/// All provider wire types use internally-tagged enums (`#[serde(tag = "type")]`)
/// for content blocks, tool definitions, etc. When the provider adds a new variant,
/// deserialization would fail. This wrapper tries the strongly-typed `Known(T)` first,
/// then falls back to `Raw(JsonObject)` for any unrecognized shape.
///
/// # Example
///
/// ```
/// use aigw_core::{ForwardCompatible, JsonObject};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// #[serde(tag = "type", rename_all = "snake_case")]
/// pub enum TypedPart {
///     Text { text: String },
///     Image { url: String },
/// }
///
/// type Part = ForwardCompatible<TypedPart>;
///
/// // Known variant deserializes into Known
/// let json = r#"{"type":"text","text":"hello"}"#;
/// let part: Part = serde_json::from_str(json).unwrap();
/// assert!(part.is_known());
///
/// // Unknown variant falls back to Raw
/// let json = r#"{"type":"video","src":"movie.mp4"}"#;
/// let part: Part = serde_json::from_str(json).unwrap();
/// assert!(part.is_raw());
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ForwardCompatible<T> {
    /// Successfully deserialized into the known type.
    Known(T),
    /// Unrecognized structure — preserved as raw JSON for pass-through.
    Raw(JsonObject),
}

impl<T> ForwardCompatible<T> {
    /// Returns `true` if this is a known variant.
    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }

    /// Returns `true` if this fell back to raw JSON.
    pub fn is_raw(&self) -> bool {
        matches!(self, Self::Raw(_))
    }

    /// Returns a reference to the known value, if any.
    pub fn as_known(&self) -> Option<&T> {
        match self {
            Self::Known(t) => Some(t),
            Self::Raw(_) => None,
        }
    }

    /// Returns a reference to the raw JSON object, if any.
    pub fn as_raw(&self) -> Option<&JsonObject> {
        match self {
            Self::Known(_) => None,
            Self::Raw(obj) => Some(obj),
        }
    }

    /// Consumes self and returns the known value, if any.
    pub fn into_known(self) -> Option<T> {
        match self {
            Self::Known(t) => Some(t),
            Self::Raw(_) => None,
        }
    }
}

/// A value that can be a single item or an array.
///
/// Many APIs accept `"stop": "END"` or `"stop": ["END", "STOP"]`.
/// This type handles both forms transparently.
///
/// # Example
///
/// ```
/// use aigw_core::OneOrMany;
///
/// let one: OneOrMany<String> = serde_json::from_str(r#""hello""#).unwrap();
/// assert_eq!(one.into_vec(), vec!["hello"]);
///
/// let many: OneOrMany<String> = serde_json::from_str(r#"["a","b"]"#).unwrap();
/// assert_eq!(many.into_vec(), vec!["a", "b"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    /// A single value.
    One(T),
    /// An array of values.
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    /// Normalize into a `Vec<T>`, consuming self.
    pub fn into_vec(self) -> Vec<T> {
        match self {
            Self::One(v) => vec![v],
            Self::Many(v) => v,
        }
    }

    /// Clone elements into a `Vec<T>` without consuming self.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        match self {
            Self::One(v) => vec![v.clone()],
            Self::Many(v) => v.clone(),
        }
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(v) => v.len(),
        }
    }

    /// Returns `true` if there are no elements.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::One(_) => false,
            Self::Many(v) => v.is_empty(),
        }
    }
}
