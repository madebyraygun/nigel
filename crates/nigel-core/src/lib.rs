//! Nigel's core — cash-basis bookkeeping for small consultancies and personal finances.
//!
//! This library holds the whole implementation apart from the terminal UI: the
//! SQLite data layer, importers, the rules engine, reports, invoicing, and the
//! web server. It links without clap, ratatui or crossterm, so a client that is
//! not a terminal can depend on it.

pub mod accounts;
pub mod backup;
pub mod categories;
pub mod categorizer;
pub mod clock;
pub mod db;
pub mod error;
pub mod fmt;
pub mod importer;
pub mod imports;
pub mod invoicing;
pub mod migrations;
pub mod models;
pub mod password;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod reconciler;
pub mod reports;
pub mod reviewer;
pub mod rules;
#[cfg(feature = "serve")]
pub mod server;
pub mod settings;
pub mod updater;
