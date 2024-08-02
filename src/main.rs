use anyhow::Context;
use diesel_async::{AsyncConnection, AsyncPgConnection};

mod schema;

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
struct Config {
    #[serde(default)]
    debug: DebugConfig,
    auth: AuthConfig,
    database: DatabaseConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let _connection = AsyncPgConnection::establish(&cfg.database.url)
        .await
        .with_context(|| format!("While connecting to the database at {}", cfg.database.url))?;

    Ok(())
}
