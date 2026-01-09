use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Failed to run command")]
pub struct Error {}

pub trait Handler {
    fn handle(&self, request: Request) -> Result<Response, Error>;
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub cmd: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub code: u16,
    pub stdout: String,
    pub stderr: String,
}

pub struct NoShell {}

impl Handler for NoShell {
    fn handle(&self, _request: Request) -> Result<Response, Error> {
        Ok(Response {
            code: 1,
            stdout: "".to_string(),
            stderr: "Not supported".to_string(),
        })
    }
}
