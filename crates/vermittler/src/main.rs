//! `agent-office-vermittler`: startet den Vermittler (siehe `vermittler::konfig::HILFE`).

use vermittler::konfig::{self, Aufruf};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let konfig = match konfig::lesen(&args, konfig::Umgebung::lesen()) {
        Ok(Aufruf::Starten(k)) => k,
        Ok(Aufruf::Hilfe) => {
            print!("{}", konfig::HILFE);
            return;
        }
        Ok(Aufruf::Version) => {
            println!("agent-office-vermittler {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Err(f) => {
            eprintln!("Fehler: {f}");
            std::process::exit(2);
        }
    };
    let (adresse, aufgabe) = match vermittler::starten(konfig).await {
        Ok(v) => v,
        Err(f) => {
            eprintln!(
                "Fehler: Der Vermittler konnte nicht lauschen ({f}). Ist der Port schon belegt?"
            );
            std::process::exit(1);
        }
    };
    println!("Vermittler läuft auf {adresse}. Gastgeber melden sich unter /vermittler/knoten an.");
    tokio::select! {
        _ = aufgabe => {}
        _ = tokio::signal::ctrl_c() => println!("Vermittler beendet."),
    }
}
