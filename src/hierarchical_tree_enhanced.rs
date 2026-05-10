// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Enhanced Hierarchical Tree - Semantic Module Grouping like Vector Database
//!
//! ENHANCEMENTS:
//! - Semantic module grouping z confidence scores (jak vector database)
//! - Module-level organization: orders, auth, psychological_engine, etc.
//! - Cross-module relationship analysis
//! - Confidence-based ranking per module
//! - Hierarchical output format podobny do vector database results

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use chrono;
use crate::psi_graph::{FunctionInfo, PsiGraphManager, get_global_file_content};

/// Enhanced hierarchical tree z semantic module grouping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedHierarchicalTree {
    pub project_path: String,
    pub repo_id: String,
    pub total_files: usize,
    pub total_functions: usize,
    pub semantic_modules: HashMap<String, ModuleGroup>,  // 🆕 Semantic grouping!
    pub files: HashMap<String, EnhancedFileNode>,
    pub cross_references: Vec<CrossReference>,
    pub module_relationships: Vec<ModuleRelationship>,   // 🆕 Inter-module relationships
    pub generated_at: i64,
}

/// Semantic module group z confidence scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleGroup {
    pub module_name: String,                           // orders, auth, psychological_engine
    pub module_path: String,                           // orders/, auth/, psychological_engine/
    pub confidence_score: f64,                         // 0.0-1.0 semantic confidence
    pub file_count: usize,
    pub function_count: usize,
    pub files: Vec<String>,                            // File paths in this module
    pub primary_functions: Vec<String>,                // Key functions with high confidence
    pub module_purpose: Option<String>,                // AI-inferred purpose description
    pub complexity_score: Option<f64>,                 // Average complexity per module
}

/// Enhanced file node z semantic module info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedFileNode {
    pub file_path: String,
    pub language: String,
    pub function_count: usize,
    pub functions: Vec<EnhancedFunctionNode>,

    // 🆕 SEMANTIC MODULE INFO
    pub module_path: String,                           // semantic/orders/models.py
    pub semantic_group: String,                        // orders, auth, psychological_engine
    pub module_confidence: f64,                        // confidence that file belongs to module
    pub module_role: Option<String>,                   // models, views, services, etc.

    // Original fields
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub file_hash: Option<String>,
    pub last_modified: Option<i64>,
}

/// Enhanced function node z module context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedFunctionNode {
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

    // 🆕 SEMANTIC FUNCTION INFO
    pub semantic_purpose: Option<String>,              // AI-inferred function purpose
    pub business_importance: Option<f64>,              // 0.0-1.0 business logic importance
    pub module_coupling: Vec<String>,                  // Which modules this function interacts with

    // Relationship info
    pub calls_to: Vec<String>,
    pub called_by: Vec<String>,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,

    // CFG analysis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_unreachable_code: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreachable_count: Option<usize>,
}

impl EnhancedFunctionNode {
    /// Get display name with semantic info
    pub fn display_name_with_purpose(&self) -> String {
        let base_name = if self.has_unreachable_code == Some(true) {
            format!("⚠️ {}", self.name)
        } else {
            self.name.clone()
        };

        if let Some(purpose) = &self.semantic_purpose {
            format!("{} - {}", base_name, purpose)
        } else {
            base_name
        }
    }

    /// Get importance indicator
    pub fn importance_indicator(&self) -> String {
        match self.business_importance {
            Some(importance) if importance >= 0.8 => "🔥 High".to_string(),
            Some(importance) if importance >= 0.6 => "⚡ Medium".to_string(),
            Some(importance) if importance >= 0.4 => "📋 Normal".to_string(),
            _ => "📝 Utility".to_string(),
        }
    }
}

