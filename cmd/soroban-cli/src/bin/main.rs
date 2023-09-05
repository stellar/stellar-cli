use clap::CommandFactory;
use dotenvy::dotenv;
use tracing_subscriber::{fmt, EnvFilter};

use soroban_cli::{commands, Root};

#[tokio::main]
async fn main() {
    let _ = dotenv().unwrap_or_default();
    let mut root = Root::new().unwrap_or_else(|e| match e {
        commands::Error::Clap(e) => {
            let mut cmd = Root::command();
            e.format(&mut cmd).exit();
        }
        e => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    });
    // Now use root to setup the logger
    if let Some(level) = root.global_args.log_level() {
        let mut e_filter = EnvFilter::from_default_env()
            .add_directive("hyper=off".parse().unwrap())
            .add_directive(format!("soroban_cli={level}").parse().unwrap());

        for filter in &root.global_args.filter_logs {
            e_filter = e_filter.add_directive(
                filter
                    .parse()
                    .map_err(|e| {
                        eprintln!("{e}: {filter}");
                        std::process::exit(1);
                    })
                    .unwrap(),
            );
        }

        let builder = fmt::Subscriber::builder()
            .with_env_filter(e_filter)
            .with_writer(std::io::stderr);

        let subscriber = builder.finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set the global tracing subscriber");
    }

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
