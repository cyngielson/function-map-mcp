// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Hierarchical Tree Structure - Function grouping per file
//!
//! - Grupowanie funkcji per plik
//! - Nested structure: Project > Files > Functions
//! - Cross-file relationships
//! - Context preservation
//! - GLOBAL IN-MEMORY CACHE for instant queries (256GB RAM = no problem!)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use crate::psi_graph::{FunctionInfo, PsiGraphManager, get_global_file_content};

// Ten plik buduje "one-scan" widok projektu (tree) na podstawie już zaindeksowanych funkcji.
// Ważne: nie robimy tu żadnego indeksowania ani efektów ubocznych (read-only). Jeśli potrzebujesz
// bogatszych metadanych (imports/docstring), wyciągamy je szybko z globalnego cache plików w RAM.

/// Hierarchical function tree grouped per file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalTree {
    pub project_path: String,
    pub repo_id: String,
    pub total_files: usize,
    pub total_functions: usize,
    pub files: HashMap<String, FileNode>,
    pub cross_references: Vec<CrossReference>,
    pub generated_at: i64,
}

/// File node in hierarchical tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub file_path: String,
    pub language: String,
    pub function_count: usize,
    pub functions: Vec<FunctionNode>,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub file_hash: Option<String>,
    pub last_modified: Option<i64>,
}

/// Function node with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionNode {
    pub name: String,
    pub signature: String,
    pub start_line: usize,
    pub end_line: usize,
    pub visibility: String,
    pub is_async: bool,
    pub is_static: bool,
    pub complexity: Option<usize>,
    pub docstring: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
    pub calls_to: Vec<String>,      // Functions this function calls
    pub called_by: Vec<String>,     // Functions that call this function
    pub context_before: Vec<String>, // Lines before function
    pub context_after: Vec<String>,  // Lines after function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_unreachable_code: Option<bool>, // ⚠️ CFG analysis result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreachable_count: Option<usize>,   // Number of unreachable blocks
}

impl FunctionNode {
    /// Get display name with warning marker if has unreachable code
    pub fn display_name(&self) -> String {
        if self.has_unreachable_code == Some(true) {
            format!("⚠️ {}", self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub param_type: Option<String>,
    pub default_value: Option<String>,
    pub is_optional: bool,
}

/// Cross-file relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReference {
    pub from_file: String,
    pub from_function: String,
    pub to_file: String,
    pub to_function: String,
    pub relationship_type: RelationshipType,
    pub line_number: Option<usize>,
}

/// Types of relationships between functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    FunctionCall,
    Import,
    Inheritance,
    Reference,
    TypeUsage,
}

/// Hierarchical tree builder - uses GLOBAL in-memory cache for instant queries
pub struct HierarchicalTreeBuilder {
    psi_manager: PsiGraphManager,
}

impl HierarchicalTreeBuilder {
    /// Create new hierarchical tree builder
    pub async fn new() -> Result<Self> {
        let psi_manager = PsiGraphManager::new().await?;
        Ok(Self {
            psi_manager,
        })
    }

