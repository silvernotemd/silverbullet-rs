use axum::{
    Json,
    extract::{FromRef, State},
    response::IntoResponse,
};
use http::StatusCode;

use crate::shell::{self, Request, Response};

pub trait Provider {
    type Output: shell::Shell + Send + Sync;

    fn provide(&self) -> Self::Output;
}

pub struct Shell<S>(pub S);

impl<S> FromRef<S> for Shell<S::Output>
where
    S: Provider + Send + Sync,
{
    fn from_ref(state: &S) -> Self {
        Shell(state.provide())
    }
}

pub async fn shell<S>(
    State(Shell(shell)): State<Shell<S>>,
    Json(request): Json<Request>,
) -> Result<Json<Response>, impl IntoResponse>
where
    S: shell::Shell,
{
    shell
        .exec(request)
        .map(Json::from)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
