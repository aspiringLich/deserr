//! This module holds some pre-made error types to eases your usage of deserr

pub mod helpers;
pub mod query_params;

pub use query_params::QueryParamError;

#[cfg(feature = "serde-json")]
pub mod json;
#[cfg(feature = "serde-json")]
pub use json::JsonError;
