#![allow(dead_code)]

use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};

#[derive(Debug, Clone, PartialEq)]
pub enum ParsecEvent {
    ClientConnected(String),  // Username
    ClientDisconnected(String),
}

pub struct ParsecWatcher {
    log_path: PathBuf,
    event_tx: broadcast::Sender<ParsecEvent>,
}

impl ParsecWatcher {
    pub fn new() -> (Self, broadcast::Receiver<ParsecEvent>) {
        let (event_tx, event_rx) = broadcast::channel(16);
        let log_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".parsec")
            .join("log.txt");

        (Self { log_path, event_tx }, event_rx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ParsecEvent> {
        self.event_tx.subscribe()
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting Parsec log watcher: {:?}", self.log_path);

        if !self.log_path.exists() {
            warn!("Parsec log file not found: {:?}", self.log_path);
            return Ok(());
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(&self.log_path, RecursiveMode::NonRecursive)?;

        // Open file and seek to end
        let mut file = File::open(&self.log_path)?;
        file.seek(SeekFrom::End(0))?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        let event_tx = self.event_tx.clone();

        // Process file changes
        loop {
            match rx.recv() {
                Ok(_event) => {
                    // Read new lines
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) => break, // No more data
                            Ok(_) => {
                                if let Some(event) = Self::parse_line(&line) {
                                    info!("Parsec event: {:?}", event);
                                    let _ = event_tx.send(event);
                                }
                            }
                            Err(e) => {
                                error!("Error reading log: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Watch error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    fn parse_line(line: &str) -> Option<ParsecEvent> {
        // Pattern: "Username#12345678 connected."
        // Pattern: "Username#12345678 disconnected."

        if line.contains(" connected.") {
            // Extract username from pattern "username#id connected."
            if let Some(user_part) = line.split(" connected.").next() {
                // Find the username part (after the timestamp)
                if let Some(username) = user_part.split(']').last() {
                    let username = username.trim();
                    if !username.is_empty() {
                        return Some(ParsecEvent::ClientConnected(username.to_string()));
                    }
                }
            }
        }

        if line.contains(" disconnected.") {
            if let Some(user_part) = line.split(" disconnected.").next() {
                if let Some(username) = user_part.split(']').last() {
                    let username = username.trim();
                    if !username.is_empty() {
                        return Some(ParsecEvent::ClientDisconnected(username.to_string()));
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connected() {
        let line = "[I 2026-04-06 10:17:53] Username#12345678 connected.";
        let event = ParsecWatcher::parse_line(line);
        assert!(matches!(event, Some(ParsecEvent::ClientConnected(_))));
    }

    #[test]
    fn test_parse_disconnected() {
        let line = "[I 2026-04-06 10:18:44] Username#12345678 disconnected.";
        let event = ParsecWatcher::parse_line(line);
        assert!(matches!(event, Some(ParsecEvent::ClientDisconnected(_))));
    }
}
