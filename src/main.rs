use anyhow::Context;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Config {
}

fn main() -> anyhow::Result<()> {
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

    Ok(())
}
