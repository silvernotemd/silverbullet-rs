use serde::Serialize;
use thiserror::Error;

#[cfg(feature = "minijinja")]
pub mod minijinja;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct Error(#[from] Box<dyn std::error::Error + Send + Sync>);

pub trait Renderer {
    fn render(&self, data: Data) -> Result<String, Error>;
}

#[derive(Debug, Clone, Serialize)]
pub struct Data {
    pub base_url: String,
    pub title: String,
    pub description: String,
    pub content: String,
}
