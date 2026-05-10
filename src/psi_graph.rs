// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! PSI Graph Manager - Extracted from Ultra-Brain for ultra-fast function indexing
//!
//! Core PSI (Program Structure Interface) operations:
//! - SQLite-based persistent storage
//! - Lightning-fast indexing (milisekundy!)
//! - Function extraction with AST analysis (tree-sitter)
//! - Real-time statistics
//! - Pre-compiled regex patterns for maximum performance
//! - IN-MEMORY CACHE for instant hierarchical tree queries

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use anyhow::{Result, Context};
use rusqlite::{Connection, params, OptionalExtension};
use log::{info, debug, warn};
use chrono::Utc;
use crate::ultra_fast_scanner;
use crate::tree_sitter_extractor::{TreeSitterExtractor, SupportedLanguage};
use crate::simple_cfg_analyzer::SimpleCfgAnalyzer;
use crate::regex_patterns::*;
use once_cell::sync::Lazy;
use regex::Regex;
use rayon::prelude::*;
use std::sync::{Arc, Mutex, RwLock};
use std::env;

/// Extraction strategy for function discovery.
///
/// Why: Tree-sitter gives nicer boundaries for some languages, but regex extraction is *much* faster
/// and in many repos is "good enough". We allow switching via env var:
/// - LFT_EXTRACTOR_MODE=regex      -> always regex
/// - LFT_EXTRACTOR_MODE=tree-sitter -> always tree-sitter (fallback to regex on failure)
/// - (default) auto               -> try tree-sitter first, then fallback to regex
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtractorMode {
    Auto,
    RegexOnly,
    TreeSitterFirst,
}

impl ExtractorMode {
    fn from_env() -> Self {
        match env::var("LFT_EXTRACTOR_MODE") {
            Ok(v) => match v.to_lowercase().as_str() {
                "regex" | "regex-only" | "regex_only" => Self::RegexOnly,
                "tree-sitter" | "treesitter" | "ts" => Self::TreeSitterFirst,
                "auto" => Self::Auto,
                _ => Self::Auto,
            },
            Err(_) => Self::Auto,
        }
    }
}

// ==================== GLOBAL IN-MEMORY CACHE ====================
// Cache all functions in RAM for instant access (72K functions = ~50MB = nothing for modern RAM)

/// Global function cache - loaded once, used everywhere
static FUNCTION_CACHE: Lazy<RwLock<FunctionCache>> = Lazy::new(|| {
    RwLock::new(FunctionCache::new())
});

/// In-memory function cache for instant hierarchical tree queries
#[derive(Debug, Clone)]
pub struct FunctionCache {
    /// All functions grouped by repo_id
    pub by_repo: HashMap<String, Vec<FunctionInfo>>,
    /// All functions grouped by file path (for context extraction)
    pub by_file: HashMap<String, Vec<FunctionInfo>>,
    /// File contents cache (read once, use many times)
    pub file_contents: HashMap<String, Vec<String>>,
    /// Cache loaded flag
    pub loaded: bool,
    /// Last load timestamp
    pub loaded_at: i64,
}

impl FunctionCache {
    pub fn new() -> Self {
        Self {
            by_repo: HashMap::new(),
            by_file: HashMap::new(),
            file_contents: HashMap::new(),
            loaded: false,
            loaded_at: 0,
        }
    }

    /// Check if cache needs refresh (older than 5 minutes)
    pub fn needs_refresh(&self) -> bool {
        if !self.loaded {
            return true;
        }
        let now = Utc::now().timestamp();
        now - self.loaded_at > 300 // 5 minutes
    }

    /// Get functions for repo from cache
    pub fn get_functions(&self, repo_id: &str) -> Option<Vec<FunctionInfo>> {
        self.by_repo.get(repo_id).cloned()
    }

    /// Get file content from cache - INSTANT (already in RAM)
    pub fn get_file_content(&self, file_path: &str) -> Option<&Vec<String>> {
        self.file_contents.get(file_path)
    }

    /// Get file content, loading if needed (mutable version)
    pub fn get_or_load_file_content(&mut self, file_path: &str) -> Option<Vec<String>> {
        if let Some(content) = self.file_contents.get(file_path) {
            return Some(content.clone());
        }

        // Try to load file
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
            self.file_contents.insert(file_path.to_string(), lines.clone());
            return Some(lines);
        }

        None
    }

    /// Pre-load ALL source files into RAM for instant context extraction
    /// With 256GB RAM this is trivial - even 1GB of source code is nothing
    pub fn preload_all_source_files(&mut self) -> (usize, usize) {
        let start = std::time::Instant::now();
        let mut files_loaded = 0;
        let mut total_bytes = 0;

        // Get all unique file paths from functions
        let file_paths: Vec<String> = self.by_file.keys().cloned().collect();

        info!("📂 Pre-loading {} source files into RAM...", file_paths.len());

        for file_path in &file_paths {
            if self.file_contents.contains_key(file_path) {
                continue; // Already loaded
            }

            if let Ok(content) = std::fs::read_to_string(file_path) {
                total_bytes += content.len();
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                self.file_contents.insert(file_path.clone(), lines);
                files_loaded += 1;
            }
        }

        let duration = start.elapsed();
        info!("✅ Loaded {} files ({:.2} MB) into RAM in {:.2}ms",
              files_loaded,
              total_bytes as f64 / 1024.0 / 1024.0,
              duration.as_secs_f64() * 1000.0);

        (files_loaded, total_bytes)
    }
}

/// Get file content from global cache - INSTANT access to pre-loaded files
/// This is the primary way to get file contents for context extraction
pub fn get_global_file_content(file_path: &str) -> Option<Vec<String>> {
    // Try global cache first (instant)
    {
        let cache = FUNCTION_CACHE.read().unwrap();
        if let Some(content) = cache.file_contents.get(file_path) {
            return Some(content.clone());
        }
    }

    // Fallback: load from disk and cache (should rarely happen after init)
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        // Add to cache for future use
        {
            let mut cache = FUNCTION_CACHE.write().unwrap();
            cache.file_contents.insert(file_path.to_string(), lines.clone());
        }

        return Some(lines);
    }

    None
}

/// Function information extracted from code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub signature: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub complexity: Option<usize>,
    /// CFG analysis: Has unreachable code after return/panic?
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub has_unreachable_code: Option<bool>,
    /// Number of unreachable code issues found
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unreachable_count: Option<usize>,
}

impl Default for FunctionInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            signature: String::new(),
            file_path: String::new(),
            start_line: 0,
            end_line: 0,
            language: String::new(),
            complexity: None,
            has_unreachable_code: None,
            unreachable_count: None,
        }
    }
}

impl FunctionInfo {
    /// Create new FunctionInfo with CFG fields set to None (for backward compatibility)
    pub fn new_with_cfg(name: String, signature: String, file_path: String, start_line: usize, end_line: usize, language: String, complexity: Option<usize>) -> Self {
        Self {
            name,
            signature,
            file_path,
            start_line,
            end_line,
            language,
            complexity,
            has_unreachable_code: None,
            unreachable_count: None,
        }
    }

    /// Create from fields without CFG (macro-like helper)
    #[allow(clippy::too_many_arguments)]
    pub fn without_cfg(name: String, signature: String, file_path: String, start_line: usize, end_line: usize, language: String, complexity: Option<usize>) -> Self {
        Self::new_with_cfg(name, signature, file_path, start_line, end_line, language, complexity)
    }
}

/// Result of indexing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResult {
    pub repo_id: String,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub files_processed: usize,
    pub duration_ms: u64,
    pub summary: IndexSummary,
}

/// Summary of indexed content
/// Summary of indexed content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSummary {
    pub functions: usize,
    pub classes: usize,
    pub imports: usize,
    pub symbols: usize,
    pub languages: HashMap<String, usize>,
}

/// Project statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub files_tracked: usize,
    pub node_types: HashMap<String, usize>,
    pub edge_types: HashMap<String, usize>,
}

/// PSI Graph Manager - Core functionality extracted from Ultra-Brain
pub struct PsiGraphManager {
    db_path: std::path::PathBuf,
    connection: Connection,
}

impl PsiGraphManager {
    /// Create new PSI graph manager
    pub async fn new() -> Result<Self> {
        let db_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".live-function-tree");

        std::fs::create_dir_all(&db_dir)?;

        let db_path = db_dir.join("function_tree.db");
        let connection = Connection::open(&db_path)?;

        // Windows + multi-process workload: avoid hard failures on SQLITE_BUSY.
        // If another process is writing, we prefer to wait (bounded) instead of instantly failing.
        connection.busy_timeout(std::time::Duration::from_secs(60))?;

        // Enable WAL for better concurrent read/write behavior (still single-writer).
        // If WAL isn't supported in a given environment, we ignore the error and proceed.
        let _ = connection.execute_batch(
            "PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = NORMAL;\n\
             PRAGMA temp_store = MEMORY;\n\
             PRAGMA foreign_keys = ON;\n",
        );

        let manager = Self {
            db_path,
            connection,
        };

        manager.init_database().await?;

