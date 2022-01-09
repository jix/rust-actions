use tracing_subscriber::EnvFilter;

static KEY_SPACE: &str = "9796546c64ab15ab7468b479f3b3c20d5840af05ac0f999ad7a089512d01572e";

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    println!("go");

    let cache =
        rust_actions_cache_api::Cache::new("jix/rust-actions/cache-api/examples/cache_util.rs")?;

    let keys = std::env::args().nth(1).unwrap();

    if let Some(data) = std::env::args().nth(2) {
        let result = cache.put_bytes(KEY_SPACE, &keys, data.into()).await?;

        println!("{:?}", result);
    } else {
        let result = cache.get_bytes(KEY_SPACE, &[&keys]).await?;

        println!("{:?}", result);
    }

    Ok(())
}
