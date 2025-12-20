use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    pub code: u16,
    pub stdout: String,
    pub stderr: String,
}
