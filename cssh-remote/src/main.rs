mod client;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("使い方: cexec <command> [args...]");
        std::process::exit(1);
    }

    let port: u16 = std::env::var("CSSH_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9999);

    match client::execute(port, args).await {
        Ok(response) => {
            use std::io::Write;
            std::io::stdout().write_all(&response.stdout).ok();
            std::io::stderr().write_all(&response.stderr).ok();
            std::process::exit(response.exit_code);
        }
        Err(e) => {
            eprintln!("cssh-remote エラー: {}", e);
            std::process::exit(1);
        }
    }
}
