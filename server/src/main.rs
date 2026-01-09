use axum::extract::FromRef;
use http::request::Parts;
use opendal::{Operator, services::Memory};
use silverbullet::{client, fs::opendal::Filesystem, server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone, FromRef)]
pub struct AppState {
    config: client::Config,
    operator: Operator,
}

impl AppState {
    pub fn new(config: client::Config, operator: Operator) -> Self {
        Self { config, operator }
    }
}

impl server::fs::FilesystemProvider for AppState {
    type Fs = Filesystem;

    fn create_fs(&self, _parts: &mut Parts) -> Result<Self::Fs, server::Error> {
        Ok(Filesystem::new(self.operator.clone()))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = client::Config {
        space_folder_path: "/".to_string(),
        index_page: "index".to_string(),
        read_only: false,
        log_push: false,
        enable_client_encryption: false,
    };

    let operator = Operator::new(Memory::default())
        .expect("failed to create memory operator")
        .finish();

    let state = AppState::new(config, operator);

    let app = server::router().with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .await
        .expect("failed to start server");
}