/// Inter-module relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleRelationship {
    pub from_module: String,
    pub to_module: String,
    pub relationship_strength: f64,                    // 0.0-1.0 how strongly connected
    pub interaction_count: usize,                      // Number of cross-module calls
    pub relationship_type: ModuleRelationType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleRelationType {
    DataFlow,        // Data passes between modules
    ServiceCall,     // One module calls services of another
    EventTrigger,    // One module triggers events in another
    Dependency,      // One module depends on another
    Composition,     // Modules work together for common goal
}

// Re-export original types for compatibility
use crate::hierarchical_tree::{Parameter, CrossReference, RelationshipType};

/// Enhanced hierarchical tree builder z semantic analysis
pub struct EnhancedHierarchicalTreeBuilder {
    psi_manager: PsiGraphManager,
}

impl EnhancedHierarchicalTreeBuilder {
    /// Create new enhanced hierarchical tree builder
    pub async fn new() -> Result<Self> {
        let psi_manager = PsiGraphManager::new().await?;
        Ok(Self {
            psi_manager,
        })
    }

    /// Build enhanced hierarchical tree z semantic module grouping
    pub async fn build_enhanced_tree_by_repo_id(
        &self,
        repo_id: &str,
        include_context: bool,
        context_lines: usize,
        enable_semantic_analysis: bool,
    ) -> Result<EnhancedHierarchicalTree> {
        let project_path = self.psi_manager.get_project_path(repo_id).await?
            .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found. Run lft_index_project first.", repo_id))?;

        let all_functions = self.psi_manager.get_all_functions(repo_id).await?;

        // 🚀 SEMANTIC MODULE DETECTION
        let semantic_modules = if enable_semantic_analysis {
            self.detect_semantic_modules(&all_functions, &project_path).await?
        } else {
            HashMap::new()
        };

        // Group functions by file z enhanced semantic info
        let mut files = HashMap::new();
        let mut total_functions = 0;

        for func in &all_functions {
            let file_path = &func.file_path;

            // 🆕 DETERMINE SEMANTIC MODULE for this file
            let (semantic_group, module_confidence, module_path, module_role) =
                self.determine_file_module_info(file_path, &semantic_modules);

            let file_node = files.entry(file_path.clone()).or_insert_with(|| EnhancedFileNode {
                file_path: file_path.clone(),
                language: func.language.clone(),
                function_count: 0,
                functions: Vec::new(),

                // 🆕 SEMANTIC MODULE INFO
                module_path: module_path.clone(),
                semantic_group: semantic_group.clone(),
                module_confidence,
                module_role: Some(module_role),

                // Original fields
                imports: Vec::new(),
                exports: Vec::new(),
                file_hash: None,
                last_modified: None,
            });

            // Convert to enhanced function node
            let enhanced_function = self.build_enhanced_function_node(
                func,
                &project_path,
                include_context,
                context_lines,
                enable_semantic_analysis,
                &semantic_group
            ).await?;

            file_node.functions.push(enhanced_function);
            file_node.function_count += 1;
            total_functions += 1;
        }

        // Build inter-module relationships
        let module_relationships = if enable_semantic_analysis {
            self.analyze_module_relationships(&files, &semantic_modules).await?
        } else {
            Vec::new()
        };

        // Build cross-references (unchanged from original)
        let cross_references = self.build_cross_references(&all_functions).await?;

        Ok(EnhancedHierarchicalTree {
            project_path: project_path.to_string(),
            repo_id: repo_id.to_string(),
            total_files: files.len(),
            total_functions,
            semantic_modules,
            files,
            cross_references,
            module_relationships,
            generated_at: chrono::Utc::now().timestamp(),
        })
    }

