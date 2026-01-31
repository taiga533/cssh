mod args;
mod ssh;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli_args = match args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("argument error: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = ssh::run(cli_args).await {
        eprintln!("cssh error: {}", e);
        std::process::exit(1);
    }
}