    /// Build hierarchical tree from an already indexed repository.
    ///
    /// Contract:
    /// - `repo_id` MUST exist in SQLite (created by `lft_index_project` / `lft_index_project_incremental`)
    /// - This method is READ-ONLY (no indexing side effects)
    pub async fn build_tree_by_repo_id(
        &self,
        repo_id: &str,
        include_context: bool,
        context_lines: usize,
    ) -> Result<HierarchicalTree> {
        let project_path = self.psi_manager.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found. Run lft_index_project first.", repo_id))?;

        let all_functions = self.psi_manager.get_all_functions(repo_id).await?;

    // Group functions by file
    let mut files = HashMap::new();
        let mut total_functions = 0;

        for func in &all_functions {
            let file_path = &func.file_path;

            let file_node = files.entry(file_path.clone()).or_insert_with(|| FileNode {
                file_path: file_path.clone(),
                language: func.language.clone(),
                function_count: 0,
                functions: Vec::new(),
                // Uzupełnimy po zbudowaniu listy plików (jedno przejście, cache w RAM)
                imports: Vec::new(),
                exports: Vec::new(),
                file_hash: None,
                last_modified: None,
            });

            // Convert FunctionInfo to FunctionNode with context
            let function_node = self.build_function_node(func, &project_path, include_context, context_lines).await?;
            file_node.functions.push(function_node);
            file_node.function_count += 1;
            total_functions += 1;
        }

        // Fill per-file metadata (imports/exports) using global RAM cache
        for file_node in files.values_mut() {
            let (imports, exports) = self.extract_file_metadata(&project_path, &file_node.file_path, &file_node.language);
            file_node.imports = imports;
            file_node.exports = exports;
        }

        // Build cross-references
        let cross_references = self.build_cross_references(&all_functions).await?;

        Ok(HierarchicalTree {
            project_path: project_path.to_string(),
            repo_id: repo_id.to_string(),
            total_files: files.len(),
            total_functions,
            files,
            cross_references,
            generated_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Backwards-compatible helper: resolve repo_id from project_path (read-only) and then build.
    ///
    /// Important: This does NOT create any repo entries. If not indexed, it errors.
    pub async fn build_tree_by_project_path(
        &self,
        project_path: &str,
        include_context: bool,
        context_lines: usize,
    ) -> Result<HierarchicalTree> {
        let repo_id = self.psi_manager.find_repo_id_by_project_path(project_path).await?
            .ok_or_else(|| anyhow::anyhow!(
                "Project '{}' is not indexed yet. Run lft_index_project first.",
                project_path
            ))?;

        self.build_tree_by_repo_id(&repo_id, include_context, context_lines).await
    }

    /// Build function node with context
    async fn build_function_node(
        &self,
        func: &FunctionInfo,
        project_path: &str,
        include_context: bool,
        context_lines: usize,
    ) -> Result<FunctionNode> {
        let (context_before, context_after) = if include_context {
            self.extract_function_context(project_path, &func.file_path, func.start_line, func.end_line, context_lines).await?
        } else {
            (Vec::new(), Vec::new())
        };

        let docstring = self.extract_python_docstring(project_path, &func.file_path, &func.language, func.start_line);

        Ok(FunctionNode {
            name: func.name.clone(),
            signature: func.signature.clone(),
            start_line: func.start_line,
            end_line: func.end_line,
            visibility: "public".to_string(), // TODO: Extract from signature
            is_async: func.signature.contains("async"),
            is_static: !func.signature.contains("self"),
            complexity: func.complexity,
            docstring,
            parameters: self.parse_parameters(&func.signature),
            return_type: self.extract_return_type(&func.signature),
            calls_to: Vec::new(), // TODO: Analyze function calls
            called_by: Vec::new(), // TODO: Reverse lookup
            context_before,
            context_after,
            has_unreachable_code: func.has_unreachable_code,
            unreachable_count: func.unreachable_count,
        })
    }

    /// Extract per-file metadata (imports/exports).
    ///
    /// Currently implemented for Python because `system_taxi` is Python-based.
    /// This is intentionally cheap (no AST) and reads from the global in-memory file cache.
    fn extract_file_metadata(
        &self,
        project_path: &str,
        file_path: &str,
        language: &str,
    ) -> (Vec<String>, Vec<String>) {
        if !language.eq_ignore_ascii_case("python") {
            return (Vec::new(), Vec::new());
        }

        let full_path_str = {
            let p = std::path::Path::new(file_path);
            if p.is_absolute() {
                p.to_string_lossy().to_string()
            } else {
                std::path::Path::new(project_path)
                    .join(file_path)
                    .to_string_lossy()
                    .to_string()
            }
        };

        let Some(lines) = get_global_file_content(&full_path_str) else {
            return (Vec::new(), Vec::new());
        };

        // Only parse the first part of file to keep it predictable and fast.
        let max_scan = std::cmp::min(lines.len(), 250);
        let mut imports: Vec<String> = Vec::new();

        for raw in lines.iter().take(max_scan) {
            let line = raw.trim();

            // Stop scanning once we are deep into code; imports usually live on top.
            if line.starts_with("def ") || line.starts_with("class ") {
                // Still allow decorators/imports above functions, but once we hit real code, stop.
                break;
            }

            if line.starts_with("import ") || line.starts_with("from ") {
                // Normalize whitespace, keep it short.
                let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
                if !normalized.is_empty() {
                    imports.push(normalized);
                }
            }
        }

        imports.sort();
        imports.dedup();

        (imports, Vec::new())
    }

    /// Extract Python docstring for a function, if present.
    ///
    /// Heuristic:
    /// - Look at lines after the `def` line (start_line) skipping decorators/blank lines.
    /// - If first meaningful statement is a string literal ("""...""" or '...'), treat it as docstring.
    ///
    /// Safety:
    /// - We cap length to keep output small and predictable.
    fn extract_python_docstring(
        &self,
        project_path: &str,
        file_path: &str,
        language: &str,
        start_line: usize,
    ) -> Option<String> {
        if !language.eq_ignore_ascii_case("python") {
            return None;
        }

        let full_path_str = {
            let p = std::path::Path::new(file_path);
            if p.is_absolute() {
                p.to_string_lossy().to_string()
            } else {
                std::path::Path::new(project_path)
                    .join(file_path)
                    .to_string_lossy()
                    .to_string()
            }
        };

        let lines = get_global_file_content(&full_path_str)?;

        // start_line is 1-based from indexer; our Vec is 0-based.
        let mut i = start_line.saturating_sub(1);
        if i >= lines.len() {
            return None;
        }

        // Scan a small window after def line.
        let scan_end = std::cmp::min(lines.len(), i + 30);
        i = std::cmp::min(i + 1, lines.len());

        while i < scan_end {
            let raw = lines[i].trim();

            if raw.is_empty() || raw.starts_with("@") {
                i += 1;
                continue;
            }

            // One-liner docstring: "text" or 'text'
            if (raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2)
                || (raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2)
            {
                let mut s = raw.trim_matches('"').trim_matches('\'').to_string();
                s = s.replace("\\n", " ");
                return Self::cap_docstring(s);
            }

            // Triple-quote docstring: """...""" or '''...'''
            if raw.starts_with("\"\"\"") || raw.starts_with("'''") {
                let delim = if raw.starts_with("\"\"\"") { "\"\"\"" } else { "'''" };
                let after_open = raw.strip_prefix(delim).unwrap_or("");

                // Same-line triple quoted
                if let Some(end_pos) = after_open.find(delim) {
                    let inside = after_open[..end_pos].trim().to_string();
                    return Self::cap_docstring(inside);
                }

                // Multi-line triple quoted
                let mut acc: Vec<String> = Vec::new();
                if !after_open.trim().is_empty() {
                    acc.push(after_open.trim().to_string());
                }

                i += 1;
                while i < scan_end {
                    let l = lines[i].trim();
                    if let Some(end_idx) = l.find(delim) {
                        let part = l[..end_idx].trim();
                        if !part.is_empty() {
                            acc.push(part.to_string());
                        }
                        break;
                    }
                    if !l.is_empty() {
                        acc.push(l.to_string());
                    }
                    i += 1;
                }

                let joined = acc.join(" ");
                if joined.trim().is_empty() {
                    return None;
                }
                return Self::cap_docstring(joined);
            }

            // First non-empty non-string statement => no docstring
            return None;
        }

        None
    }

    fn cap_docstring(mut s: String) -> Option<String> {
        let s_trim = s.trim();
        if s_trim.is_empty() {
            return None;
        }
        s = s_trim.to_string();
        const MAX: usize = 240;
        if s.len() > MAX {
            s.truncate(MAX);
            s.push_str("...");
        }
        Some(s)
    }

    /// Extract function context (lines before/after) - FROM GLOBAL RAM CACHE (INSTANT!)
    async fn extract_function_context(
        &self,
        project_path: &str,
        file_path: &str,
        start_line: usize,
        end_line: usize,
        context_lines: usize,
    ) -> Result<(Vec<String>, Vec<String>)> {
        // Combine project path with file path safely.
        // NOTE: In our DB, `file_path` is usually ABSOLUTE (e.g. C:\taxi\...\app.py).
        // Joining an absolute path in Rust discards the prefix on Unix-ish paths, but on Windows
        // it can produce surprising results and break cache lookups.
        let full_path_str = {
            let p = std::path::Path::new(file_path);
            if p.is_absolute() {
                p.to_string_lossy().to_string()
            } else {
                std::path::Path::new(project_path)
                    .join(file_path)
                    .to_string_lossy()
                    .to_string()
            }
        };

        // Get from GLOBAL cache (pre-loaded at startup - INSTANT!)
        let lines = match get_global_file_content(&full_path_str) {
            Some(content) => content,
            None => return Ok((Vec::new(), Vec::new())),
        };

        // Extract context before
        let context_start = if start_line > context_lines { start_line - context_lines } else { 0 };
        let context_before: Vec<String> = lines
            .get(context_start..start_line.saturating_sub(1))
            .unwrap_or(&[])
            .to_vec();

        // Extract context after
        let context_after: Vec<String> = lines
            .get(end_line..std::cmp::min(end_line + context_lines, lines.len()))
            .unwrap_or(&[])
            .to_vec();

        Ok((context_before, context_after))
    }

    /// Parse function parameters from signature
    fn parse_parameters(&self, signature: &str) -> Vec<Parameter> {
        // Simple parameter parsing - TODO: Improve with AST
        let mut params = Vec::new();

        if let Some(params_start) = signature.find('(') {
            if let Some(params_end) = signature.find(')') {
                let params_str = &signature[params_start + 1..params_end];
                if !params_str.trim().is_empty() {
                    for param in params_str.split(',') {
                        let param = param.trim();
                        if !param.is_empty() && param != "self" && param != "&self" {
                            params.push(Parameter {
                                name: param.split(':').next().unwrap_or(param).trim().to_string(),
                                param_type: None, // TODO: Extract type
                                default_value: None,
                                is_optional: false,
                            });
                        }
                    }
                }
            }
        }

        params
    }

    /// Extract return type from signature
    fn extract_return_type(&self, signature: &str) -> Option<String> {
        if let Some(arrow_pos) = signature.find("->") {
            let return_part = signature[arrow_pos + 2..].trim();
            if let Some(brace_pos) = return_part.find('{') {
                Some(return_part[..brace_pos].trim().to_string())
            } else {
                Some(return_part.to_string())
            }
        } else {
            None
        }
    }

    /// Build cross-references between functions
    async fn build_cross_references(&self, functions: &[FunctionInfo]) -> Result<Vec<CrossReference>> {
        // TODO: Implement cross-reference analysis
        // - Analyze function calls within code
        // - Track import/export relationships
        // - Find inheritance relationships
        Ok(Vec::new())
    }

    /// Create enhanced display with better formatting
    pub fn create_display(&self, tree: HierarchicalTree) -> HierarchicalTreeDisplay {
        let stats = self.get_tree_stats(&tree);

        let ascii_only = std::env::var("LFT_ASCII_ONLY")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // Create header
        let project_name = std::path::Path::new(&tree.project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown Project")
            .to_string();

        let generated_at_formatted = chrono::DateTime::from_timestamp(tree.generated_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let header = TreeHeader {
            title: if ascii_only {
                "HIERARCHICAL FUNCTION TREE".to_string()
            } else {
                "🌳 HIERARCHICAL FUNCTION TREE".to_string()
            },
            project_name: project_name.clone(),
            repo_id: tree.repo_id.clone(),
            generated_at_formatted,
            summary: if ascii_only {
                format!(
                    "{} files | {} functions | {} languages | Generated from {} analysis",
                    stats.total_files,
                    stats.total_functions,
                    stats.languages.len(),
                    tree.repo_id
                )
            } else {
                format!(
                    "📊 {} files • {} functions • {} languages • Generated from {} analysis",
                    stats.total_files,
                    stats.total_functions,
                    stats.languages.len(),
                    tree.repo_id
                )
            },
        };

        // Create file summary
        let mut file_summary: Vec<FileSummary> = tree.files.values()
            .map(|file| {
                let relative_path = if let Ok(stripped) = std::path::Path::new(&file.file_path)
                    .strip_prefix(&tree.project_path) {
                    stripped.to_string_lossy().to_string()
                } else {
                    std::path::Path::new(&file.file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&file.file_path)
                        .to_string()
                };

                FileSummary {
                    file_path: file.file_path.clone(),
                    language: file.language.clone(),
                    function_count: file.function_count,
                    relative_path,
                }
            })
            .collect();

        file_summary.sort_by(|a, b| b.function_count.cmp(&a.function_count));

        // Create top functions by complexity
        let mut all_functions: Vec<(String, FunctionNode, Option<usize>)> = tree.files.values()
            .flat_map(|file| {
                file.functions.iter().map(|func| {
                    (file.file_path.clone(), func.clone(), func.complexity)
                })
            })
            .collect();

        all_functions.sort_by(|a, b| {
            let a_complexity = a.2.unwrap_or(0);
            let b_complexity = b.2.unwrap_or(0);
            b_complexity.cmp(&a_complexity)
        });

        let top_functions: Vec<TopFunction> = all_functions
            .into_iter()
            .take(15)
            .map(|(file_path, func, _complexity)| TopFunction {
                name: func.name.clone(),
                file_path: file_path.clone(),
                complexity: func.complexity,
                line_range: format!("{}-{}", func.start_line, func.end_line),
                signature_preview: if func.signature.len() > 80 {
                    format!("{}...", &func.signature[..77])
                } else {
                    func.signature.clone()
                },
            })
            .collect();

        HierarchicalTreeDisplay {
            header,
            tree,
            stats,
            file_summary,
            top_functions,
        }
    }

    /// Get tree statistics
    pub fn get_tree_stats(&self, tree: &HierarchicalTree) -> TreeStats {
        let mut languages = HashMap::new();
        let mut total_lines = 0;
        let mut complexity_sum = 0;
        let mut complex_functions = 0;

        for file in tree.files.values() {
            *languages.entry(file.language.clone()).or_insert(0) += 1;

            for func in &file.functions {
                total_lines += func.end_line - func.start_line;
                if let Some(complexity) = func.complexity {
                    complexity_sum += complexity;
                    complex_functions += 1;
                }
            }
        }

        TreeStats {
            total_files: tree.total_files,
            total_functions: tree.total_functions,
            total_lines,
            languages,
            average_complexity: if complex_functions > 0 {
                complexity_sum as f64 / complex_functions as f64
            } else {
                0.0
            },
            cross_references: tree.cross_references.len(),
            generated_at: tree.generated_at,
        }
    }
}

/// Tree statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeStats {
    pub total_files: usize,
    pub total_functions: usize,
    pub total_lines: usize,
    pub languages: HashMap<String, usize>,
    pub average_complexity: f64,
    pub cross_references: usize,
    pub generated_at: i64,
}

/// Enhanced hierarchical tree display with better formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalTreeDisplay {
    pub header: TreeHeader,
    pub tree: HierarchicalTree,
    pub stats: TreeStats,
    pub file_summary: Vec<FileSummary>,
    pub top_functions: Vec<TopFunction>,
}

/// Tree header information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeHeader {
    pub title: String,
    pub project_name: String,
    pub repo_id: String,
    pub generated_at_formatted: String,
    pub summary: String,
}

/// File summary for overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub file_path: String,
    pub language: String,
    pub function_count: usize,
    pub relative_path: String,
}

/// Top function by complexity or importance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopFunction {
    pub name: String,
    pub file_path: String,
    pub complexity: Option<usize>,
    pub line_range: String,
    pub signature_preview: String,
}