    /// 🧠 SEMANTIC MODULE DETECTION - core algorithm
    async fn detect_semantic_modules(
        &self,
        functions: &[FunctionInfo],
        project_path: &str,
    ) -> Result<HashMap<String, ModuleGroup>> {
        let mut modules = HashMap::new();
        let mut file_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Collect all unique file paths
        for func in functions {
            file_paths.insert(func.file_path.clone());
        }

        // 🎯 SEMANTIC GROUPING ALGORITHM (podobny do vector database)
        for file_path in file_paths {
            let module_info = self.infer_semantic_module_from_path(&file_path, project_path);

            let module_entry = modules.entry(module_info.0.clone()).or_insert_with(|| ModuleGroup {
                module_name: module_info.0.clone(),
                module_path: module_info.1.clone(),
                confidence_score: 0.0,
                file_count: 0,
                function_count: 0,
                files: Vec::new(),
                primary_functions: Vec::new(),
                module_purpose: None,
                complexity_score: None,
            });

            module_entry.files.push(file_path.clone());
            module_entry.file_count += 1;

            // Update confidence based on path consistency
            module_entry.confidence_score = self.calculate_module_confidence(&module_entry.files);
        }

        // Add function counts and primary functions
        for func in functions {
            if let Some(module_name) = self.determine_function_module(&func.file_path, &modules) {
                if let Some(module) = modules.get_mut(&module_name) {
                    module.function_count += 1;

                    // Mark as primary function if important (heuristic)
                    if self.is_primary_function(&func.name) {
                        module.primary_functions.push(func.name.clone());
                    }
                }
            }
        }

        Ok(modules)
    }

    /// Infer semantic module from file path
    fn infer_semantic_module_from_path(&self, file_path: &str, project_path: &str) -> (String, String) {
        let normalized_path = file_path.replace("\\", "/");
        let path_parts: Vec<&str> = normalized_path.split('/').collect();

        // 🎯 SEMANTIC DETECTION PATTERNS (based on Django taxi analysis)
        for (i, part) in path_parts.iter().enumerate() {
            match *part {
                // Django-style modules
                "orders" | "order" => return ("orders".to_string(), "orders/".to_string()),
                "auth" | "authentication" => return ("auth".to_string(), "auth/".to_string()),
                "psychological_engine" => return ("psychological_engine".to_string(), "psychological_engine/".to_string()),
                "dispatch" | "dispatcher" => return ("dispatch".to_string(), "dispatch/".to_string()),
                "drivers" | "driver" => return ("drivers".to_string(), "drivers/".to_string()),
                "customers" | "customer" => return ("customers".to_string(), "customers/".to_string()),
                "payments" | "payment" => return ("payments".to_string(), "payments/".to_string()),
                "analytics" | "stats" => return ("analytics".to_string(), "analytics/".to_string()),

                // Function-based detection
                part if part.contains("admin") => return ("admin".to_string(), "admin/".to_string()),
                part if part.contains("api") => return ("api".to_string(), "api/".to_string()),
                part if part.contains("core") => return ("core".to_string(), "core/".to_string()),
                part if part.contains("utils") => return ("utils".to_string(), "utils/".to_string()),
                part if part.contains("models") => {
                    // Try to get parent directory as module
                    if i > 0 {
                        return (path_parts[i-1].to_string(), format!("{}/", path_parts[i-1]));
                    } else {
                        return ("models".to_string(), "models/".to_string());
                    }
                }
                _ => {}
            }
        }

        // Fallback: use top-level directory or filename
        if path_parts.len() >= 2 {
            (path_parts[path_parts.len()-2].to_string(), format!("{}/", path_parts[path_parts.len()-2]))
        } else {
            ("root".to_string(), "".to_string())
        }
    }

    /// Determine file's module info
    fn determine_file_module_info(
        &self,
        file_path: &str,
        modules: &HashMap<String, ModuleGroup>,
    ) -> (String, f64, String, String) {
        let (semantic_group, module_path) = self.infer_semantic_module_from_path(file_path, "");

        let module_confidence = modules.get(&semantic_group)
            .map(|m| m.confidence_score)
            .unwrap_or(0.5);

        let module_role = self.determine_file_role(file_path);

        (semantic_group, module_confidence, module_path, module_role)
    }

