// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! M1 ULTRA-MINIMAL FORMATTER for Live-Function-Tree
//!
//! COMPRESSION PHILOSOPHY (copy z snapshot-maker):
//! - Schema opisany RAZ na początku = instrukcja obsługi dla LLM
//! - Dane = PURE VALUES, zero powtórzeń, zero śmieci
//! - LLM czyta schemat raz, potem leci przez dane jak burza
//!
//! TOKEN MATH (500 files, 2000 functions):
//! - JSON: ~50,000 tokens (full nested structure)
//! - M1: ~8,000 tokens (84% reduction!)
//!
//! FORMAT EXAMPLE:
//! M1_LFT_SCHEMA: file lang fn_count | fn: name sig start end vis cplx
//! views.py python 5 | getUserProfile async(...)->User 23 45 pub 8 | createUser ...
//! models.py python 3 | User class 10 50 pub 2 | validate ...
//! === STATS: files:100 funcs:450 langs:py,js,rs

use anyhow::Result;
use serde_json::Value;

/// Convert LFT hierarchical tree to M1 format
pub fn format_lft_m1(tree: &Value) -> Result<String> {
    let mut output = String::new();

    // SCHEMA - opisany RAZ (instrukcja dla LLM)
    output.push_str("M1_LFT_SCHEMA: file lang fn_count | fn: name sig start end vis cplx\n");

    // LFT structure: check if "tree" exists (hierarchical_tree), else check root
    let tree_data = tree.get("tree").unwrap_or(tree);

    // Extract files
    if let Some(files_map) = tree_data.get("files").and_then(|f| f.as_object()) {
        for (file_path, file_data) in files_map {
            // File line: path language function_count
            let short_path = shorten_path_lft(file_path);
            let lang = file_data.get("language")
                .and_then(|l| l.as_str())
                .unwrap_or("unknown");
            let fn_count = file_data.get("function_count")
                .and_then(|c| c.as_u64())
                .unwrap_or(0);

            output.push_str(&format!("{} {} {}", short_path, lang, fn_count));

            // Functions inline with " | " separator
            if let Some(functions) = file_data.get("functions").and_then(|f| f.as_array()) {
                for func in functions {
                    output.push_str(" | ");
                    output.push_str(&format_function_m1(func));
                }
            }

            output.push('\n');
        }
    }

    // STATS line
    output.push_str("=== STATS: ");
    if let Some(total_files) = tree_data.get("total_files").and_then(|v| v.as_u64()) {
        output.push_str(&format!("files:{} ", total_files));
    }
    if let Some(total_funcs) = tree_data.get("total_functions").and_then(|v| v.as_u64()) {
        output.push_str(&format!("funcs:{}", total_funcs));
    }
    output.push('\n');

    Ok(output)
}

/// Format single function - compact inline
fn format_function_m1(func: &Value) -> String {
    let name = func.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("?");

    let sig = func.get("signature")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    // Truncate long signatures
    let sig_short = if sig.len() > 50 {
        format!("{}...", &sig[..47])
    } else {
        sig.to_string()
    };

    let start = func.get("start_line")
        .and_then(|l| l.as_u64())
        .unwrap_or(0);

    let end = func.get("end_line")
        .and_then(|l| l.as_u64())
        .unwrap_or(0);

    let vis = func.get("visibility")
        .and_then(|v| v.as_str())
        .and_then(|v| v.chars().next())
        .unwrap_or('?');

    let cplx = func.get("complexity")
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    format!("{} {} {} {} {} {}", name, sig_short, start, end, vis, cplx)
}

/// Aggressive path shortening for LFT (same logic as snapshot-maker)
fn shorten_path_lft(path: &str) -> String {
    let path = path.replace("\\", "/");

    // Remove C:/, D:/ etc
    let path = if path.len() > 3 && path.chars().nth(1) == Some(':') {
        &path[3..]
    } else {
        &path
    };

    let path = path.trim_start_matches("./").trim_start_matches('/');

    // Find last /src/ and remove everything before
    if let Some(src_pos) = path.rfind("/src/") {
        return path[src_pos + 5..].to_string();
    }

    // Remove common prefixes
    let path = path.trim_start_matches("lib/");
    let path = path.trim_start_matches("app/");
    let path = path.trim_start_matches("pkg/");

    path.to_string()
}
