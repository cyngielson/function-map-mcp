// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Pre-compiled regex patterns for ultra-fast function extraction
//!
//! All regexes are compiled once at startup using once_cell::sync::Lazy
//! This eliminates the overhead of regex compilation in hot loops

use once_cell::sync::Lazy;
use regex::Regex;

// ==================== PYTHON PATTERNS ====================

pub static PYTHON_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(async\s+)?def\s+(\w+)\s*\(([^)]*)\)").unwrap()
});

pub static PYTHON_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*class\s+(\w+)(?:\([^)]*\))?:").unwrap()
});

// ==================== RUST PATTERNS ====================

pub static RUST_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(pub\s+)?(async\s+)?fn\s+(\w+)").unwrap()
});

pub static RUST_IMPL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*impl(?:<[^>]+>)?\s+(?:(\w+)\s+for\s+)?(\w+)").unwrap()
});

pub static RUST_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(pub\s+)?struct\s+(\w+)").unwrap()
});

pub static RUST_TRAIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(pub\s+)?trait\s+(\w+)").unwrap()
});

// ==================== JAVASCRIPT/TYPESCRIPT PATTERNS ====================

pub static JS_FUNCTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(export\s+)?(async\s+)?function\s+(\w+)\s*\(").unwrap()
});

pub static JS_ARROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(export\s+)?(const|let|var)\s+(\w+)\s*=\s*(async\s+)?\([^)]*\)\s*=>").unwrap()
});

pub static JS_ARROW_SIMPLE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(export\s+)?(const|let|var)\s+(\w+)\s*=\s*(async\s+)?(\w+)\s*=>").unwrap()
});

pub static JS_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(export\s+)?class\s+(\w+)").unwrap()
});

pub static JS_METHOD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(async\s+)?(\w+)\s*\([^)]*\)\s*\{").unwrap()
});

// ==================== JAVA PATTERNS ====================

pub static JAVA_METHOD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:public|private|protected)?\s*(?:static)?\s*(?:\w+(?:<[^>]+>)?)\s+(\w+)\s*\(([^)]*)\)").unwrap()
});

pub static JAVA_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|protected)?\s*(?:abstract|final)?\s*class\s+(\w+)").unwrap()
});

// ==================== GO PATTERNS ====================

pub static GO_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"func\s+(?:\([^)]+\)\s+)?(\w+)\s*\(([^)]*)\)").unwrap()
});

// ==================== C/C++ PATTERNS ====================

pub static C_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:static\s+)?(?:inline\s+)?(?:const\s+)?(?:\w+\s*\*?\s+)+(\w+)\s*\(([^)]*)\)\s*\{?").unwrap()
});

pub static CPP_METHOD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:virtual\s+)?(?:static\s+)?(?:inline\s+)?(?:const\s+)?(?:\w+(?:<[^>]+>)?\s*\*?\s+)+(\w+)\s*\(([^)]*)\)").unwrap()
});

pub static CPP_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:template\s*<[^>]+>\s*)?class\s+(\w+)").unwrap()
});

// ==================== KOTLIN PATTERNS ====================

pub static KOTLIN_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:private|public|internal|protected)?\s*(?:suspend\s+)?(?:inline\s+)?fun\s+(?:<[^>]+>\s*)?(\w+)\s*\(").unwrap()
});

pub static KOTLIN_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:data\s+)?(?:sealed\s+)?(?:open\s+)?(?:abstract\s+)?class\s+(\w+)").unwrap()
});

pub static KOTLIN_OBJECT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:companion\s+)?object\s+(\w+)?").unwrap()
});

// ==================== SWIFT PATTERNS ====================

pub static SWIFT_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:@\w+\s+)*(?:private|public|internal|fileprivate|open)?\s*(?:static\s+)?(?:class\s+)?func\s+(\w+)\s*(?:<[^>]+>)?\s*\(").unwrap()
});

pub static SWIFT_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:final\s+)?(?:public|private|internal|fileprivate|open)?\s*class\s+(\w+)").unwrap()
});

pub static SWIFT_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|internal|fileprivate)?\s*struct\s+(\w+)").unwrap()
});

pub static SWIFT_PROTOCOL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|internal|fileprivate)?\s*protocol\s+(\w+)").unwrap()
});

pub static SWIFT_ENUM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|internal|fileprivate)?\s*enum\s+(\w+)").unwrap()
});

