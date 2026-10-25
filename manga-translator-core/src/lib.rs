// manga-translator-core/src/lib.rs
//
// Shared, framework-agnostic core for the manga-translator frontends.
//
// Contains everything that does not depend on a specific UI toolkit:
//   - `config`      — persisted settings / API keys and all option tables
//   - `ipc_bridge`  — newline-delimited JSON bridge to backend/server.py
//   - `i18n`        — gettext setup and lookups

pub mod config;
pub mod i18n;
pub mod ipc_bridge;
