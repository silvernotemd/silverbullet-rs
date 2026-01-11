use axum::{
    Json,
    extract::{FromRef, State},
    response::IntoResponse,
};
use http::StatusCode;

use crate::shell::{Handler, Request, Response};

pub trait ShellProvider {
    type Shell: Handler + Clone + Send + Sync;

    fn shell(&self) -> Self::Shell;
}

pub struct Shell<S>(pub S);

impl<S> FromRef<S> for Shell<S::Shell>
where
    S: ShellProvider + Send + Sync,
{
    fn from_ref(state: &S) -> Self {
        Shell(state.shell())
    }
}

pub async fn shell<S>(
    State(Shell(shell)): State<Shell<S>>,
    Json(request): Json<Request>,
) -> Result<Json<Response>, impl IntoResponse>
where
    S: Handler,
{
    shell
        .handle(request)
        .map(|resp| Json(resp))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