    /// Determine file role (models, views, services, etc.)
    fn determine_file_role(&self, file_path: &str) -> String {
        let filename = file_path.split('/').last().unwrap_or(file_path);

        if filename.contains("models") { "models".to_string() }
        else if filename.contains("views") { "views".to_string() }
        else if filename.contains("services") { "services".to_string() }
        else if filename.contains("api") { "api".to_string() }
        else if filename.contains("admin") { "admin".to_string() }
        else if filename.contains("utils") { "utils".to_string() }
        else if filename.contains("tests") { "tests".to_string() }
        else { "implementation".to_string() }
    }

    /// Calculate module confidence based on file path consistency
    fn calculate_module_confidence(&self, files: &[String]) -> f64 {
        if files.len() <= 1 {
            return 0.8; // Single file gets reasonable confidence
        }

        // Count how many files follow similar patterns
        let mut pattern_consistency = 0;
        let total_files = files.len();

        for file in files {
            // Check if file follows expected patterns (models.py, views.py, services.py, etc.)
            if file.contains("models") || file.contains("views") ||
               file.contains("services") || file.contains("admin") {
                pattern_consistency += 1;
            }
        }

        // Confidence = consistency ratio + base confidence
        0.5 + (pattern_consistency as f64 / total_files as f64) * 0.4
    }

    /// Check if function is primary/important for module
    fn is_primary_function(&self, func_name: &str) -> bool {
        // Heuristics for important functions (customize per domain)
        func_name.contains("dispatch") || func_name.contains("create") ||
        func_name.contains("process") || func_name.contains("handle") ||
        func_name.contains("manage") || func_name.contains("find") ||
        func_name.starts_with("get_") || func_name.starts_with("set_") ||
        func_name.contains("main") || func_name.contains("core")
    }

    /// Determine which module a function belongs to
    fn determine_function_module(
        &self,
        file_path: &str,
        modules: &HashMap<String, ModuleGroup>,
    ) -> Option<String> {
        for (module_name, module_group) in modules {
            if module_group.files.iter().any(|f| f == file_path) {
                return Some(module_name.clone());
            }
        }
        None
    }

    /// Build enhanced function node z semantic analysis
    async fn build_enhanced_function_node(
        &self,
        func: &FunctionInfo,
        project_path: &str,
        include_context: bool,
        context_lines: usize,
        enable_semantic_analysis: bool,
        semantic_group: &str,
    ) -> Result<EnhancedFunctionNode> {
        // Base function node creation (similar to original)
        let mut context_before = Vec::new();
        let mut context_after = Vec::new();

        if include_context {
            // Get file content and extract context
            if let Some(lines) = get_global_file_content(&func.file_path) {

                // Context before
                let start_idx = func.start_line.saturating_sub(context_lines + 1);
                let end_idx = (func.start_line - 1).min(lines.len());
                for i in start_idx..end_idx {
                    if let Some(line) = lines.get(i) {
                        context_before.push(format!("{}: {}", i + 1, line));
                    }
                }

                // Context after
                let start_idx = func.end_line.min(lines.len());
                let end_idx = (func.end_line + context_lines).min(lines.len());
                for i in start_idx..end_idx {
                    if let Some(line) = lines.get(i) {
                        context_after.push(format!("{}: {}", i + 1, line));
                    }
                }
            }
        }

        // 🧠 SEMANTIC ANALYSIS
        let (semantic_purpose, business_importance, module_coupling) = if enable_semantic_analysis {
            self.analyze_function_semantics(func, semantic_group).await?
        } else {
            (None, None, Vec::new())
        };

        Ok(EnhancedFunctionNode {
            name: func.name.clone(),
            signature: func.signature.clone(),
            start_line: func.start_line,
            end_line: func.end_line,

            // Infer missing fields from signature and name
            visibility: self.infer_visibility(&func.name, &func.signature),
            is_async: func.signature.contains("async"),
            is_static: func.signature.contains("static") || func.signature.contains("@staticmethod"),
            complexity: func.complexity,
            docstring: None, // Could be extracted from context if needed
            parameters: self.extract_parameters_from_signature(&func.signature),
            return_type: self.extract_return_type_from_signature(&func.signature),

            // 🆕 SEMANTIC INFO
            semantic_purpose,
            business_importance,
            module_coupling,

            // Relationships (inferred from signature for now)
            calls_to: self.infer_calls_from_signature(&func.signature),
            called_by: Vec::new(), // Would need cross-reference analysis
            context_before,
            context_after,

            // CFG analysis (from existing FunctionInfo)
            has_unreachable_code: func.has_unreachable_code,
            unreachable_count: func.unreachable_count,
        })
    }

