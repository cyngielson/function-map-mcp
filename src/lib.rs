// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
// Biblioteka dla testów i ponownego użycia modułów.
//
// Dlaczego: projekt był tylko binarny (src/main.rs), co utrudnia testy integracyjne
// w `tests/` (nie mogą zaimportować crate przez nazwę). Minimalny `lib.rs`
// pozwala pisać regresje dla krytycznych bugów (path mapping/cache).

pub mod psi_graph;
pub mod hierarchical_tree;
pub mod hierarchical_tree_enhanced; // 🆕 ENHANCED with semantic module grouping like vector DB
pub mod m1_formatter;  // M1 ULTRA-MINIMAL: 82% token reduction (snapshot-maker validated)

// Zależności modułów (psi_graph korzysta z tych modułów przez `crate::...`).
pub mod ultra_fast_scanner;
pub mod tree_sitter_extractor;
pub mod regex_patterns;
pub mod db_maintenance;

// CFG/DFG Analysis - Simple line-based approach (Phase 1)
pub mod simple_cfg_analyzer;

