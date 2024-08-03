use std::{path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Context;
use axum::{http::HeaderName, routing::get, Router};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncConnection, AsyncPgConnection,
};
use serde::Deserializer;

mod metadata;
mod models;
mod routes;
mod schema;

type State = axum::extract::State<Arc<AppState>>;

fn deserialize_hdr<'de, D>(de: D) -> Result<HeaderName, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrVisitor;
    impl<'de> serde::de::Visitor<'de> for StrVisitor {
        type Value = HeaderName;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "an HTTP header name")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            HeaderName::from_str(s)
                .map_err(|_| serde::de::Error::invalid_value(serde::de::Unexpected::Str(s), &self))
        }
    }

    de.deserialize_str(StrVisitor)
}

#[derive(serde::Deserialize, Debug)]
struct AuthConfig {
    #[serde(deserialize_with = "deserialize_hdr")]
    header: HeaderName,
    #[serde(default)]
    admin: Vec<String>,
}

#[derive(serde::Deserialize, Debug, Default)]
struct DebugConfig {
    #[serde(default)]
    assume_user: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct DatabaseConfig {
    url: String,
}

#[derive(serde::Deserialize, Debug)]
struct MetadataConfig {
    fetcher: String,
    image_dir: PathBuf,
}

#[derive(serde::Deserialize, Debug)]
struct ServerConfig {
    port: u16,
}

#[derive(serde::Deserialize, Debug)]
struct Config {
    #[serde(default)]
    debug: DebugConfig,
    metadata: MetadataConfig,
    auth: AuthConfig,
    database: DatabaseConfig,
    server: ServerConfig,
}

type PgPool = diesel_async::pooled_connection::deadpool::Pool<AsyncPgConnection>;

struct AppState {
    config: Config,
    db: PgPool,
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

    std::fs::create_dir_all(&cfg.metadata.image_dir)
        .with_context(|| "Could not create image directory")?;

    if let Some(user) = &cfg.debug.assume_user {
        tracing::warn!("Running in debug mode, user is assumed to be '{user}'");
    }

    let pool_config =
        AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(&cfg.database.url);
    let db = Pool::builder(pool_config)
        .build()
        .with_context(|| "Could not build database pool")?;

    let port = cfg.server.port;

    let state = Arc::new(AppState { config: cfg, db });

    let app = Router::new()
        .route("/", get(routes::index))
        .route("/add", get(routes::add_book))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| "Could not create TCP Listener")?;

    axum::serve(listener, app).await?;

    Ok(())
}
