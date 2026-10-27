//! Spielsachen im Office: Hund, Ball, Basketball, Golf, Arcade, Bar, Kaffee, Whiteboard ...

use serde::{Deserialize, Serialize};

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Platzhalter, bis die Domäne ihre echten Nachrichten hat.
    Platzhalter,
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Platzhalter, bis die Domäne ihre echten Nachrichten hat.
    Platzhalter,
}
