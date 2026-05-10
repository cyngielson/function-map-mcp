// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Junk Filter - Inteligentny filtr funkcji śmieciowych
//!
//! Zaawansowane algorytmy do filtrowania:
//! - Gettery/settery (getName, setName)
//! - Konstruktory boilerplate
//! - Funkcje jednoliniowe
//! - Auto-generated kod
//! - Test helpers
//! - Trivial functions

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use regex::Regex;
use log::debug;

use crate::psi_graph::{FunctionInfo};

/// Filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JunkFilterConfig {
    pub filter_getters_setters: bool,
    pub filter_constructors: bool,
    pub filter_one_liners: bool,
    pub filter_test_helpers: bool,
    pub filter_generated_code: bool,
    pub min_complexity: usize,
    pub min_lines: usize,
    pub preserve_important: bool,
}

impl Default for JunkFilterConfig {
    fn default() -> Self {
        Self {
            filter_getters_setters: true,
            filter_constructors: true,
            filter_one_liners: true,
            filter_test_helpers: true,
            filter_generated_code: true,
            min_complexity: 2,
            min_lines: 3,
            preserve_important: true,
        }
    }
}

/// Reasons why a function was filtered
/// Reasons why a function was filtered
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FilterReason {
    Getter,
    Setter,
    Constructor,
    OneLiner,
    TestHelper,
    GeneratedCode,
    LowComplexity,
    TooShort,
    Boilerplate,
}
/// Result of filtering operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterResult {
    pub kept_functions: Vec<FunctionInfo>,
    pub filtered_functions: Vec<(FunctionInfo, Vec<FilterReason>)>,
    pub statistics: FilterStatistics,
}

/// Filtering statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStatistics {
    pub total_functions: usize,
    pub kept_functions: usize,
    pub filtered_functions: usize,
    pub filter_breakdown: std::collections::HashMap<FilterReason, usize>,
}

/// Smart junk filter for functions
pub struct JunkFilter {
    config: JunkFilterConfig,
    getter_patterns: Vec<Regex>,
    setter_patterns: Vec<Regex>,
    constructor_patterns: Vec<Regex>,
    test_patterns: Vec<Regex>,
    generated_patterns: Vec<Regex>,
    important_keywords: HashSet<String>,
}

impl JunkFilter {
    /// Create new junk filter with configuration
    pub fn new(config: JunkFilterConfig) -> anyhow::Result<Self> {
        let getter_patterns = vec![
            // Python getters
            Regex::new(r"^get_?\w+$")?,
            Regex::new(r"^is_?\w+$")?,
            Regex::new(r"^has_?\w+$")?,
            // Rust getters
            Regex::new(r"^\w+$")?, // Simple field accessors
            // JavaScript getters
            Regex::new(r"^get[A-Z]\w*$")?,
            Regex::new(r"^is[A-Z]\w*$")?,
        ];

        let setter_patterns = vec![
            // Python setters
            Regex::new(r"^set_?\w+$")?,
            // JavaScript setters
            Regex::new(r"^set[A-Z]\w*$")?,
        ];

        let constructor_patterns = vec![
            // Python constructors
            Regex::new(r"^__init__$")?,
            Regex::new(r"^new$")?,
            Regex::new(r"^create$")?,
            // Rust constructors
            Regex::new(r"^new$")?,
            Regex::new(r"^from_\w+$")?,
            Regex::new(r"^with_\w+$")?,
            // JavaScript constructors
            Regex::new(r"^constructor$")?,
        ];

        let test_patterns = vec![
            Regex::new(r"^test_\w+$")?,
            Regex::new(r"^\w+_test$")?,
            Regex::new(r"^it_\w+$")?,
            Regex::new(r"^should_\w+$")?,
            Regex::new(r"^expect_\w+$")?,
            Regex::new(r"^mock_\w+$")?,
            Regex::new(r"^stub_\w+$")?,
            Regex::new(r"^setup_?\w*$")?,
            Regex::new(r"^teardown_?\w*$")?,
        ];

        let generated_patterns = vec![
            Regex::new(r"^__\w+__$")?, // Python dunder methods
            Regex::new(r"^_generated_\w+$")?,
            Regex::new(r"^auto_\w+$")?,
        ];

        // Important keywords that should preserve functions
        let mut important_keywords = HashSet::new();
        important_keywords.extend([
            // Business logic
            "process", "handle", "execute", "run", "start", "stop", "init", "main",
            "calculate", "compute", "analyze", "validate", "verify", "authenticate",
            "authorize", "login", "logout", "register", "create", "update", "delete",
            "save", "load", "fetch", "send", "receive", "parse", "format", "transform",

            // API endpoints
            "api", "endpoint", "route", "handler", "controller", "service", "manager",

            // Core algorithms
            "algorithm", "sort", "search", "filter", "map", "reduce", "merge", "split",

            // Database operations
            "query", "select", "insert", "update", "delete", "migrate", "seed",

            // Critical operations
            "critical", "important", "core", "essential", "key", "primary", "main",
        ].iter().map(|s| s.to_string()));

        Ok(Self {
            config,
            getter_patterns,
            setter_patterns,
            constructor_patterns,
            test_patterns,
            generated_patterns,
            important_keywords,
        })
    }

