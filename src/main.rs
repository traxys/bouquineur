use std::sync::Arc;

use anyhow::Context;
use axum::{routing::get, Router};
use diesel_async::{AsyncConnection, AsyncPgConnection};

mod models;
mod routes;
mod schema;

type State = axum::extract::State<Arc<AppState>>;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct AuthConfig {
    header: String,
    #[serde(default)]
    admin: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
struct DebugConfig {
    #[serde(default)]
    assume_user: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct DatabaseConfig {
    url: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct MetadataConfig {
    fetcher: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ServerConfig {
    port: u16,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Config {
    #[serde(default)]
    debug: DebugConfig,
    auth: AuthConfig,
    database: DatabaseConfig,
    server: ServerConfig,
}

struct AppState {
    config: Config,
    db: AsyncPgConnection,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut args = std::env::args();
    args.next();

    let cfg: Config = if let Some(arg) = args.next() {
        toml::from_str(
            &std::fs::read_to_string(&arg)
                .with_context(|| format!("Could not load the configuration file '{arg}'"))?,
        )
        .with_context(|| "Could not parse the configuration file")?
    } else if let Ok(arg) = std::env::var("BOUQUINEUR_CONFIG") {
        toml::from_str(
            &std::fs::read_to_string(&arg)
                .with_context(|| format!("Could not load the configuration file '{arg}'"))?,
        )
        .with_context(|| "Could not parse the configuration file")?
    } else {
        anyhow::bail!("No configuration was supplied");
    };

    let db = AsyncPgConnection::establish(&cfg.database.url)
        .await
        .with_context(|| format!("While connecting to the database at {}", cfg.database.url))?;

    let port = cfg.server.port;

    let state = Arc::new(AppState { config: cfg, db });

    let app = Router::new()
        .route("/", get(routes::index))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| "Could not create TCP Listener")?;

    axum::serve(listener, app).await?;

    Ok(())
}
