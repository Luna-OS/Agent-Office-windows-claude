//! `open-agentic-office`: startet das Office (siehe `--hilfe`).

use server::konfig::{self, Befehl};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("OFFICE_LOG").unwrap_or_else(|_| "info".into()))
        .with_target(false)
        .init();
    let befehl = match konfig::lesen(std::env::args().skip(1)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Fehler: {e:#}\n\n{}", konfig::HILFE);
            std::process::exit(2);
        }
    };
    let ergebnis = match befehl {
        Befehl::Hilfe => {
            print!("{}", konfig::HILFE);
            Ok(())
        }
        Befehl::Version => {
            println!("open-agentic-office {}", server::VERSION);
            Ok(())
        }
        Befehl::Konten { daten, argumente } => server::konten_cli::ausfuehren(&daten, &argumente),
        Befehl::Starten(k) => tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(anyhow::Error::from).and_then(|rt| rt.block_on(server::starten(*k))),
    };
    if let Err(e) = ergebnis {
        eprintln!("Fehler: {e:#}");
        std::process::exit(1);
    }
}
