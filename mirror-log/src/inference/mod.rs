#[path = "trait.rs"]
mod backend_trait;

pub mod http_backend;

pub use backend_trait::{
    Event, InferenceBackend, InferenceConfig, InferenceError, WasmInferenceBackend,
};
pub use http_backend::{HttpBackend, HttpConfig, HttpError};