    /// Infer function visibility from name and signature
    fn infer_visibility(&self, name: &str, signature: &str) -> String {
        if name.starts_with("_") || name.starts_with("__") {
            "private".to_string()
        } else if signature.contains("pub ") || signature.contains("public ") {
            "public".to_string()
        } else if signature.contains("protected ") {
            "protected".to_string()
        } else {
            "public".to_string() // Default assumption
        }
    }

    /// Extract parameters from function signature (heuristic)
    fn extract_parameters_from_signature(&self, signature: &str) -> Vec<Parameter> {
        let mut params = Vec::new();

        // Simple heuristic: look for parameters in parentheses
        if let Some(start) = signature.find('(') {
            if let Some(end) = signature.find(')') {
                let params_str = &signature[start+1..end];
                if !params_str.trim().is_empty() {
                    for param in params_str.split(',') {
                        let param = param.trim();
                        if !param.is_empty() && param != "self" {
                            // Simple parameter parsing
                            let parts: Vec<&str> = param.split(':').collect();
                            let name = parts[0].trim().to_string();
                            let param_type = if parts.len() > 1 {
                                Some(parts[1].trim().to_string())
                            } else {
                                None
                            };

                            params.push(Parameter {
                                name,
                                param_type,
                                default_value: None, // Could be extracted with more complex parsing
                                is_optional: param.contains("=") || param.contains("Optional"),
                            });
                        }
                    }
                }
            }
        }

        params
    }

    /// Extract return type from function signature (heuristic)
    fn extract_return_type_from_signature(&self, signature: &str) -> Option<String> {
        // Look for -> return_type
        if let Some(arrow_pos) = signature.find("->") {
            let return_part = &signature[arrow_pos + 2..];
            if let Some(brace_pos) = return_part.find('{') {
                Some(return_part[..brace_pos].trim().to_string())
            } else {
                Some(return_part.trim().to_string())
            }
        } else {
            None
        }
    }

    /// Infer function calls from signature (heuristic)
    fn infer_calls_from_signature(&self, _signature: &str) -> Vec<String> {
        // This would require more sophisticated analysis
        // For now, return empty vector
        Vec::new()
    }

    /// Analyze function semantics for business importance and purpose
    async fn analyze_function_semantics(
        &self,
        func: &FunctionInfo,
        semantic_group: &str,
    ) -> Result<(Option<String>, Option<f64>, Vec<String>)> {
        // 🎯 SEMANTIC PURPOSE INFERENCE
        let semantic_purpose = self.infer_function_purpose(&func.name, &func.signature, semantic_group);

        // 🎯 BUSINESS IMPORTANCE SCORING (0.0-1.0)
        let business_importance = self.calculate_business_importance(&func.name, semantic_group);

        // 🎯 MODULE COUPLING ANALYSIS
        let module_coupling = self.analyze_module_coupling(func);

        Ok((semantic_purpose, Some(business_importance), module_coupling))
    }