pub static SWIFT_INIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:required\s+)?(?:convenience\s+)?(?:public|private|internal|fileprivate|open)?\s*init\s*(?:\?|!)?\s*\(").unwrap()
});

// ==================== PHP PATTERNS ====================

pub static PHP_FUNCTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|protected)?\s*(?:static)?\s*function\s+(\w+)\s*\(([^)]*)\)").unwrap()
});

pub static PHP_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:abstract|final)?\s*class\s+(\w+)").unwrap()
});

// ==================== BASH PATTERNS ====================

pub static BASH_FUNCTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:function\s+)?(\w+)\s*\(\s*\)").unwrap()
});

// ==================== RUST FALLBACK PATTERNS ====================

pub static RUST_FALLBACK_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(([^)]*)\)").unwrap()
});

// ==================== JS FALLBACK PATTERNS ====================

/// Pattern: function name()
pub static JS_FALLBACK_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(([^)]*)\)").unwrap()
});

/// Pattern: const name = () =>
pub static JS_FALLBACK_ARROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s+)?\(([^)]*)\)\s*=>").unwrap()
});

/// Pattern: name: function()
pub static JS_FALLBACK_OBJ_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\w+)\s*:\s*(?:async\s+)?function\s*\(([^)]*)\)").unwrap()
});

/// Pattern: name: () =>
pub static JS_FALLBACK_OBJ_ARROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\w+)\s*:\s*(?:async\s+)?\(([^)]*)\)\s*=>").unwrap()
});

// ==================== RUBY PATTERNS ====================

pub static RUBY_DEF: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*def\s+(self\.)?(\w+[?!=]?)\s*(?:\(([^)]*)\))?").unwrap()
});

pub static RUBY_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*class\s+(\w+)(?:\s*<\s*(\w+))?").unwrap()
});

pub static RUBY_MODULE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*module\s+(\w+)").unwrap()
});

// ==================== C# PATTERNS ====================

pub static CSHARP_METHOD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|protected|internal)?\s*(?:static|virtual|override|abstract|async)?\s*(?:\w+(?:<[^>]+>)?)\s+(\w+)\s*\(([^)]*)\)").unwrap()
});

pub static CSHARP_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|protected|internal)?\s*(?:partial|abstract|sealed|static)?\s*class\s+(\w+)").unwrap()
});

pub static CSHARP_INTERFACE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:public|private|protected|internal)?\s*interface\s+(I\w+)").unwrap()
});

// ==================== SCALA PATTERNS ====================

pub static SCALA_DEF: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:private|protected)?\s*(?:override\s+)?def\s+(\w+)(?:\[[^\]]+\])?\s*(?:\(([^)]*)\))?").unwrap()
});

pub static SCALA_VAL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:private|protected)?\s*(?:lazy\s+)?val\s+(\w+)").unwrap()
});

pub static SCALA_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:abstract|sealed|final|case)?\s*class\s+(\w+)").unwrap()
});

pub static SCALA_OBJECT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:case\s+)?object\s+(\w+)").unwrap()
});

pub static SCALA_TRAIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:sealed\s+)?trait\s+(\w+)").unwrap()
});

// ==================== BASH PATTERNS ====================

pub static BASH_FUNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:function\s+)?(\w+)\s*\(\s*\)").unwrap()
});

// ==================== HTML PATTERNS ====================

pub static HTML_SCRIPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<script[^>]*(?:src=["']([^"']+)["'])?[^>]*>"#).unwrap()
});

pub static HTML_EVENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(on\w+)=["']([^"']+)["']"#).unwrap()
});

pub static HTML_FORM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<form[^>]*action=["']([^"']+)["'][^>]*>"#).unwrap()
});

pub static HTML_JS_LINK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<a[^>]*href=["'](javascript:[^"']+)["'][^>]*>"#).unwrap()
});

// ==================== JSON PATTERNS ====================

pub static JSON_SCRIPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"["']?(scripts?|command|start|build|test|dev|serve|url|endpoint|api)["']?\s*:\s*["']([^"']+)["']"#).unwrap()
});

// ==================== COMPLEXITY PATTERNS ====================

pub static COMPLEXITY_KEYWORDS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(if|else|elif|while|for|match|case|catch|except|try|&&|\|\||and|or)\b").unwrap()
});