    /// Filter functions and remove junk
    pub fn filter_functions(&self, functions: Vec<FunctionInfo>) -> FilterResult {
        let mut kept_functions = Vec::new();
        let mut filtered_functions = Vec::new();
        let mut filter_breakdown = std::collections::HashMap::new();

        debug!("🧹 Starting junk filtering of {} functions", functions.len());

        for function in functions {
            let reasons = self.should_filter_function(&function);

            if reasons.is_empty() {
                kept_functions.push(function);
            } else {
                // Update statistics
                for reason in &reasons {
                    *filter_breakdown.entry(reason.clone()).or_insert(0) += 1;
                }
                filtered_functions.push((function, reasons));
            }
        }

        let statistics = FilterStatistics {
            total_functions: kept_functions.len() + filtered_functions.len(),
            kept_functions: kept_functions.len(),
            filtered_functions: filtered_functions.len(),
            filter_breakdown,
        };

        debug!("✅ Junk filtering completed: kept {}, filtered {}",
               kept_functions.len(), filtered_functions.len());

        FilterResult {
            kept_functions,
            filtered_functions,
            statistics,
        }
    }

    /// Determine if a function should be filtered
    fn should_filter_function(&self, function: &FunctionInfo) -> Vec<FilterReason> {
        let mut reasons = Vec::new();

        // Check if function is important (preserve important functions)
        if self.config.preserve_important && self.is_important_function(function) {
            return reasons; // Don't filter important functions
        }

        // Check getters
        if self.config.filter_getters_setters && self.is_getter(function) {
            reasons.push(FilterReason::Getter);
        }

        // Check setters
        if self.config.filter_getters_setters && self.is_setter(function) {
            reasons.push(FilterReason::Setter);
        }

        // Check constructors
        if self.config.filter_constructors && self.is_constructor(function) {
            reasons.push(FilterReason::Constructor);
        }

        // Check one-liners
        if self.config.filter_one_liners && self.is_one_liner(function) {
            reasons.push(FilterReason::OneLiner);
        }

        // Check test helpers
        if self.config.filter_test_helpers && self.is_test_helper(function) {
            reasons.push(FilterReason::TestHelper);
        }

        // Check generated code
        if self.config.filter_generated_code && self.is_generated_code(function) {
            reasons.push(FilterReason::GeneratedCode);
        }

        // Check complexity
        if function.complexity.unwrap_or(1) < self.config.min_complexity {
            reasons.push(FilterReason::LowComplexity);
        }

        // Check function length
        let function_length = function.end_line.saturating_sub(function.start_line);
        if function_length < self.config.min_lines {
            reasons.push(FilterReason::TooShort);
        }

        // Check if it's boilerplate
        if self.is_boilerplate(function) {
            reasons.push(FilterReason::Boilerplate);
        }

        reasons
    }

    /// Check if function is a getter
    fn is_getter(&self, function: &FunctionInfo) -> bool {
        // Check name patterns
        for pattern in &self.getter_patterns {
            if pattern.is_match(&function.name) {
                return true;
            }
        }

        // Check for typical getter characteristics (simplified)
        match function.language.as_str() {
            "python" => {
                // Python getter: simple functions with get/is/has prefix
                function.complexity.unwrap_or(1) <= 1 &&
                (function.name.starts_with("get_") ||
                 function.name.starts_with("is_") ||
                 function.name.starts_with("has_"))
            }
            "rust" => {
                // Rust getter: simple functions that likely return fields
                function.complexity.unwrap_or(1) <= 1 &&
                (function.name.starts_with("get_") ||
                 !function.name.contains("_"))  // simple field name
            }
            "javascript" | "typescript" => {
                // JS/TS getter: simple return functions
                function.complexity.unwrap_or(1) <= 1 &&
                function.name.starts_with("get")
            }
            _ => false
        }
    }

    /// Check if function is a setter
    fn is_setter(&self, function: &FunctionInfo) -> bool {
        // Check name patterns
        for pattern in &self.setter_patterns {
            if pattern.is_match(&function.name) {
                return true;
            }
        }

        // Check for typical setter characteristics
        match function.language.as_str() {
            "python" => {
                // Python setter: simple set_ functions
                function.complexity.unwrap_or(1) <= 1 &&
                function.name.starts_with("set_")
            }
            "javascript" | "typescript" => {
                // JS/TS setter: simple set functions
                function.complexity.unwrap_or(1) <= 1 &&
                function.name.starts_with("set")
            }
            _ => false
        }
    }