    /// Infer function purpose from name and context
    fn infer_function_purpose(&self, name: &str, signature: &str, module: &str) -> Option<String> {
        // Function name patterns to purpose mapping
        let purpose = if name.contains("dispatch") && module == "dispatch" {
            "Core dispatch algorithm"
        } else if name.starts_with("find_") {
            "Search and retrieval logic"
        } else if name.starts_with("create_") || name.starts_with("add_") {
            "Entity creation"
        } else if name.starts_with("update_") || name.starts_with("modify_") {
            "Data modification"
        } else if name.starts_with("delete_") || name.starts_with("remove_") {
            "Entity removal"
        } else if name.starts_with("get_") {
            "Data accessor"
        } else if name.starts_with("set_") {
            "Data mutator"
        } else if name.contains("process") {
            "Business logic processor"
        } else if name.contains("handle") {
            "Event/request handler"
        } else if name.contains("validate") {
            "Input validation"
        } else if name.contains("calculate") || name.contains("compute") {
            "Mathematical computation"
        } else if name.contains("trigger") && module == "psychological_engine" {
            "Psychological trigger activation"
        } else if name.contains("optimize") {
            "Performance optimization"
        } else {
            return None;
        };

        Some(purpose.to_string())
    }

    /// Calculate business importance score
    fn calculate_business_importance(&self, name: &str, module: &str) -> f64 {
        let mut score: f64 = 0.5; // Base score

        // Module-specific importance
        match module {
            "dispatch" | "orders" => score += 0.3, // Core business logic
            "psychological_engine" => score += 0.2, // Revenue optimization
            "payments" => score += 0.25, // Financial critical
            "auth" => score += 0.15, // Security important
            _ => {}
        }

        // Function name importance
        if name.contains("dispatch") || name.contains("assign") {
            score += 0.2; // Core dispatch functions
        } else if name.contains("payment") || name.contains("charge") {
            score += 0.15; // Financial functions
        } else if name.contains("optimize") || name.contains("algorithm") {
            score += 0.1; // Optimization functions
        }

        // Cap at 1.0
        score.min(1.0)
    }

    /// Analyze module coupling
    fn analyze_module_coupling(&self, func: &FunctionInfo) -> Vec<String> {
        let mut coupling = Vec::new();

        // Analyze function signature and name to determine cross-module dependencies
        let combined_text = format!("{} {}", func.name, func.signature);

        // Heuristic: if signature/name contains module-specific keywords
        if combined_text.contains("order") && !func.file_path.contains("order") {
            coupling.push("orders".to_string());
        }
        if combined_text.contains("auth") && !func.file_path.contains("auth") {
            coupling.push("auth".to_string());
        }
        if combined_text.contains("dispatch") && !func.file_path.contains("dispatch") {
            coupling.push("dispatch".to_string());
        }
        if combined_text.contains("payment") && !func.file_path.contains("payment") {
            coupling.push("payments".to_string());
        }

        coupling.sort();
        coupling.dedup();
        coupling
    }

    /// Analyze relationships between modules
    async fn analyze_module_relationships(
        &self,
        files: &HashMap<String, EnhancedFileNode>,
        modules: &HashMap<String, ModuleGroup>,
    ) -> Result<Vec<ModuleRelationship>> {
        let mut relationships = Vec::new();

        // For each module pair, analyze interaction strength
        let module_names: Vec<String> = modules.keys().cloned().collect();

        for i in 0..module_names.len() {
            for j in i+1..module_names.len() {
                let module_a = &module_names[i];
                let module_b = &module_names[j];

                let relationship = self.calculate_module_relationship(module_a, module_b, files, modules).await?;
                if let Some(rel) = relationship {
                    relationships.push(rel);
                }
            }
        }

        Ok(relationships)
    }

