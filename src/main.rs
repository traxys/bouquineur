use std::{path::PathBuf, str::FromStr, sync::Arc};

use anyhow::{anyhow, Context};
use axum::{http::HeaderName, routing::get, Router};
use diesel::Connection;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use metadata::MetadataProvider;
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
struct CalibreConfig {
    fetcher: String,
}

#[derive(serde::Deserialize, Debug)]
struct OpenLibraryConfig {
    contact: String,
}

#[derive(serde::Deserialize, Debug)]
struct MetadataConfig {
    #[serde(default)]
    providers: Option<Vec<MetadataProvider>>,
    #[serde(default)]
    default_provider: Option<MetadataProvider>,
    image_dir: PathBuf,

    #[serde(default)]
    calibre: Option<CalibreConfig>,
    #[serde(default)]
    open_library: Option<OpenLibraryConfig>,
}

impl MetadataConfig {
    fn check_calibre(&self) -> anyhow::Result<()> {
        let has = match &self.providers {
            None => true,
            Some(v) => v.contains(&MetadataProvider::Calibre),
        };

        match has && self.calibre.is_none() {
            true => Err(anyhow!("Missing `[metadata.calibre]`")),
            false => Ok(()),
        }
    }

    fn check_openlibrary(&self) -> anyhow::Result<()> {
        let has = match &self.providers {
            None => true,
            Some(v) => v.contains(&MetadataProvider::OpenLibrary),
        };

        match has && self.open_library.is_none() {
            true => Err(anyhow!("Missing `[metadata.open_library]`")),
            false => Ok(()),
        }
    }
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

fn run_migrations(state: &AppState) -> anyhow::Result<()> {
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

    let mut conn = diesel::PgConnection::establish(&state.config.database.url)?;

    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
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

    cfg.metadata.check_calibre()?;
    cfg.metadata.check_openlibrary()?;

    if let Some(p) = &cfg.metadata.providers {
        match &cfg.metadata.default_provider {
            None => {
                if p.len() > 1 {
                    anyhow::bail!(
                        "When more than one providers are enabled a default must be chosen"
                    )
                }
            }
            Some(def) => {
                if !p.contains(def) {
                    anyhow::bail!(
                        "metadata.default_provider ({def:?}) must be present in metadata.providers"
                    )
                }
            }
        }
    }

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

    run_migrations(&state)?;

    let app = Router::new()
        .route("/", get(routes::index))
        .route("/add", get(routes::add_book).post(routes::do_add_book))
        .route("/images/not_found", get(routes::image_not_found))
        .route("/images/:id", get(routes::image))
        .route("/book/:id", get(routes::get_book))
        .route("/unread", get(routes::unread))
        .route(
            "/book/:id/edit",
            get(routes::edit_book).post(routes::do_edit_book),
        )
        .route("/series", get(routes::series))
        .route("/series/:id", get(routes::get_series))
        .route(
            "/series/:id/edit",
            get(routes::series_edit).post(routes::do_series_edit),
        )
        .route("/author/:id", get(routes::get_author))
        .route("/ongoing", get(routes::ongoing))
        .route(
            "/profile",
            get(routes::profile).post(routes::do_edit_profile),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| "Could not create TCP Listener")?;

    axum::serve(listener, app).await?;

    Ok(())
}
