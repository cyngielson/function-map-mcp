// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Live Function Tree MCP Server
//!
//! Ultra-fast function indexing MCP server extracted from Ultra-Brain.
//! Provides real-time function tree visualization with smart junk filtering.
//!
//! Features:
//! - Milisecond PSI indexing performance
//! - SQLite-based persistent storage
//! - Real-time file watching
//! - Smart junk filtering (getters/setters/boilerplate)
//! - Multi-language support (Rust, Python, JS/TS)
//! - MCP protocol compliance

use anyhow::Result;
use env_logger::Env;
use log::info;
use std::env;

mod psi_graph;
mod function_extractor;
mod junk_filter;
mod file_watcher;
mod mcp_protocol;
mod hierarchical_tree;
mod hierarchical_tree_enhanced;  // 🆕 Enhanced hierarchical tree with semantic module grouping
mod ultra_fast_scanner;
mod tree_sitter_extractor;
mod regex_patterns;
mod simple_cfg_analyzer;
mod m1_formatter;  // M1 ULTRA-MINIMAL: 82% token reduction

/// Configure Rayon thread pool
/// Default: 2x CPU cores (for I/O + CPU balance)
/// Override with LFT_THREADS env variable
fn configure_thread_pool() {
    let cpu_count = num_cpus::get();
    let default_threads = cpu_count * 2; // 2x for I/O overlap

    let num_threads = env::var("LFT_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default_threads);

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .thread_name(|i| format!("lft-worker-{}", i))
        .build_global()
        .expect("Failed to configure Rayon thread pool");

    info!("🔧 Thread pool: {} workers (CPU cores: {})", num_threads, cpu_count);
}

fn init_logging() {
    // Na Windows klienci uruchamiani przez Python subprocess(text=True) potrafią dekodować stderr jako cp1252
    // i wywalać się na emoji. Dlatego mamy tryb ASCII-only: LFT_ASCII_ONLY=1.
    let ascii_only = env::var("LFT_ASCII_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if !ascii_only {
        env_logger::Builder::from_env(Env::default().default_filter_or("info"))
            .format_timestamp_millis()
            .init();
        return;
    }

    use env_logger::Builder;
    use log::LevelFilter;
    use std::io::Write;

    let mut builder = Builder::from_env(Env::default());
    if env::var("RUST_LOG").is_err() {
        builder.filter_level(LevelFilter::Info);
    }

    builder
        .format_timestamp_millis()
        .format(|buf, record| {
            let mut msg = format!("{} {}\n", record.level(), record.args());

            // Usuń znane emoji / symbole, które rozjeżdżają decode na Windows.
            for bad in [
                "✅", "🚀", "⚡", "📂", "📦", "📊", "📡", "📨", "🔍", "🧠", "🗑️", "🔧", "🌳",
            ] {
                msg = msg.replace(bad, "");
            }

            buf.write_all(msg.as_bytes())
        })
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging();

    // Configure parallel processing - 2x CPU cores by default
    configure_thread_pool();

    info!("🚀 Starting Live Function Tree MCP Server");
    info!("⚡ Ultra-fast PSI indexing with milisecond performance");
    info!("🧠 Extracted from Ultra-Brain for maximum speed");

    // Create and run the MCP server
    let mut handler = mcp_protocol::McpProtocolHandler::new().await?;
    handler.run().await?;

    Ok(())
}