    /// Calculate relationship between two modules
    async fn calculate_module_relationship(
        &self,
        module_a: &str,
        module_b: &str,
        files: &HashMap<String, EnhancedFileNode>,
        modules: &HashMap<String, ModuleGroup>,
    ) -> Result<Option<ModuleRelationship>> {
        let mut interaction_count = 0;
        let mut total_coupling_strength = 0.0;

        // Count cross-module interactions
        for file in files.values() {
            if file.semantic_group == module_a {
                for func in &file.functions {
                    for coupled_module in &func.module_coupling {
                        if coupled_module == module_b {
                            interaction_count += 1;
                            total_coupling_strength += 1.0;
                        }
                    }
                }
            }
        }

        if interaction_count == 0 {
            return Ok(None);
        }

        let relationship_strength = (total_coupling_strength / interaction_count as f64).min(1.0);

        // Determine relationship type based on modules
        let relationship_type = match (module_a, module_b) {
            ("orders", "dispatch") | ("dispatch", "orders") => ModuleRelationType::DataFlow,
            ("auth", _) | (_, "auth") => ModuleRelationType::Dependency,
            ("psychological_engine", _) => ModuleRelationType::EventTrigger,
            ("payments", "orders") | ("orders", "payments") => ModuleRelationType::ServiceCall,
            _ => ModuleRelationType::Composition,
        };

        Ok(Some(ModuleRelationship {
            from_module: module_a.to_string(),
            to_module: module_b.to_string(),
            relationship_strength,
            interaction_count,
            relationship_type,
        }))
    }

    /// Build cross-references (reuse from original implementation)
    async fn build_cross_references(&self, functions: &[FunctionInfo]) -> Result<Vec<CrossReference>> {
        // TODO: Implement cross-reference building logic (copy from original)
        // For now, return empty vector
        Ok(Vec::new())
    }
}

/// Format enhanced hierarchical tree z vector database-style output
impl EnhancedHierarchicalTree {
    /// Format output similar to vector database hierarchical grouping
    pub fn format_hierarchical_output(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("🏗️ **HIERARCHICAL PROJECT ANALYSIS** - {}\n", self.project_path));
        output.push_str(&format!("📊 **STATISTICS**: {} files, {} functions, {} semantic modules\n\n",
            self.total_files, self.total_functions, self.semantic_modules.len()));

        // Sort modules by confidence score (highest first)
        let mut sorted_modules: Vec<_> = self.semantic_modules.iter().collect();
        sorted_modules.sort_by(|a, b| b.1.confidence_score.partial_cmp(&a.1.confidence_score).unwrap());

        output.push_str("🎯 **SEMANTIC MODULES** (by confidence):\n");
        for (module_name, module_group) in sorted_modules {
            output.push_str(&format!("  📁 **{}**: {:.3} confidence | {} files | {} functions\n",
                module_name, module_group.confidence_score, module_group.file_count, module_group.function_count));

            if let Some(purpose) = &module_group.module_purpose {
                output.push_str(&format!("     Purpose: {}\n", purpose));
            }

            if !module_group.primary_functions.is_empty() {
                output.push_str(&format!("     Key functions: {}\n", module_group.primary_functions.join(", ")));
            }
            output.push_str("\n");
        }

        // Module relationships
        if !self.module_relationships.is_empty() {
            output.push_str("🔗 **INTER-MODULE RELATIONSHIPS**:\n");
            for rel in &self.module_relationships {
                output.push_str(&format!("  {} ↔️ {}: {:.3} strength ({} interactions) [{}]\n",
                    rel.from_module, rel.to_module, rel.relationship_strength,
                    rel.interaction_count, format!("{:?}", rel.relationship_type)));
            }
            output.push_str("\n");
        }

        // Top functions by importance
        output.push_str("🔥 **HIGH-IMPORTANCE FUNCTIONS**:\n");
        let mut important_functions = Vec::new();
        for file in self.files.values() {
            for func in &file.functions {
                if let Some(importance) = func.business_importance {
                    if importance >= 0.7 {
                        important_functions.push((func, file, importance));
                    }
                }
            }
        }
        important_functions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        for (func, file, importance) in important_functions.iter().take(10) {
            output.push_str(&format!("  🔥 **{}** ({:.2}): {} - {}\n",
                func.name, importance, file.semantic_group,
                func.semantic_purpose.as_deref().unwrap_or("Core function")));
        }

        output
    }
}