    /// Check if function is a constructor
    fn is_constructor(&self, function: &FunctionInfo) -> bool {
        for pattern in &self.constructor_patterns {
            if pattern.is_match(&function.name) {
                return true;
            }
        }

        // Language-specific constructor checks
        match function.language.as_str() {
            "python" => function.name == "__init__",
            "rust" => function.name == "new" || function.name.starts_with("from_"),
            "javascript" | "typescript" => function.name == "constructor",
            _ => false
        }
    }

    /// Check if function is a test helper
    fn is_test_helper(&self, function: &FunctionInfo) -> bool {
        for pattern in &self.test_patterns {
            if pattern.is_match(&function.name) {
                return true;
            }
        }

        // Check file path for test indicators
        let file_lower = function.file_path.to_lowercase();
        file_lower.contains("test") ||
        file_lower.contains("spec") ||
        file_lower.contains("mock") ||
        file_lower.contains("__tests__")
    }

    /// Check if function is generated code
    fn is_generated_code(&self, function: &FunctionInfo) -> bool {
        for pattern in &self.generated_patterns {
            if pattern.is_match(&function.name) {
                return true;
            }
        }

        // Check for auto-generated indicators in function name or signature
        let name_lower = function.name.to_lowercase();
        let sig_lower = function.signature.to_lowercase();
        name_lower.contains("generated") ||
        name_lower.contains("auto_") ||
        sig_lower.contains("generated") ||
        sig_lower.contains("auto-gen")
    }

    /// Check if function is one-liner
    fn is_one_liner(&self, function: &FunctionInfo) -> bool {
        let function_length = function.end_line.saturating_sub(function.start_line);
        function_length <= 1 && function.complexity.unwrap_or(1) <= 1
    }

    /// Check if function is boilerplate
    fn is_boilerplate(&self, function: &FunctionInfo) -> bool {
        // Common boilerplate patterns
        let boilerplate_names = [
            "toString", "valueOf", "equals", "hashCode", "clone",
            "__str__", "__repr__", "__eq__", "__hash__", "__copy__",
            "toJSON", "fromJSON", "serialize", "deserialize",
        ];

        boilerplate_names.contains(&function.name.as_str()) ||
        (function.complexity.unwrap_or(1) <= 1)
    }

    /// Check if function is important and should be preserved
    fn is_important_function(&self, function: &FunctionInfo) -> bool {
        let name_lower = function.name.to_lowercase();

        // Check for important keywords in function name
        for keyword in &self.important_keywords {
            if name_lower.contains(keyword) {
                return true;
            }
        }

        // Check for main/entry point functions
        if function.name == "main" || function.name == "run" || function.name == "start" {
            return true;
        }

        // Check for API endpoints or handlers
        if name_lower.contains("handler") ||
           name_lower.contains("controller") ||
           name_lower.contains("endpoint") ||
           name_lower.contains("route") {
            return true;
        }

        // Check for high complexity functions (likely important business logic)
        if function.complexity.unwrap_or(1) > 10 {
            return true;
        }

        // Check for public API functions
        if !function.name.starts_with('_') {
            return true;
        }

        false
    }

    /// Get comprehensive filter statistics
    pub fn get_filter_stats(&self, result: &FilterResult) -> String {
        let mut stats = format!(
            "🧹 JUNK FILTER RESULTS\n\
             =====================\n\
             Total Functions: {}\n\
             Kept Functions: {} ({:.1}%)\n\
             Filtered Functions: {} ({:.1}%)\n\n",
            result.statistics.total_functions,
            result.statistics.kept_functions,
            (result.statistics.kept_functions as f64 / result.statistics.total_functions as f64) * 100.0,
            result.statistics.filtered_functions,
            (result.statistics.filtered_functions as f64 / result.statistics.total_functions as f64) * 100.0
        );

        stats.push_str("Filter Breakdown:\n");
        for (reason, count) in &result.statistics.filter_breakdown {
            stats.push_str(&format!("  {:?}: {}\n", reason, count));
        }

        stats
    }
}

/// Create a smart filter configuration for different purposes
impl JunkFilterConfig {
    /// Configuration for development (aggressive filtering)
    pub fn development() -> Self {
        Self {
            filter_getters_setters: true,
            filter_constructors: true,
            filter_one_liners: true,
            filter_test_helpers: true,
            filter_generated_code: true,
            min_complexity: 3,
            min_lines: 2,
            preserve_important: true,
        }
    }

    /// Configuration for code review (moderate filtering)
    pub fn code_review() -> Self {
        Self {
            filter_getters_setters: true,
            filter_constructors: false,
            filter_one_liners: true,
            filter_test_helpers: false,
            filter_generated_code: true,
            min_complexity: 2,
            min_lines: 1,
            preserve_important: true,
        }
    }

    /// Configuration for analysis (minimal filtering)
    pub fn analysis() -> Self {
        Self {
            filter_getters_setters: false,
            filter_constructors: false,
            filter_one_liners: false,
            filter_test_helpers: true,
            filter_generated_code: true,
            min_complexity: 1,
            min_lines: 0,
            preserve_important: true,
        }
    }
}