        Ok(manager)
    }

    /// Initialize database schema
    async fn init_database(&self) -> Result<()> {
        // Projects table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS projects (
                repo_id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                indexed_at INTEGER NOT NULL,
                file_count INTEGER DEFAULT 0,
                function_count INTEGER DEFAULT 0
            )",
            [],
        )?;

        // Functions table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS functions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                name TEXT NOT NULL,
                signature TEXT,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL,
                complexity INTEGER,
                indexed_at INTEGER NOT NULL,
                file_modified_at INTEGER DEFAULT 0,
                has_unreachable_code INTEGER DEFAULT 0,
                unreachable_count INTEGER DEFAULT 0,
                FOREIGN KEY (repo_id) REFERENCES projects (repo_id)
            )",
            [],
        )?;

        // Add the new columns if they don't exist (for existing databases)
        let _ = self.connection.execute(
            "ALTER TABLE functions ADD COLUMN file_modified_at INTEGER DEFAULT 0",
            [],
        ); // Ignore error if column already exists

        let _ = self.connection.execute(
            "ALTER TABLE functions ADD COLUMN has_unreachable_code INTEGER DEFAULT 0",
            [],
        );

        let _ = self.connection.execute(
            "ALTER TABLE functions ADD COLUMN unreachable_count INTEGER DEFAULT 0",
            [],
        );

        // Create indexes for performance
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_functions_repo_id ON functions (repo_id)",
            [],
        )?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_functions_name ON functions (name)",
            [],
        )?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_functions_file_path ON functions (file_path)",
            [],
        )?;

        info!("📊 PSI database initialized: {:?}", self.db_path);
        Ok(())
    }

    /// 🚀 INCREMENTAL Index project - ultra-fast PSI indexing with path-based updates
    pub async fn index_project_incremental(
        &self,
        project_path: &str,
        repo_id: &str,
        languages: Option<&[&str]>,
        force_full_reindex: bool,
    ) -> Result<IndexResult> {
        let start_time = std::time::Instant::now();
        info!("🔍 Starting INCREMENTAL PSI indexing for: {}", project_path);

        if force_full_reindex {
            info!("🔄 Force full reindex requested - clearing all data");
            // Clear existing data for this repo
            self.connection.execute(
                "DELETE FROM functions WHERE repo_id = ?1",
                params![repo_id],
            )?;
            self.connection.execute(
                "DELETE FROM projects WHERE repo_id = ?1",
                params![repo_id],
            )?;
        }

        // 🚀 ULTRA-FAST file collection with Ultra-Brain optimizations
        let language_strings = languages.map(|langs| langs.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            .unwrap_or_default();

        let files_to_process = ultra_fast_scanner::collect_source_files_parallel(
            project_path,
            None, // No file limit for indexing
            &language_strings
        ).context("Failed to collect source files")?;

        info!("📁 Ultra-fast collection found {} files to analyze", files_to_process.len());

        // 🚀 ULTRA-FAST INCREMENTAL: Batch load ALL file modification times from DB
        let mut existing_times: HashMap<String, i64> = HashMap::new();
        if !force_full_reindex {
            let mut stmt = self.connection.prepare(
                "SELECT DISTINCT file_path, file_modified_at FROM functions WHERE repo_id = ?1"
            )?;
            let rows = stmt.query_map([repo_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;

            for row in rows {
                if let Ok((path, modified)) = row {
                    existing_times.insert(path, modified);
                }
            }
            info!("📊 Loaded {} file timestamps from DB in bulk", existing_times.len());
        }

        // 🎯 INCREMENTAL: TRUE PARALLEL check - ZERO MUTEX CONTENTION!
        info!("⚡ PARALLEL file metadata check starting (lock-free)...");

        // ⚡ LOCK-FREE: Each thread collects locally, then merge at end (ULTRA FAST!)
        let files_to_update: Vec<(String, String, i64)> = files_to_process
            .par_iter()
            .filter_map(|file_path| {
                let file_path_str = file_path.to_string_lossy().to_string();
                let file_metadata = std::fs::metadata(file_path).ok()?;
                let modified_time = file_metadata.modified().ok()?;
                let duration = modified_time.duration_since(std::time::UNIX_EPOCH).ok()?;
                let file_modified = duration.as_secs() as i64;

                let relative_path = file_path
                    .strip_prefix(project_path)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .to_string();

                // Check if file needs updating (ZERO DB QUERIES, ZERO LOCKS!)
                let needs_update = if force_full_reindex {
                    true
                } else {
                    match existing_times.get(&relative_path) {
                        Some(&stored_time) => file_modified > stored_time,
                        None => true, // File not in database
                    }
                };

                if needs_update {
                    Some((file_path_str, relative_path, file_modified))
                } else {
                    None
                }
            })
            .collect();

        let updated_files_count = files_to_update.len();
        let skipped_files_count = files_to_process.len() - updated_files_count;        info!("🎯 INCREMENTAL analysis: {} files to update, {} files up-to-date (skipped)",
              updated_files_count, skipped_files_count);

        if files_to_update.is_empty() {
            info!("✅ No files need updating - repository is up to date");

            // Get current stats
            let current_functions: usize = self.connection.query_row(
                "SELECT COUNT(*) FROM functions WHERE repo_id = ?1",
                [repo_id],
                |row| Ok(row.get::<_, i64>(0)? as usize)
            ).unwrap_or(0);

            return Ok(IndexResult {
                repo_id: repo_id.to_string(),
                total_nodes: current_functions,
                total_edges: 0,
                files_processed: files_to_process.len(),
                duration_ms: start_time.elapsed().as_millis() as u64,
                summary: IndexSummary {
                    functions: current_functions,
                    classes: 0,
                    imports: 0,
                    symbols: current_functions,
                    languages: HashMap::new(),
                },
            });
        }

        // 🗑️ DELETE functions from files that will be updated
        for (_, relative_path, _) in &files_to_update {
            let deleted: usize = self.connection.execute(
                "DELETE FROM functions WHERE repo_id = ?1 AND file_path = ?2",
                params![repo_id, relative_path],
            )? as usize;
            if deleted > 0 {
                debug!("🗑️ Deleted {} functions from: {}", deleted, relative_path);
            }
        }

        // 🚀 ULTRA-FAST parallel function extraction for updated files only (LOCK-FREE!)
        info!("⚡ Starting PARALLEL function extraction for {} updated files...", files_to_update.len());

        let all_functions: Vec<FunctionInfo> = files_to_update
            .par_iter()
            .enumerate()
            .filter_map(|(idx, (file_path, relative_path, file_modified))| {
                // Progress tracking (lock-free - only logs every 20 files)
                if idx > 0 && idx % 20 == 0 {
                    info!("📊 Processed {}/{} updated files...", idx, files_to_update.len());
                }

                match Self::extract_functions_from_file_static(std::path::Path::new(file_path), project_path) {
                    Ok(mut functions) => {
                        // Add file modification time to each function
                        for func in &mut functions {
                            // We'll store file_modified in the signature temporarily, then extract it in batch_insert
                            func.signature = format!("{}|MODIFIED:{}", func.signature, file_modified);
                        }

                        if !functions.is_empty() {
                            Some(functions)
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        debug!("Failed to extract from {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .flatten()
            .collect();
        info!("✨ Incremental extraction completed - found {} functions in updated files", all_functions.len());

        // Update or insert project record
        let indexed_at = Utc::now().timestamp();
        let _ = self.connection.execute(
            "INSERT OR REPLACE INTO projects (repo_id, project_path, indexed_at, file_count, function_count)
             VALUES (?1, ?2, ?3, ?4, (SELECT COUNT(*) FROM functions WHERE repo_id = ?1) + ?5)",
            params![repo_id, project_path, indexed_at, files_to_process.len(), all_functions.len()],
        )?;

        // 🚀 BATCH INSERT with file modification times
        info!("💾 Starting BATCH INSERT of {} functions...", all_functions.len());
        self.batch_insert_functions_with_modified_time(repo_id, &all_functions)?;

        // Get final stats
        let final_function_count: usize = self.connection.query_row(
            "SELECT COUNT(*) FROM functions WHERE repo_id = ?1",
            [repo_id],
            |row| Ok(row.get::<_, i64>(0)? as usize)
        )?;

        info!("✅ Incremental indexing: {} files processed ({} updated, {} skipped), {} total functions",
              files_to_process.len(), updated_files_count, skipped_files_count, final_function_count);

        // Build language statistics
        let mut languages_count = HashMap::new();
        for func in &all_functions {
            // Extract original signature (remove the MODIFIED: part)
            let original_signature = func.signature.split("|MODIFIED:").next().unwrap_or(&func.signature);
            *languages_count.entry(func.language.clone()).or_insert(0) += 1;
        }

        Ok(IndexResult {
            repo_id: repo_id.to_string(),
            total_nodes: final_function_count,
            total_edges: 0,
            files_processed: updated_files_count, // Only count actually processed files
            duration_ms: start_time.elapsed().as_millis() as u64,
            summary: IndexSummary {
                functions: final_function_count,
                classes: 0,
                imports: 0,
                symbols: final_function_count,
                languages: languages_count,
            },
        })
    }

    /// Index project - ultra-fast PSI indexing
    pub async fn index_project(
        &self,
        project_path: &str,
        repo_id: &str,
        languages: Option<&[&str]>,
    ) -> Result<IndexResult> {
        let start_time = std::time::Instant::now();
        info!("🔍 Starting PSI indexing for: {}", project_path);

        // Clear existing data for this repo
        self.connection.execute(
            "DELETE FROM functions WHERE repo_id = ?1",
            params![repo_id],
        )?;
        self.connection.execute(
            "DELETE FROM projects WHERE repo_id = ?1",
            params![repo_id],
        )?;

        // 🚀 ULTRA-FAST file collection with Ultra-Brain optimizations
        let language_strings = languages.map(|langs| langs.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            .unwrap_or_default();

        let files_to_process = ultra_fast_scanner::collect_source_files_parallel(
            project_path,
            None, // No file limit for indexing
            &language_strings
        ).context("Failed to collect source files")?;

        let files_processed = files_to_process.len();
        info!("📁 Ultra-fast collection found {} files to analyze", files_processed);

        // 🚀 ULTRA-FAST parallel function extraction like Ultra-Brain (LOCK-FREE!)
        info!("⚡ Starting PARALLEL function extraction with Rayon...");

        let all_functions: Vec<FunctionInfo> = files_to_process
            .par_iter()
            .enumerate()
            .filter_map(|(idx, file_path)| {
                // Progress tracking (lock-free - only logs every 50 files)
                if idx > 0 && idx % 50 == 0 {
                    info!("📊 Processed {}/{} files...", idx, files_to_process.len());
                }

                match Self::extract_functions_from_file_static(file_path, project_path) {
                    Ok(functions) => {
                        if !functions.is_empty() {
                            Some(functions)
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        debug!("Failed to extract from {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .flatten()
            .collect();
        info!("✨ Parallel extraction completed - found {} functions", all_functions.len());

        // Insert project record
        let indexed_at = Utc::now().timestamp();
        let normalized_project_path = Self::normalize_project_path(project_path);
        self.connection.execute(
            "INSERT INTO projects (repo_id, project_path, indexed_at, file_count, function_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![repo_id, normalized_project_path, indexed_at, files_processed, all_functions.len()],
        )?;

        // 🚀 BATCH INSERT for ultra-fast SQLite performance
        info!("💾 Starting BATCH INSERT of {} functions...", all_functions.len());
        self.batch_insert_functions(repo_id, &all_functions)?;

        info!("✅ Indexed project: {} files processed, {} functions", files_processed, all_functions.len());

        // Build language statistics
        let mut languages_count = HashMap::new();
        for func in &all_functions {
            *languages_count.entry(func.language.clone()).or_insert(0) += 1;
        }

        Ok(IndexResult {
            repo_id: repo_id.to_string(),
            total_nodes: all_functions.len(),
            total_edges: 0,
            files_processed,
            duration_ms: start_time.elapsed().as_millis() as u64,
            summary: IndexSummary {
                functions: all_functions.len(),
                classes: 0,
                imports: 0,
                symbols: all_functions.len(),
                languages: languages_count,
            },
        })
    }

    /// (opcjonalne) Rozgrzewanie cache po indeksowaniu.
    ///
    /// Domyślnie NIE robimy preloadu wszystkich plików do RAM, bo to potrafi zabić „ultra-fast”
    /// na Windows (dużo I/O + AV + cold cache FS). Jeśli ktoś chce mieć super-instant context
    /// na naprawdę dużych projektach, może to włączyć envem.
    pub async fn warm_cache_if_enabled(&self) -> Result<()> {
        let enable_preload = std::env::var("LFT_PRELOAD_FILES")
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        if enable_preload {
            let _ = self.load_into_memory_cache().await?;
        } else {
            // Minimum: oznacz cache jako nieaktualny, żeby tree wiedziało, że ma dociągnąć z DB.
            self.invalidate_cache();
        }

        Ok(())
    }

    /// Store extracted functions in PSI database (NEW - respects max_files limit)
    pub async fn store_extracted_functions(
        &self,
        project_path: &str,
        repo_id: &str,
        functions: &[FunctionInfo],
        files_processed: usize,
    ) -> Result<IndexResult> {
        let start_time = std::time::Instant::now();
        info!("💾 Storing {} functions to PSI database for repo: {}", functions.len(), repo_id);

        // Clear existing data for this repo
        self.connection.execute(
            "DELETE FROM functions WHERE repo_id = ?1",
            params![repo_id],
        )?;
        self.connection.execute(
            "DELETE FROM projects WHERE repo_id = ?1",
            params![repo_id],
        )?;

        // Insert project record
        let indexed_at = Utc::now().timestamp();
        let normalized_project_path = Self::normalize_project_path(project_path);
        self.connection.execute(
            "INSERT INTO projects (repo_id, project_path, indexed_at, file_count, function_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![repo_id, normalized_project_path, indexed_at, files_processed, functions.len()],
        )?;

        // Insert all functions
        for function in functions {
            self.connection.execute(
                "INSERT INTO functions (repo_id, name, signature, file_path, start_line, end_line, language, complexity, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    repo_id,
                    function.name,
                    function.signature,
                    function.file_path,
                    function.start_line,
                    function.end_line,
                    function.language,
                    function.complexity.unwrap_or(1),
                    indexed_at
                ],
            )?;
        }

        let duration = start_time.elapsed();
        info!("✅ Successfully stored {} functions in {}ms", functions.len(), duration.as_millis());

        // Build language statistics
        let mut languages_count = HashMap::new();
        for func in functions {
            *languages_count.entry(func.language.clone()).or_insert(0) += 1;
        }

        Ok(IndexResult {
            repo_id: repo_id.to_string(),
            total_nodes: functions.len(),
            total_edges: 0, // We don't calculate edges yet
            files_processed,
            duration_ms: duration.as_millis() as u64,
            summary: IndexSummary {
                functions: functions.len(),
                classes: 0,
                imports: 0,
                symbols: functions.len(),
                languages: languages_count,
            },
        })
    }

    pub async fn query_functions(
        &self,
        repo_id: &str,
        query_type: &str,
        symbol: Option<&str>,
        _include_context: bool,
        max_results: usize,
    ) -> Result<Vec<serde_json::Value>> {
        let mut sql = "SELECT name, signature, file_path, start_line, end_line, language, complexity
                       FROM functions WHERE repo_id = ?1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];

        // Add filters based on query type and symbol
        match query_type {
            "by_name" if symbol.is_some() => {
                sql.push_str(" AND name LIKE ?2");
                params.push(Box::new(format!("%{}%", symbol.unwrap())));
            }
            "all_functions" => {
                // No additional filter
            }
            _ => {
                // For other query types, we'd need more sophisticated analysis
                // For now, treat as all_functions
            }
        }

        sql.push_str(" ORDER BY complexity DESC LIMIT ?");
        let param_count = params.len() + 1;
        params.push(Box::new(max_results));

        let mut stmt = self.connection.prepare(&sql)?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let _param_count = params.len() + 1;
        let rows = stmt.query_map(&param_refs[..], |row| {
            Ok(serde_json::json!({
                "name": row.get::<_, String>(0)?,
                "signature": row.get::<_, Option<String>>(1)?,
                "file": row.get::<_, String>(2)?,
                "startLine": row.get::<_, i64>(3)?,
                "endLine": row.get::<_, i64>(4)?,
                "language": row.get::<_, String>(5)?,
                "complexity": row.get::<_, Option<i64>>(6)?,
            }))
        })?;

        let results: Vec<serde_json::Value> = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Load ALL functions into memory cache - call once, query instantly forever
    pub async fn load_into_memory_cache(&self) -> Result<usize> {
        let start = std::time::Instant::now();

        // Check if already loaded
        {
            let cache = FUNCTION_CACHE.read().unwrap();
            if !cache.needs_refresh() {
                let total: usize = cache.by_repo.values().map(|v| v.len()).sum();
                info!("📦 Cache already loaded: {} functions", total);
                return Ok(total);
            }
        }

        info!("🚀 Loading ALL functions into memory cache...");

        // Query ALL functions from database
        let mut stmt = self.connection.prepare(
            "SELECT repo_id, name, signature, file_path, start_line, end_line, language, complexity, has_unreachable_code, unreachable_count
             FROM functions ORDER BY repo_id, file_path, start_line"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // repo_id
                FunctionInfo {
                    name: row.get(1)?,
                    signature: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    file_path: row.get(3)?,
                    start_line: row.get::<_, i64>(4)? as usize,
                    end_line: row.get::<_, i64>(5)? as usize,
                    language: row.get(6)?,
                    complexity: row.get::<_, Option<i64>>(7)?.map(|c| c as usize),
                    has_unreachable_code: row.get::<_, Option<i32>>(8)?.map(|v| v != 0),
                    unreachable_count: row.get::<_, Option<i32>>(9)?.map(|v| v as usize),
                }
            ))
        })?;

        let mut by_repo: HashMap<String, Vec<FunctionInfo>> = HashMap::new();
        let mut by_file: HashMap<String, Vec<FunctionInfo>> = HashMap::new();
        let mut total = 0;

        for row in rows {
            let (repo_id, func) = row?;

            // Group by repo
            by_repo.entry(repo_id).or_default().push(func.clone());

            // Group by file
            by_file.entry(func.file_path.clone()).or_default().push(func);

            total += 1;
        }

        // Update global cache with functions
        {
            let mut cache = FUNCTION_CACHE.write().unwrap();
            cache.by_repo = by_repo;
            cache.by_file = by_file;
            cache.loaded = true;
            cache.loaded_at = Utc::now().timestamp();

            // Pre-load ALL source files into RAM for instant context extraction
            // With 256GB RAM this is trivial!
            let (files_loaded, bytes_loaded) = cache.preload_all_source_files();
            info!("📦 Total cache: {} functions + {} files ({:.2} MB)",
                  total, files_loaded, bytes_loaded as f64 / 1024.0 / 1024.0);
        }

        let duration = start.elapsed();
        info!("✅ Full cache loaded in {:.2}ms", duration.as_secs_f64() * 1000.0);

        Ok(total)
    }

    /// Get all functions for a repository - FROM MEMORY CACHE (instant!)
    pub async fn get_all_functions(&self, repo_id: &str) -> Result<Vec<FunctionInfo>> {
        // Try cache first
        {
            let cache = FUNCTION_CACHE.read().unwrap();
            if cache.loaded {
                if let Some(functions) = cache.get_functions(repo_id) {
                    debug!("⚡ Cache hit: {} functions for repo '{}'", functions.len(), repo_id);
                    return Ok(functions);
                }
            }
        }

        // Cache miss or not loaded - load from DB and update cache
        info!("📥 Cache miss for '{}', loading from database...", repo_id);

        let mut stmt = self.connection.prepare(
            "SELECT name, signature, file_path, start_line, end_line, language, complexity, has_unreachable_code, unreachable_count
             FROM functions WHERE repo_id = ?1 ORDER BY file_path, start_line"
        )?;

        let rows = stmt.query_map([repo_id], |row| {
            Ok(FunctionInfo {
                name: row.get(0)?,
                signature: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                file_path: row.get(2)?,
                start_line: row.get::<_, i64>(3)? as usize,
                end_line: row.get::<_, i64>(4)? as usize,
                language: row.get(5)?,
                complexity: row.get::<_, Option<i64>>(6)?.map(|c| c as usize),
                has_unreachable_code: row.get::<_, Option<i32>>(7)?.map(|v| v != 0),
                unreachable_count: row.get::<_, Option<i32>>(8)?.map(|v| v as usize),
            })
        })?;

        let functions: Vec<FunctionInfo> = rows.collect::<Result<Vec<_>, _>>()?;

        // Update cache with this repo's functions
        {
            let mut cache = FUNCTION_CACHE.write().unwrap();
            cache.by_repo.insert(repo_id.to_string(), functions.clone());
            for func in &functions {
                cache.by_file.entry(func.file_path.clone()).or_default().push(func.clone());
            }
        }

        Ok(functions)
    }

    /// Get file content from cache (for context extraction)
    pub fn get_cached_file_content(&self, file_path: &str) -> Option<Vec<String>> {
        let cache = FUNCTION_CACHE.read().unwrap();
        cache.get_file_content(file_path).map(|v| v.clone())
    }

    /// Invalidate cache (call after indexing)
    pub fn invalidate_cache(&self) {
        let mut cache = FUNCTION_CACHE.write().unwrap();
        cache.loaded = false;
        cache.by_repo.clear();
        cache.by_file.clear();
        cache.file_contents.clear();
        info!("🗑️ Memory cache invalidated");
    }

    /// Get project path for repository
    pub async fn get_project_path(&self, repo_id: &str) -> Result<Option<String>> {
        let result: Result<String, _> = self.connection.query_row(
            "SELECT project_path FROM projects WHERE repo_id = ?1",
            [repo_id],
            |row| row.get(0)
        );

        match result {
            Ok(path) => Ok(Some(path)),
            Err(_) => Ok(None), // Repository not found
        }
    }

    /// Find repo_id by project path
    pub async fn find_repo_id_by_project_path(&self, project_path: &str) -> Result<Option<String>> {
        // Normalize path for consistent lookup
        let normalized_path = Self::normalize_project_path(project_path);

        // Fast path: exact match on stored normalized form
        let result: Result<String, _> = self.connection.query_row(
            "SELECT repo_id FROM projects WHERE project_path = ?1 ORDER BY indexed_at DESC LIMIT 1",
            [&normalized_path],
            |row| row.get(0)
        );

        if let Ok(repo_id) = result {
            return Ok(Some(repo_id));
        }

        // Windows reality check: same path can differ only by drive-letter case.
        // SQLite string comparisons are case-sensitive by default, so we add a fallback.
        let result_ci: Result<String, _> = self.connection.query_row(
            "SELECT repo_id FROM projects WHERE lower(project_path) = lower(?1) ORDER BY indexed_at DESC LIMIT 1",
            [&normalized_path],
            |row| row.get(0)
        );

        match result_ci {
            Ok(repo_id) => Ok(Some(repo_id)),
            Err(_) => Ok(None), // Project path not found
        }
    }

    /// Normalize project path - convert backslashes to forward slashes, remove trailing slashes
    pub fn normalize_project_path(path: &str) -> String {
        let mut p = path.replace('\\', "/").trim_end_matches('/').to_string();
        // On Windows, normalize drive letter & casing to avoid duplicate repo entries.
        // We only do this on Windows to not break Linux/macOS case-sensitive paths.
        if cfg!(windows) {
            p = p.to_lowercase();
        }
        p
    }

    /// Generate repo_id from project path - uses normalized full path hash for uniqueness
    pub fn generate_repo_id_from_path(project_path: &str) -> String {
        let normalized = Self::normalize_project_path(project_path);

        // Use path hash + folder name for unique but readable repo_id
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        let path_hash = hasher.finish();

        let folder_name = std::path::Path::new(&normalized)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default");

        // Format: foldername-hash (last 8 chars of hash for brevity)
        format!("{}-{:08x}", folder_name, path_hash & 0xFFFFFFFF)
    }

    /// Get or create repo_id for project path
    /// Returns existing repo_id if project is already indexed, otherwise generates new one
    pub async fn get_or_create_repo_id(&self, project_path: &str) -> Result<String> {
        // First check if project is already indexed
        if let Some(existing_id) = self.find_repo_id_by_project_path(project_path).await? {
            return Ok(existing_id);
        }

        // Generate new repo_id from path
        Ok(Self::generate_repo_id_from_path(project_path))
    }

    /// Store functions in database (alternative interface)
    pub async fn store_functions(&self, repo_id: &str, functions: &[FunctionInfo]) -> Result<()> {
        let indexed_at = Utc::now().timestamp();

        // Insert functions
        let mut stmt = self.connection.prepare(
            "INSERT INTO functions (repo_id, name, signature, file_path, start_line, end_line, language, complexity, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        )?;

        for func in functions {
            stmt.execute(params![
                repo_id,
                func.name,
                func.signature,
                func.file_path,
                func.start_line,
                func.end_line,
                func.language,
                func.complexity,
                indexed_at
            ])?;
        }

        Ok(())
    }

    /// Get repository statistics (alias for get_project_stats)
    pub async fn get_repository_stats(&self, repo_id: &str) -> Result<ProjectStats> {
        self.get_project_stats(repo_id).await
    }

    /// Get project statistics
    pub async fn get_project_stats(&self, repo_id: &str) -> Result<ProjectStats> {
        // Get basic counts
        let function_count: usize = self.connection.query_row(
            "SELECT COUNT(*) FROM functions WHERE repo_id = ?1",
            [repo_id],
            |row| Ok(row.get::<_, i64>(0)? as usize)
        )?;

        let file_count: usize = self.connection.query_row(
            "SELECT COUNT(DISTINCT file_path) FROM functions WHERE repo_id = ?1",
            [repo_id],
            |row| Ok(row.get::<_, i64>(0)? as usize)
        )?;

        // Get language breakdown
        let mut stmt = self.connection.prepare(
            "SELECT language, COUNT(*) FROM functions WHERE repo_id = ?1 GROUP BY language"
        )?;

        let rows = stmt.query_map([repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;

        let mut node_types = HashMap::new();
        node_types.insert("function".to_string(), function_count);

        for row in rows {
            let (language, count) = row?;
            node_types.insert(language, count);
        }

        Ok(ProjectStats {
            total_nodes: function_count,
            total_edges: 0, // TODO: Add relationship tracking
            files_tracked: file_count,
            node_types,
            edge_types: HashMap::new(),
        })
    }

    /// Extract functions from a single file (instance method)
    fn extract_functions_from_file(&self, file_path: &Path, project_root: &str) -> Result<Vec<FunctionInfo>> {
        Self::extract_functions_from_file_static(file_path, project_root)
    }

    /// Extract functions from a single file using tree-sitter AST parsing
    /// Falls back to regex for unsupported languages
    fn extract_functions_from_file_static(file_path: &Path, project_root: &str) -> Result<Vec<FunctionInfo>> {
        let content = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;

        // Perf knob: allow forcing regex-only extraction (fast) or tree-sitter-first via env.
        // Default keeps existing behavior (auto = tree-sitter first, regex fallback).
        let mode = ExtractorMode::from_env();

        let extension = file_path.extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("No file extension"))?;

        let relative_path = file_path.strip_prefix(project_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Try tree-sitter extraction first for supported languages (unless regex-only)
        if mode != ExtractorMode::RegexOnly {
            if let Some(lang) = SupportedLanguage::from_extension(extension) {
                match Self::extract_with_tree_sitter(&content, &relative_path, lang) {
                    Ok(functions) if !functions.is_empty() => return Ok(functions),
                    _ => {} // Fall through to regex
                }
            }
        }

        // Fallback to regex extraction
        match extension.to_lowercase().as_str() {
            "py" | "pyw" | "pyi" => Self::extract_python_functions_static(&content, &relative_path),
            "rs" => Self::extract_rust_functions_static(&content, &relative_path),
            "js" | "jsx" | "mjs" => Self::extract_js_functions_static(&content, &relative_path, "javascript"),
            "ts" | "tsx" | "mts" => Self::extract_js_functions_static(&content, &relative_path, "typescript"),
            "java" => Self::extract_java_functions_static(&content, &relative_path),
            "go" => Self::extract_go_functions_static(&content, &relative_path),
            "c" | "h" => Self::extract_c_functions_static(&content, &relative_path),
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" => Self::extract_cpp_functions_static(&content, &relative_path),
            "dart" => Self::extract_js_functions_static(&content, &relative_path, "dart"),
            "kt" | "kts" => Self::extract_kotlin_functions_static(&content, &relative_path),
            "swift" => Self::extract_swift_functions_static(&content, &relative_path),
            "php" | "phtml" => Self::extract_php_functions_static(&content, &relative_path),
            "rb" | "rake" | "gemspec" => Self::extract_ruby_functions_static(&content, &relative_path),
            "cs" => Self::extract_csharp_functions_static(&content, &relative_path),
            "scala" | "sc" => Self::extract_scala_functions_static(&content, &relative_path),
            "sh" | "bash" | "zsh" => Self::extract_bash_functions_static(&content, &relative_path),
            // Web & config files (regex-only extraction)
            "html" | "htm" | "xhtml" | "vue" | "svelte" => Self::extract_html_functions_static(&content, &relative_path),
            "json" | "jsonc" => Self::extract_json_functions_static(&content, &relative_path),
            _ => Ok(Vec::new()),
        }
    }

    /// Extract functions using tree-sitter AST parsing
    fn extract_with_tree_sitter(content: &str, file_path: &str, language: SupportedLanguage) -> Result<Vec<FunctionInfo>> {
        // Use thread-local storage for parser reuse
        thread_local! {
            static EXTRACTOR: std::cell::RefCell<Option<TreeSitterExtractor>> = std::cell::RefCell::new(None);
        }

        EXTRACTOR.with(|ext| {
            let mut ext_ref = ext.borrow_mut();
            if ext_ref.is_none() {
                *ext_ref = TreeSitterExtractor::new().ok();
            }

            if let Some(extractor) = ext_ref.as_mut() {
                let ast_functions = extractor.extract_functions(content, file_path, language)?;

                // Run CFG analysis during indexing
                let cfg_analyzer = SimpleCfgAnalyzer::new();

                // Convert ASTFunction to FunctionInfo with CFG analysis
                let functions = ast_functions.into_iter().map(|f| {
                    // Run CFG analysis for this function
                    let (has_unreachable_code, unreachable_count) = match cfg_analyzer.analyze_function(
                        &f.name,
                        file_path,
                        content,
                        f.start_line,
                        f.end_line
                    ) {
                        Ok(result) => {
                            let has_code = result.has_unreachable_code;
                            let count = result.unreachable_issues.len();
                            (Some(has_code), if count > 0 { Some(count) } else { None })
                        },
                        Err(_) => (None, None) // CFG analysis failed, skip
                    };

                    FunctionInfo {
                        name: f.name,
                        signature: f.signature,
                        file_path: f.file_path,
                        start_line: f.start_line,
                        end_line: f.end_line,
                        language: f.language,
                        complexity: Some(f.complexity),
                        has_unreachable_code,
                        unreachable_count,
                        ..Default::default()
                    }
                }).collect();

                Ok(functions)
            } else {
                anyhow::bail!("Failed to initialize tree-sitter extractor")
            }
        })
    }

    /// Extract Java functions using regex
    fn extract_java_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex
            if let Some(captures) = JAVA_METHOD.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = captures.get(2).unwrap().as_str();

                // Skip if it looks like a class declaration or control statement
                if trimmed.starts_with("class ") || trimmed.starts_with("interface ") ||
                   trimmed.starts_with("if ") || trimmed.starts_with("while ") ||
                   trimmed.starts_with("for ") || trimmed.starts_with("switch ") {
                    continue;
                }

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("{} {}({})", "method", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "java".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract Go functions using regex
    fn extract_go_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex
            if let Some(captures) = GO_FUNC.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = captures.get(2).unwrap().as_str();

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("func {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "go".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract C functions using regex
    fn extract_c_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Match C function definitions (type name(...))
            // Skip preprocessor directives, struct/enum definitions, and control statements
            if trimmed.starts_with("#") || trimmed.starts_with("//") ||
               trimmed.starts_with("typedef") || trimmed.starts_with("struct") ||
               trimmed.starts_with("enum") || trimmed.starts_with("if") ||
               trimmed.starts_with("while") || trimmed.starts_with("for") {
                continue;
            }

            // Use pre-compiled regex
            if let Some(captures) = C_FUNC.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = captures.get(2).unwrap().as_str();

                // Skip common non-function patterns
                if func_name == "if" || func_name == "while" || func_name == "for" ||
                   func_name == "switch" || func_name == "sizeof" || func_name == "return" {
                    continue;
                }

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("{} {}({})", "func", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "c".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract C++ functions using regex (extends C extraction)
    fn extract_cpp_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Skip preprocessor directives, comments, and class/struct declarations
            if trimmed.starts_with("#") || trimmed.starts_with("//") ||
               trimmed.starts_with("class ") || trimmed.starts_with("struct ") ||
               trimmed.starts_with("template") || trimmed.starts_with("namespace") {
                continue;
            }

            // Use pre-compiled regex for C++
            if let Some(captures) = CPP_METHOD.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = captures.get(2).unwrap().as_str();

                // Skip common non-function patterns
                if func_name == "if" || func_name == "while" || func_name == "for" ||
                   func_name == "switch" || func_name == "sizeof" || func_name == "return" ||
                   func_name == "new" || func_name == "delete" {
                    continue;
                }

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("{} {}({})", "func", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "cpp".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract Kotlin functions using regex
    fn extract_kotlin_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for Kotlin functions
            if let Some(captures) = KOTLIN_FUNC.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = "";

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("fun {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "kotlin".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for Kotlin classes
            if let Some(captures) = KOTLIN_CLASS.captures(trimmed) {
                let class_name = captures.get(1).unwrap().as_str().to_string();

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("class {}", class_name);

                functions.push(FunctionInfo {
                    name: class_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "kotlin".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract Swift functions using regex
    fn extract_swift_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for Swift functions
            if let Some(captures) = SWIFT_FUNC.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = "";

                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("func {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "swift".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for Swift classes/structs
            if let Some(captures) = SWIFT_CLASS.captures(trimmed) {
                let type_name = captures.get(1).unwrap().as_str().to_string();

                let end_line = Self::find_brace_end(&lines, i);
                let kind = if trimmed.contains("class ") {
                    "class"
                } else if trimmed.contains("struct ") {
                    "struct"
                } else if trimmed.contains("protocol ") {
                    "protocol"
                } else {
                    "enum"
                };
                let signature = format!("{} {}", kind, type_name);

                functions.push(FunctionInfo {
                    name: type_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "swift".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract PHP functions using regex
    fn extract_php_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for PHP functions
            if let Some(captures) = PHP_FUNCTION.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = "";
                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("function {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "php".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for PHP classes
            if let Some(captures) = PHP_CLASS.captures(trimmed) {
                let class_name = captures.get(1).unwrap().as_str().to_string();
                let end_line = Self::find_brace_end(&lines, i);

                functions.push(FunctionInfo {
                    name: class_name.clone(),
                    signature: format!("class {}", class_name),
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "php".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }
        Ok(functions)
    }

    /// Extract Ruby functions using regex
    fn extract_ruby_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for Ruby methods
            if let Some(captures) = RUBY_DEF.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                let end_line = Self::find_end_keyword(&lines, i);
                let signature = format!("def {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "ruby".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for Ruby classes/modules
            if let Some(captures) = RUBY_CLASS.captures(trimmed) {
                let class_name = captures.get(1).unwrap().as_str().to_string();
                let end_line = Self::find_end_keyword(&lines, i);
                let kind = if trimmed.starts_with("class") { "class" } else { "module" };

                functions.push(FunctionInfo {
                    name: class_name.clone(),
                    signature: format!("{} {}", kind, class_name),
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "ruby".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }
        Ok(functions)
    }

    /// Extract C# functions using regex
    fn extract_csharp_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for C# methods
            if let Some(captures) = CSHARP_METHOD.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                if func_name == "if" || func_name == "while" || func_name == "for" || func_name == "switch" {
                    continue;
                }
                let params = "";
                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("{}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "csharp".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for C# classes
            if let Some(captures) = CSHARP_CLASS.captures(trimmed) {
                let type_name = captures.get(1).unwrap().as_str().to_string();
                let end_line = Self::find_brace_end(&lines, i);
                let kind = if trimmed.contains("interface ") { "interface" }
                    else if trimmed.contains("struct ") { "struct" }
                    else if trimmed.contains("enum ") { "enum" }
                    else { "class" };

                functions.push(FunctionInfo {
                    name: type_name.clone(),
                    signature: format!("{} {}", kind, type_name),
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "csharp".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }
        Ok(functions)
    }

    /// Extract Scala functions using regex
    fn extract_scala_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for Scala defs
            if let Some(captures) = SCALA_DEF.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = "";
                let end_line = Self::find_scala_block_end(&lines, i);
                let signature = format!("def {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "scala".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for Scala classes
            if let Some(captures) = SCALA_CLASS.captures(trimmed) {
                let type_name = captures.get(1).unwrap().as_str().to_string();
                let end_line = Self::find_brace_end(&lines, i);
                let kind = if trimmed.contains("object ") { "object" }
                    else if trimmed.contains("trait ") { "trait" }
                    else { "class" };

                functions.push(FunctionInfo {
                    name: type_name.clone(),
                    signature: format!("{} {}", kind, type_name),
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "scala".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }
        Ok(functions)
    }

    /// Extract Bash functions using regex
    fn extract_bash_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for Bash functions
            if let Some(captures) = BASH_FUNC.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                // Skip common keywords that look like functions
                if func_name == "if" || func_name == "while" || func_name == "for" || func_name == "case" {
                    continue;
                }
                let end_line = Self::find_brace_end(&lines, i);
                let signature = format!("function {}", func_name);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "bash".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }
        Ok(functions)
    }

    /// Extract HTML elements that trigger JavaScript (onclick, onsubmit, etc.)
    fn extract_html_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for script tags
            if let Some(captures) = HTML_SCRIPT.captures(trimmed) {
                let name = captures.get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "inline_script".to_string());
                let signature = format!("<script src=\"{}\">", name);
                functions.push(FunctionInfo {
                    name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    language: "html".to_string(),
                    complexity: Some(1),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }

            // Use pre-compiled regex for event handlers
            if let Some(captures) = HTML_EVENT.captures(trimmed) {
                let event = captures.get(1).map(|m| m.as_str()).unwrap_or("onclick");
                let handler = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                let signature = format!("{}=\"{}\"", event, handler);
                functions.push(FunctionInfo {
                    name: format!("{}:{}", event, handler.split('(').next().unwrap_or(handler)),
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    language: "html".to_string(),
                    complexity: Some(1),
                ..Default::default()
                });
            }

            // Use pre-compiled regex for form actions
            if let Some(captures) = HTML_FORM.captures(trimmed) {
                let action = captures.get(1).map(|m| m.as_str()).unwrap_or("/");
                let signature = format!("<form action=\"{}\">", action);
                functions.push(FunctionInfo {
                    name: format!("form:{}", action),
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    language: "html".to_string(),
                    complexity: Some(1),
                ..Default::default()
                });
            }

            // Use pre-compiled regex for JS links
            if let Some(captures) = HTML_JS_LINK.captures(trimmed) {
                let js_code = captures.get(1).map(|m| m.as_str()).unwrap_or("javascript:void(0)");
                let signature = format!("<a href=\"{}\">", js_code);
                functions.push(FunctionInfo {
                    name: format!("link:{}", js_code.replace("javascript:", "")),
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    language: "html".to_string(),
                    complexity: Some(1),
                ..Default::default()
                });
            }
        }
        Ok(functions)
    }

    /// Extract JSON/YAML configuration keys (especially API endpoints, scripts)
    fn extract_json_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for script/command keys
            if let Some(captures) = JSON_SCRIPT.captures(trimmed) {
                let key = captures.get(1).map(|m| m.as_str()).unwrap_or("key");
                let value = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                let signature = format!("\"{}\": \"{}\"", key, value);
                functions.push(FunctionInfo {
                    name: format!("{}:{}", key, value.split_whitespace().next().unwrap_or(value)),
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    language: "json".to_string(),
                    complexity: Some(1),
                ..Default::default()
                });
            }
        }
        Ok(functions)
    }

    /// Helper to find matching 'end' keyword (Ruby-style)
    fn find_end_keyword(lines: &[&str], start: usize) -> usize {
        let mut depth = 1;
        for (j, next_line) in lines.iter().enumerate().skip(start + 1) {
            let trimmed = next_line.trim();
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") ||
               trimmed.starts_with("module ") || trimmed.starts_with("do") ||
               trimmed.starts_with("if ") || trimmed.starts_with("unless ") ||
               trimmed.starts_with("case ") || trimmed.starts_with("begin") {
                depth += 1;
            }
            if trimmed == "end" || trimmed.starts_with("end ") {
                depth -= 1;
                if depth == 0 {
                    return j + 1;
                }
            }
        }
        start + 10 // Default fallback
    }

    /// Helper to find Scala block end (handles both brace and expression-style)
    fn find_scala_block_end(lines: &[&str], start: usize) -> usize {
        let line = lines.get(start).unwrap_or(&"");
        if line.contains('{') {
            Self::find_brace_end(lines, start)
        } else if line.contains('=') && !line.contains('{') {
            // Single-line expression
            start + 1
        } else {
            start + 10
        }
    }

    /// Helper to find matching closing brace
    fn find_brace_end(lines: &[&str], start: usize) -> usize {
        let mut brace_count = 0;
        let mut found_opening = false;

        for (j, next_line) in lines.iter().enumerate().skip(start) {
            for ch in next_line.chars() {
                match ch {
                    '{' => {
                        brace_count += 1;
                        found_opening = true;
                    }
                    '}' => {
                        brace_count -= 1;
                        if found_opening && brace_count == 0 {
                            return j + 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        start + 10 // Default fallback
    }

    /// Extract Python functions using regex (simplified) - instance method
    #[allow(dead_code)]
    fn extract_python_functions(&self, content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        Self::extract_python_functions_static(content, file_path)
    }

    /// Extract Python functions using regex (simplified) - static for parallel
    fn extract_python_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            // Use pre-compiled regex for speed
            if let Some(captures) = PYTHON_FUNC.captures(line) {
                let func_name = captures.get(2).unwrap().as_str().to_string();
                let params = captures.get(3).unwrap().as_str();

                // Calculate function end (simplified - find next def or end of file)
                let mut end_line = i + 1;
                let mut indent_level = None;

                for (j, next_line) in lines.iter().enumerate().skip(i + 1) {
                    let next_trimmed = next_line.trim();

                    // Skip empty lines and comments
                    if next_trimmed.is_empty() || next_trimmed.starts_with('#') {
                        continue;
                    }

                    // Get indentation level
                    let current_indent = next_line.len() - next_line.trim_start().len();

                    if indent_level.is_none() && !next_trimmed.is_empty() {
                        indent_level = Some(current_indent);
                    }

                    // Check if we've reached the end of the function
                    if let Some(expected_indent) = indent_level {
                        if current_indent <= expected_indent.saturating_sub(4) &&
                           (next_trimmed.starts_with("def ") ||
                            next_trimmed.starts_with("class ") ||
                            next_trimmed.starts_with("async def ")) {
                            break;
                        }
                    }

                    end_line = j + 1;
                }

                let signature = format!("def {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "python".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract Rust functions using regex (simplified) - instance method
    fn extract_rust_functions(&self, content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        Self::extract_rust_functions_static(content, file_path)
    }

    /// Extract Rust functions using regex (simplified) - static for parallel
    fn extract_rust_functions_static(content: &str, file_path: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Use pre-compiled regex for Rust functions
            if let Some(captures) = RUST_FALLBACK_FUNC.captures(trimmed) {
                let func_name = captures.get(1).unwrap().as_str().to_string();
                let params = captures.get(2).map(|m| m.as_str()).unwrap_or("");

                // Find function end by matching braces
                let mut end_line = i + 1;
                let mut brace_count = 0;
                let mut found_opening = false;

                for (j, next_line) in lines.iter().enumerate().skip(i) {
                    for ch in next_line.chars() {
                        match ch {
                            '{' => {
                                brace_count += 1;
                                found_opening = true;
                            }
                            '}' => {
                                brace_count -= 1;
                                if found_opening && brace_count == 0 {
                                    end_line = j + 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if found_opening && brace_count == 0 {
                        break;
                    }
                }

                let signature = format!("fn {}({})", func_name, params);
                let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                functions.push(FunctionInfo {
                    name: func_name,
                    signature,
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line,
                    language: "rust".to_string(),
                    complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });
            }
        }

        Ok(functions)
    }

    /// Extract JavaScript/TypeScript functions (simplified) - instance method
    fn extract_js_functions(&self, content: &str, file_path: &str, language: &str) -> Result<Vec<FunctionInfo>> {
        Self::extract_js_functions_static(content, file_path, language)
    }

    /// Extract JavaScript/TypeScript functions (simplified) - static for parallel
    fn extract_js_functions_static(content: &str, file_path: &str, language: &str) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Use pre-compiled regexes for all JS patterns
        let js_patterns: &[&Lazy<Regex>] = &[
            &JS_FALLBACK_FUNC,       // function name()
            &JS_FALLBACK_ARROW,      // const name = () =>
            &JS_FALLBACK_OBJ_FUNC,   // name: function()
            &JS_FALLBACK_OBJ_ARROW,  // name: () =>
        ];

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            for pattern in js_patterns {
                if let Some(captures) = pattern.captures(trimmed) {
                    let func_name = captures.get(1).unwrap().as_str().to_string();
                    let params = captures.get(2).map(|m| m.as_str()).unwrap_or("");

                    // Find function end (simplified)
                    let mut end_line = i + 10; // Default estimate
                    let mut brace_count = 0;
                    let mut found_opening = false;

                    for (j, next_line) in lines.iter().enumerate().skip(i) {
                        for ch in next_line.chars() {
                            match ch {
                                '{' => {
                                    brace_count += 1;
                                    found_opening = true;
                                }
                                '}' => {
                                    brace_count -= 1;
                                    if found_opening && brace_count == 0 {
                                        end_line = j + 1;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if found_opening && brace_count == 0 {
                            break;
                        }
                    }

                    let signature = format!("function {}({})", func_name, params);
                    let complexity = Self::estimate_complexity_static(&lines[i..end_line.min(lines.len())]);

                    functions.push(FunctionInfo {
                        name: func_name,
                        signature,
                        file_path: file_path.to_string(),
                        start_line: i + 1,
                        end_line,
                        language: language.to_string(),
                        complexity: Some(complexity),
                    has_unreachable_code: None,
                    unreachable_count: None,
                });

                    break; // Found a match, don't check other patterns
                }
            }
        }

        Ok(functions)
    }

    /// Estimate cyclomatic complexity (simplified) - instance method
    fn estimate_complexity(&self, lines: &[&str]) -> usize {
        Self::estimate_complexity_static(lines)
    }

    /// Estimate cyclomatic complexity (simplified) - static for parallel
    fn estimate_complexity_static(lines: &[&str]) -> usize {
        let mut complexity = 1; // Base complexity

        for line in lines {
            let trimmed = line.trim().to_lowercase();

            // Count decision points
            if trimmed.contains("if ") || trimmed.contains("elif ") || trimmed.contains("else if") {
                complexity += 1;
            }
            if trimmed.contains("while ") || trimmed.contains("for ") {
                complexity += 1;
            }
            if trimmed.contains("match ") || trimmed.contains("switch ") {
                complexity += 1;
            }
            if trimmed.contains("catch ") || trimmed.contains("except ") {
                complexity += 1;
            }
            if trimmed.contains("&&") || trimmed.contains("||") {
                complexity += 1;
            }
        }

        complexity
    }

    /// Check if file extension should be analyzed (basic version)
    fn should_analyze_extension_basic(&self, extension: &str) -> bool {
        matches!(extension, "py" | "rs" | "js" | "ts" | "tsx" | "jsx" | "dart")
    }

    /// Check if file extension should be analyzed
    fn should_analyze_extension(&self, extension: &str, languages: &[String]) -> bool {
        match extension {
            "py" => languages.contains(&"python".to_string()),
            "rs" => languages.contains(&"rust".to_string()),
            "js" => languages.contains(&"javascript".to_string()),
            "ts" | "tsx" => languages.contains(&"typescript".to_string()),
            "jsx" => languages.contains(&"javascript".to_string()) || languages.contains(&"typescript".to_string()),
            "dart" => languages.contains(&"dart".to_string()),
            _ => false,
        }
    }
    fn extension_to_language(&self, ext: &str) -> &'static str {
        Self::extension_to_language_static(ext)
    }

    fn extension_to_language_static(ext: &str) -> &'static str {
        match ext {
            "py" => "python",
            "rs" => "rust",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "dart" => "dart",
            _ => "unknown",
        }
    }

    /// 🚀 Ultra-fast batch INSERT for SQLite - inspired by Ultra-Brain performance patterns
    fn batch_insert_functions(&self, repo_id: &str, functions: &[FunctionInfo]) -> Result<()> {
        if functions.is_empty() {
            return Ok(());
        }

        let batch_size = 1000; // SQLite optimal batch size
        let indexed_at = Utc::now().timestamp();

        info!("💾 Batch inserting {} functions in chunks of {}", functions.len(), batch_size);

        // Use transaction for maximum performance
        let tx = self.connection.unchecked_transaction()?;

        for chunk in functions.chunks(batch_size) {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO functions (repo_id, name, signature, file_path, start_line, end_line, language, complexity, indexed_at, file_modified_at, has_unreachable_code, unreachable_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
            )?;

            for func in chunk {
                stmt.execute(params![
                    repo_id,
                    func.name,
                    func.signature,
                    func.file_path,
                    func.start_line,
                    func.end_line,
                    func.language,
                    func.complexity,
                    indexed_at,
                    0, // Default file_modified_at for backward compatibility
                    func.has_unreachable_code.unwrap_or(false) as i32,
                    func.unreachable_count.unwrap_or(0) as i32,
                ])?;
            }

            debug!("✓ Inserted batch of {} functions", chunk.len());
        }

        tx.commit()?;
        info!("✅ Batch INSERT completed - {} functions stored", functions.len());

        Ok(())
    }

    /// 🚀 Ultra-fast batch INSERT with file modification times for incremental indexing
    fn batch_insert_functions_with_modified_time(&self, repo_id: &str, functions: &[FunctionInfo]) -> Result<()> {
        if functions.is_empty() {
            return Ok(());
        }

        let batch_size = 1000; // SQLite optimal batch size
        let indexed_at = Utc::now().timestamp();

        info!("💾 Batch inserting {} functions with modification times in chunks of {}", functions.len(), batch_size);

        // Use transaction for maximum performance
        let tx = self.connection.unchecked_transaction()?;

        for chunk in functions.chunks(batch_size) {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO functions (repo_id, name, signature, file_path, start_line, end_line, language, complexity, indexed_at, file_modified_at, has_unreachable_code, unreachable_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
            )?;

            for func in chunk {
                // Extract file modification time from signature
                let (original_signature, file_modified) = if func.signature.contains("|MODIFIED:") {
                    let parts: Vec<&str> = func.signature.split("|MODIFIED:").collect();
                    let signature = parts[0];
                    let modified = parts.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                    (signature, modified)
                } else {
                    (func.signature.as_str(), 0)
                };

                stmt.execute(params![
                    repo_id,
                    func.name,
                    original_signature,
                    func.file_path,
                    func.start_line,
                    func.end_line,
                    func.language,
                    func.complexity,
                    indexed_at,
                    file_modified,
                    func.has_unreachable_code.unwrap_or(false) as i32,
                    func.unreachable_count.unwrap_or(0) as i32,
                ])?;
            }

            debug!("✓ Inserted batch of {} functions with modification times", chunk.len());
        }

        tx.commit()?;
        info!("✅ Batch INSERT with modification times completed - {} functions stored", functions.len());

        Ok(())
    }

    /// 🗑️ Clear all data for specific repository (cleanup)
    pub async fn clear_repository(&self, repo_id: &str) -> Result<()> {
        info!("🗑️ Clearing all data for repository: {}", repo_id);

        // Delete functions first (foreign key)
        let deleted_functions: usize = self.connection.execute(
            "DELETE FROM functions WHERE repo_id = ?1",
            params![repo_id],
        )? as usize;

        // Delete project record
        let deleted_projects: usize = self.connection.execute(
            "DELETE FROM projects WHERE repo_id = ?1",
            params![repo_id],
        )? as usize;

        info!("✅ Cleared {} functions and {} project records for: {}",
              deleted_functions, deleted_projects, repo_id);

        Ok(())
    }

    /// 📊 List all repositories in database with stats
    pub async fn list_repositories(&self) -> Result<Vec<serde_json::Value>> {
        info!("📊 Listing all repositories in database...");

        let mut stmt = self.connection.prepare(
            "SELECT p.repo_id, p.project_path, p.indexed_at, p.file_count, p.function_count,
                    COUNT(f.id) as actual_functions
             FROM projects p
             LEFT JOIN functions f ON p.repo_id = f.repo_id
             GROUP BY p.repo_id, p.project_path, p.indexed_at, p.file_count, p.function_count
             ORDER BY p.indexed_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let indexed_at: i64 = row.get(2)?;
            let indexed_date = chrono::DateTime::from_timestamp(indexed_at, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();

            Ok(serde_json::json!({
                "repo_id": row.get::<_, String>(0)?,
                "project_path": row.get::<_, String>(1)?,
                "indexed_at": indexed_date,
                "file_count": row.get::<_, i64>(3)?,
                "function_count_stored": row.get::<_, i64>(4)?,
                "function_count_actual": row.get::<_, i64>(5)?,
            }))
        })?;

        let repositories: Vec<serde_json::Value> = rows.collect::<Result<Vec<_>, _>>()?;

        info!("📋 Found {} repositories in database", repositories.len());
        for repo in &repositories {
            info!("  - {}: {} functions, {} files",
                  repo["repo_id"], repo["function_count_actual"], repo["file_count"]);
        }

        Ok(repositories)
    }

    /// 🧹 Cleanup old/duplicate repositories (keep only latest)
    pub async fn cleanup_old_repositories(&self, keep_latest: usize) -> Result<()> {
        info!("🧹 Cleaning up old repositories, keeping latest {} entries per path...", keep_latest);

        // Find repositories to delete (keep only latest N per project_path)
        let mut stmt = self.connection.prepare(
            "SELECT repo_id, project_path, indexed_at,
                    ROW_NUMBER() OVER (PARTITION BY project_path ORDER BY indexed_at DESC) as row_num
             FROM projects"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // repo_id
                row.get::<_, String>(1)?, // project_path
                row.get::<_, i64>(2)?,    // indexed_at
                row.get::<_, i64>(3)?,    // row_num
            ))
        })?;

        let mut repos_to_delete = Vec::new();
        for row_result in rows {
            let (repo_id, project_path, _indexed_at, row_num) = row_result?;
            if row_num > keep_latest as i64 {
                repos_to_delete.push((repo_id, project_path));
            }
        }

        if repos_to_delete.is_empty() {
            info!("✅ No old repositories to cleanup");
            return Ok(());
        }

        // Delete old repositories
        let mut deleted_count = 0;
        for (repo_id, project_path) in repos_to_delete {
            info!("🗑️ Deleting old repository: {} ({})", repo_id, project_path);
            self.clear_repository(&repo_id).await?;
            deleted_count += 1;
        }

        info!("✅ Cleaned up {} old repositories", deleted_count);
        Ok(())
    }

    /// 📈 Get database statistics and health
    pub async fn get_database_stats(&self) -> Result<serde_json::Value> {
        info!("📈 Gathering database statistics...");

        // Basic counts
        let total_projects: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM projects", [], |row| Ok(row.get(0)?)
        )?;

        let total_functions: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM functions", [], |row| Ok(row.get(0)?)
        )?;

        let total_files: i64 = self.connection.query_row(
            "SELECT COUNT(DISTINCT file_path) FROM functions", [], |row| Ok(row.get(0)?)
        )?;

        // Language breakdown
        let mut lang_stmt = self.connection.prepare(
            "SELECT language, COUNT(*) FROM functions GROUP BY language ORDER BY COUNT(*) DESC"
        )?;

        let lang_rows = lang_stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut languages = serde_json::Map::new();
        for lang_result in lang_rows {
            let (lang, count) = lang_result?;
            languages.insert(lang, serde_json::Value::Number(count.into()));
        }

        // Database file size
        let db_size_bytes = std::fs::metadata(&self.db_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let db_size_mb = db_size_bytes as f64 / (1024.0 * 1024.0);

        Ok(serde_json::json!({
            "database_path": self.db_path.display().to_string(),
            "database_size_mb": format!("{:.2}", db_size_mb),
            "total_projects": total_projects,
            "total_functions": total_functions,
            "total_files": total_files,
            "languages": languages,
            "avg_functions_per_file": if total_files > 0 {
                format!("{:.1}", total_functions as f64 / total_files as f64)
            } else { "0".to_string() }
        }))
    }

    /// 🔍 Vacuum and optimize database
    pub async fn optimize_database(&self) -> Result<()> {
        info!("🔍 Optimizing database...");

        // VACUUM to reclaim space
        self.connection.execute("VACUUM", [])?;

        // Analyze to update query planner statistics
        self.connection.execute("ANALYZE", [])?;

        info!("✅ Database optimized");
        Ok(())
    }

    /// 🔍 Search functions by name pattern (with regex support)
    pub async fn search_functions(
        &self,
        repo_id: &str,
        pattern: &str,
        is_regex: bool,
        language: Option<&str>,
        include_body: bool,
        context_lines: usize,
        max_results: usize,
    ) -> Result<Vec<serde_json::Value>> {
        info!("🔍 Searching functions with pattern: {} (regex: {})", pattern, is_regex);

        let project_path = self.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))?;

        // Simple approach: get all functions and filter in Rust
        let all_functions = self.get_all_functions(repo_id).await?;

        let mut results = Vec::new();
        let regex_pattern = if is_regex {
            Some(regex::Regex::new(pattern)?)
        } else {
            None
        };

        for func in all_functions {
            if results.len() >= max_results {
                break;
            }

            // Filter by language if specified
            if let Some(lang) = language {
                if func.language != lang {
                    continue;
                }
            }

            // Filter by pattern
            let matches = if let Some(ref re) = regex_pattern {
                re.is_match(&func.name)
            } else {
                func.name.to_lowercase().contains(&pattern.to_lowercase())
            };

            if !matches {
                continue;
            }

            let mut func_json = serde_json::json!({
                "name": func.name,
                "signature": func.signature,
                "file_path": func.file_path,
                "start_line": func.start_line,
                "end_line": func.end_line,
                "language": func.language,
                "complexity": func.complexity,
            });

            // Add body and context if requested
            if include_body || context_lines > 0 {
                let full_path = std::path::Path::new(&project_path).join(&func.file_path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = func.start_line.saturating_sub(1);
                    let end = func.end_line.min(lines.len());

                    if include_body && start < lines.len() {
                        let body: String = lines[start..end].join("\n");
                        func_json["body"] = serde_json::json!(body);
                    }

                    if context_lines > 0 {
                        let ctx_start = start.saturating_sub(context_lines);
                        let ctx_end = (end + context_lines).min(lines.len());

                        if ctx_start < start {
                            let before: String = lines[ctx_start..start].join("\n");
                            func_json["context_before"] = serde_json::json!(before);
                        }
                        if end < ctx_end {
                            let after: String = lines[end..ctx_end].join("\n");
                            func_json["context_after"] = serde_json::json!(after);
                        }
                    }
                }
            }

            results.push(func_json);
        }

        info!("✅ Found {} matching functions", results.len());
        Ok(results)
    }

    /// 📄 Get all functions from a specific file
    pub async fn get_file_functions(
        &self,
        repo_id: &str,
        file_path: &str,
        include_body: bool,
        context_lines: usize,
    ) -> Result<Vec<serde_json::Value>> {
        info!("📄 Getting functions from file: {}", file_path);

        let project_path = self.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))?;

        // Get all functions and filter by file path
        let all_functions = self.get_all_functions(repo_id).await?;

        let mut results = Vec::new();
        let mut file_content: Option<Vec<String>> = None;

        for func in all_functions {
            // Filter by file path
            if !func.file_path.contains(file_path) && !func.file_path.ends_with(file_path) {
                continue;
            }

            // Load file content once if needed
            if (include_body || context_lines > 0) && file_content.is_none() {
                let full_path = std::path::Path::new(&project_path).join(&func.file_path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    file_content = Some(content.lines().map(|s| s.to_string()).collect());
                }
            }

            let mut func_json = serde_json::json!({
                "name": func.name,
                "signature": func.signature,
                "file_path": func.file_path,
                "start_line": func.start_line,
                "end_line": func.end_line,
                "language": func.language,
                "complexity": func.complexity,
            });

            if let Some(ref lines) = file_content {
                // ⚡ DEFENSIVE: File may have changed since indexing - clamp to actual file size
                let start = func.start_line.saturating_sub(1).min(lines.len().saturating_sub(1)); // 0-based index
                let end = func.end_line.saturating_sub(1).min(lines.len().saturating_sub(1)); // 0-based index, clamped to file size

                // Safety check: ensure valid range
                if start >= lines.len() || end >= lines.len() || start > end {
                    // Skip this function - file has changed significantly
                    continue;
                }

                if include_body {
                    let body: String = lines[start..=end].join("\n"); // Use inclusive range
                    func_json["body"] = serde_json::json!(body);
                }

                if context_lines > 0 {
                    let ctx_start = start.saturating_sub(context_lines);
                    let ctx_end = (end + context_lines + 1).min(lines.len()); // +1 because end is now 0-based index

                    if ctx_start < start {
                        let before: String = lines[ctx_start..start].join("\n");
                        func_json["context_before"] = serde_json::json!(before);
                    }
                    if end + 1 < ctx_end && end + 1 < lines.len() {
                        let after: String = lines[(end + 1)..ctx_end].join("\n");
                        func_json["context_after"] = serde_json::json!(after);
                    }
                }
            }

            results.push(func_json);
        }

        info!("✅ Found {} functions in file", results.len());
        Ok(results)
    }

    /// 🔎 Get detailed function information
    pub async fn get_function_details(
        &self,
        repo_id: &str,
        function_name: &str,
        file_path: Option<&str>,
        include_callers: bool,
        include_callees: bool,
        context_lines: usize,
    ) -> Result<serde_json::Value> {
        info!("🔎 Getting details for function: {}", function_name);

        let project_path = self.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))?;

        // Get all functions and find the matching one
        let all_functions = self.get_all_functions(repo_id).await?;

        let func = all_functions.iter()
            .find(|f| {
                let name_matches = f.name == function_name;
                let path_matches = file_path.map(|fp| f.file_path.contains(fp) || f.file_path.ends_with(fp)).unwrap_or(true);
                name_matches && path_matches
            })
            .ok_or_else(|| anyhow::anyhow!("Function not found: {}", function_name))?;

        // Read file content
        let full_path = std::path::Path::new(&project_path).join(&func.file_path);
        let file_content = std::fs::read_to_string(&full_path).ok();
        let lines: Vec<&str> = file_content.as_ref().map(|c| c.lines().collect()).unwrap_or_default();

        let start = func.start_line.saturating_sub(1);
        let end = func.end_line.min(lines.len());

        let body = if start < lines.len() {
            lines[start..end].join("\n")
        } else {
            String::new()
        };

        // Extract context
        let ctx_start = start.saturating_sub(context_lines);
        let ctx_end = (end + context_lines).min(lines.len());

        let context_before = if ctx_start < start && !lines.is_empty() {
            lines[ctx_start..start].join("\n")
        } else {
            String::new()
        };

        let context_after = if end < ctx_end && !lines.is_empty() {
            lines[end..ctx_end].join("\n")
        } else {
            String::new()
        };

        // Find callers and callees by analyzing function body
        let mut callers = Vec::new();
        let mut callees = Vec::new();

        if include_callees && !body.is_empty() {
            // Simple callees detection: look for function call patterns in body
            for other_func in &all_functions {
                if other_func.name != func.name && body.contains(&format!("{}(", other_func.name)) {
                    callees.push(serde_json::json!({
                        "name": other_func.name,
                        "file_path": other_func.file_path,
                        "start_line": other_func.start_line
                    }));
                }
            }
        }

        if include_callers {
            // Find functions that call this function
            let call_pattern = format!("{}(", func.name);

            for other_func in &all_functions {
                if other_func.name == func.name {
                    continue;
                }

                // Read each function's body and check for calls
                let other_full_path = std::path::Path::new(&project_path).join(&other_func.file_path);
                if let Ok(other_content) = std::fs::read_to_string(&other_full_path) {
                    let other_lines: Vec<&str> = other_content.lines().collect();
                    let other_start = other_func.start_line.saturating_sub(1);
                    let other_end = other_func.end_line.min(other_lines.len());

                    if other_start < other_lines.len() {
                        let other_body: String = other_lines[other_start..other_end].join("\n");
                        if other_body.contains(&call_pattern) {
                            callers.push(serde_json::json!({
                                "name": other_func.name,
                                "file_path": other_func.file_path,
                                "start_line": other_func.start_line
                            }));
                        }
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "name": func.name,
            "signature": func.signature,
            "file_path": func.file_path,
            "start_line": func.start_line,
            "end_line": func.end_line,
            "language": func.language,
            "complexity": func.complexity,
            "body": body,
            "context_before": context_before,
            "context_after": context_after,
            "callers": callers,
            "callees": callees,
            "callers_count": callers.len(),
            "callees_count": callees.len()
        }))
    }

    /// 📦 Get imports from files
    pub async fn get_imports(
        &self,
        repo_id: &str,
        file_path: Option<&str>,
        include_external: bool,
        group_by_file: bool,
    ) -> Result<serde_json::Value> {
        info!("📦 Analyzing imports for repo: {}", repo_id);

        let project_path = self.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))?;

        // Get unique files from functions
        let mut stmt = self.connection.prepare(
            "SELECT DISTINCT file_path, language FROM functions WHERE repo_id = ?1"
        )?;

        let rows = stmt.query_map([repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut all_imports: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        let mut total_imports = 0;

        for row_result in rows {
            let (fp, lang) = row_result?;

            // Filter by specific file if provided
            if let Some(target_file) = file_path {
                if !fp.contains(target_file) {
                    continue;
                }
            }

            let full_path = std::path::Path::new(&project_path).join(&fp);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let imports = Self::extract_imports_from_content(&content, &lang, include_external);

                if !imports.is_empty() {
                    total_imports += imports.len();
                    all_imports.insert(fp.clone(), imports);
                }
            }
        }

        if group_by_file {
            Ok(serde_json::json!({
                "repo_id": repo_id,
                "total_imports": total_imports,
                "files_analyzed": all_imports.len(),
                "imports_by_file": all_imports
            }))
        } else {
            // Flatten all imports into single list
            let flat_imports: Vec<serde_json::Value> = all_imports
                .into_iter()
                .flat_map(|(file, imports)| {
                    imports.into_iter().map(move |mut imp| {
                        imp["source_file"] = serde_json::json!(file.clone());
                        imp
                    }).collect::<Vec<_>>()
                })
                .collect();

            Ok(serde_json::json!({
                "repo_id": repo_id,
                "total_imports": flat_imports.len(),
                "imports": flat_imports
            }))
        }
    }

    /// Extract imports from file content based on language
    fn extract_imports_from_content(
        content: &str,
        language: &str,
        include_external: bool
    ) -> Vec<serde_json::Value> {
        let mut imports = Vec::new();

        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            let import_info = match language {
                "python" => {
                    if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                        let is_external = !trimmed.contains("from .") && !trimmed.starts_with("from .");
                        if include_external || !is_external {
                            Some(serde_json::json!({
                                "line": i + 1,
                                "statement": trimmed,
                                "type": if trimmed.starts_with("from ") { "from_import" } else { "import" },
                                "is_external": is_external
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                "rust" => {
                    if trimmed.starts_with("use ") || trimmed.starts_with("extern crate ") {
                        let is_external = trimmed.starts_with("use std::") ||
                            trimmed.contains("::") && !trimmed.starts_with("use crate::");
                        if include_external || !is_external {
                            Some(serde_json::json!({
                                "line": i + 1,
                                "statement": trimmed,
                                "type": if trimmed.starts_with("extern ") { "extern_crate" } else { "use" },
                                "is_external": is_external
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                "javascript" | "typescript" | "tsx" | "jsx" => {
                    if trimmed.starts_with("import ") || trimmed.starts_with("const ") && trimmed.contains("require(") {
                        let is_external = !trimmed.contains("from './") &&
                            !trimmed.contains("from \"./") &&
                            !trimmed.contains("from '../");
                        if include_external || !is_external {
                            Some(serde_json::json!({
                                "line": i + 1,
                                "statement": trimmed,
                                "type": if trimmed.starts_with("import ") { "es_import" } else { "require" },
                                "is_external": is_external
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                "java" => {
                    if trimmed.starts_with("import ") {
                        let is_external = !trimmed.contains("import com.") || trimmed.contains("import java.") ||
                            trimmed.contains("import javax.");
                        if include_external || !is_external {
                            Some(serde_json::json!({
                                "line": i + 1,
                                "statement": trimmed,
                                "type": "import",
                                "is_external": is_external
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                "go" => {
                    if trimmed.starts_with("import ") || (trimmed.starts_with("\"") && trimmed.ends_with("\"")) {
                        let is_external = !trimmed.contains("./") && !trimmed.contains("../");
                        if include_external || !is_external {
                            Some(serde_json::json!({
                                "line": i + 1,
                                "statement": trimmed,
                                "type": "import",
                                "is_external": is_external
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                _ => None
            };

            if let Some(info) = import_info {
                imports.push(info);
            }
        }

        imports
    }

    /// 🔗 Get call graph for project
    pub async fn get_call_graph(
        &self,
        repo_id: &str,
        function_name: Option<&str>,
        depth: usize,
        direction: &str,
        include_external: bool,
    ) -> Result<serde_json::Value> {
        info!("🔗 Building call graph for repo: {} (depth: {}, direction: {})", repo_id, depth, direction);

        let project_path = self.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))?;

        let all_functions = self.get_all_functions(repo_id).await?;
        let mut edges: Vec<serde_json::Value> = Vec::new();
        let mut nodes: HashMap<String, serde_json::Value> = HashMap::new();

        // Build nodes map
        for func in &all_functions {
            nodes.insert(func.name.clone(), serde_json::json!({
                "name": func.name,
                "file_path": func.file_path,
                "start_line": func.start_line,
                "language": func.language
            }));
        }

        // Analyze calls
        for func in &all_functions {
            // Skip if targeting specific function and this isn't it
            if let Some(target) = function_name {
                if func.name != target {
                    continue;
                }
            }

            let full_path = std::path::Path::new(&project_path).join(&func.file_path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let lines: Vec<&str> = content.lines().collect();
                let start = func.start_line.saturating_sub(1);
                let end = func.end_line.min(lines.len());

                if start < lines.len() {
                    let body: String = lines[start..end].join("\n");

                    // Find function calls in body (callees)
                    if direction == "both" || direction == "callees" {
                        for other_func in &all_functions {
                            if other_func.name == func.name {
                                continue;
                            }

                            let call_pattern = format!("{}(", other_func.name);
                            if body.contains(&call_pattern) {
                                edges.push(serde_json::json!({
                                    "from": func.name,
                                    "to": other_func.name,
                                    "type": "calls",
                                    "from_file": func.file_path,
                                    "to_file": other_func.file_path
                                }));
                            }
                        }

                        // Detect external calls if requested
                        if include_external {
                            let external_calls = Self::extract_external_calls(&body, &func.language);
                            for ext_call in external_calls {
                                edges.push(serde_json::json!({
                                    "from": func.name,
                                    "to": ext_call,
                                    "type": "external_call",
                                    "from_file": func.file_path,
                                    "to_file": "external"
                                }));
                            }
                        }
                    }
                }
            }
        }

        // For "callers" direction, we need reverse edges
        if direction == "both" || direction == "callers" {
            if let Some(target) = function_name {
                let call_pattern = format!("{}(", target);

                for func in &all_functions {
                    if func.name == target {
                        continue;
                    }

                    let full_path = std::path::Path::new(&project_path).join(&func.file_path);
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        let lines: Vec<&str> = content.lines().collect();
                        let start = func.start_line.saturating_sub(1);
                        let end = func.end_line.min(lines.len());

                        if start < lines.len() {
                            let body: String = lines[start..end].join("\n");
                            if body.contains(&call_pattern) {
                                edges.push(serde_json::json!({
                                    "from": func.name,
                                    "to": target,
                                    "type": "calls",
                                    "from_file": func.file_path,
                                    "to_file": "target"
                                }));
                            }
                        }
                    }
                }
            }
        }

        // Calculate stats
        let unique_callers: std::collections::HashSet<String> = edges.iter()
            .filter_map(|e| e["from"].as_str().map(|s| s.to_string()))
            .collect();
        let unique_callees: std::collections::HashSet<String> = edges.iter()
            .filter_map(|e| e["to"].as_str().map(|s| s.to_string()))
            .collect();

        Ok(serde_json::json!({
            "repo_id": repo_id,
            "function": function_name,
            "depth": depth,
            "direction": direction,
            "total_functions": all_functions.len(),
            "total_edges": edges.len(),
            "unique_callers": unique_callers.len(),
            "unique_callees": unique_callees.len(),
            "nodes": nodes.values().collect::<Vec<_>>(),
            "edges": edges
        }))
    }

    /// Extract external function calls from body
    fn extract_external_calls(body: &str, language: &str) -> Vec<String> {
        let mut external_calls = Vec::new();

        // Common external function patterns by language
        let patterns: Vec<&str> = match language {
            "python" => vec!["print(", "len(", "str(", "int(", "list(", "dict(", "range(", "open(", "json."],
            "rust" => vec!["println!", "eprintln!", "format!", "vec!", "panic!", "assert!", "debug!"],
            "javascript" | "typescript" => vec!["console.log", "console.error", "fetch(", "JSON.", "Math.", "Array.", "Object."],
            "java" => vec!["System.out.", "System.err.", "String.", "Integer.", "Arrays.", "Collections."],
            "go" => vec!["fmt.Print", "fmt.Sprint", "log.", "json.", "http.", "os.", "io."],
            _ => vec![],
        };

        for pattern in patterns {
            if body.contains(pattern) {
                let name = pattern.trim_end_matches(&['(', '.', '!'][..]);
                if !external_calls.contains(&name.to_string()) {
                    external_calls.push(name.to_string());
                }
            }
        }

        external_calls
    }
}
