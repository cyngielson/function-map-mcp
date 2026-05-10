// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Function Extractor - Ultra-Brain Compatible Implementation
//!
//! Exact implementation copied from Ultra-Brain for perfect compatibility

// use std::collections::HashMap; // Unused
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImplementationType {
    Full,
    Partial,
    Stub,
    Mock,
    Placeholder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClassType {
    Widget,           // Flutter: StatefulWidget, StatelessWidget
    Service,          // Service classes (ApiService, DatabaseService)
    Model,            // Data models, DTOs, entities
    Controller,       // Controllers, ViewModels, Providers
    Utility,          // Utility classes, helpers
    Interface,        // Abstract classes, protocols, interfaces
    Extension,        // Extension methods, mixins
    Regular,          // Regular classes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyInfo {
    pub name: String,
    pub property_type: String,
    pub access_modifier: String,
    pub is_static: bool,
    pub is_final: bool,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub signature: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub complexity: Option<usize>,
    pub implementation_type: ImplementationType,
    pub has_logic: bool,
    pub contains_todos: bool,
    pub returns_constant: bool,
    pub has_error_handling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassInfo {
    pub name: String,
    pub class_type: ClassType,
    pub signature: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub parent_class: Option<String>,        // Inheritance
    pub implemented_interfaces: Vec<String>, // Interfaces/protocols
    pub methods: Vec<FunctionInfo>,          // Methods within class
    pub properties: Vec<PropertyInfo>,       // Class properties/fields
    pub is_abstract: bool,
    pub is_final: bool,
    pub access_modifier: String,            // public, private, protected
    pub annotations: Vec<String>,           // @override, @immutable, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetInfo {
    pub name: String,
    pub widget_type: String,
    pub signature: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub build_method: Option<FunctionInfo>,
    pub lifecycle_methods: Vec<FunctionInfo>, // initState, dispose, didUpdateWidget
    pub properties: Vec<PropertyInfo>,
}

pub struct FunctionExtractor;

impl FunctionExtractor {
    pub fn new() -> Self {
        FunctionExtractor
    }

    /// Extract functions from project (compatibility method)
    pub async fn extract_from_project(
        &self,
        project_path: &str,
        languages: &[String],
        include_tests: bool,
        max_files: usize,
    ) -> Result<Vec<crate::psi_graph::FunctionInfo>, anyhow::Error> {
        let mut all_functions = Vec::new();

        let walker = walkdir::WalkDir::new(project_path).max_depth(10);
        let mut file_count = 0;

        for entry in walker {
            if file_count >= max_files {
                break;
            }

            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let path_str = path.to_string_lossy();

            // Skip files we don't want to analyze
            if !include_tests && (path_str.contains("test") || path_str.contains("spec")) {
                continue;
            }

            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let lang = match ext {
                    "py" => "python",
                    "rs" => "rust",
                    "js" => "javascript",
                    "ts" => "typescript",
                    "jsx" => "jsx",  // React JSX components
                    "dart" => "dart",
                    "kt" | "kts" => "kotlin",
                    "java" => "java",
                    "tsx" => "tsx",
                    "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "cpp",
                    "c" | "h" => "c",
                    "go" => "go",
                    "jsm" | "mjs" => "mozjs",
                    _ => continue,
                };

                if !languages.contains(&lang.to_string()) && !languages.is_empty() {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(path) {
                    let functions_result = self.extract_functions_ast_with_mobile(
                        &content,
                        lang,
                        &path_str,
                        true, // include_mobile_apps
                    );

                    // Convert to our FunctionInfo format
                    if let Ok(functions) = functions_result {
                        for func in functions {
                        all_functions.push(crate::psi_graph::FunctionInfo {
                            name: func.name,
                            signature: func.signature,
                            file_path: func.file_path,
                            start_line: func.start_line,
                            end_line: func.end_line,
                            language: func.language,
                            complexity: func.complexity,
                            has_unreachable_code: None,
                            unreachable_count: None,
                        });
                        }
                    }

                    file_count += 1;
                }
            }
        }

        Ok(all_functions)
    }

    // 🚀 MAIN EXTRACTION FUNCTION - Exactly like Ultra-Brain
    pub fn extract_functions_ast_with_mobile(
        &self,
        content: &str,
        language: &str,
        file_path: &str,
        include_mobile_apps: bool,
    ) -> Result<Vec<FunctionInfo>, Box<dyn std::error::Error>> {
        let mut functions = Vec::new();

        match language {
            "rust" => self.extract_rust_functions(content, file_path, &mut functions)?,
            "python" => self.extract_python_functions(content, file_path, &mut functions)?,
            "javascript" | "typescript" => self.extract_js_functions(content, file_path, &mut functions)?,
            // 📱 MOBILE APPS SUPPORT
            "dart" if include_mobile_apps => self.extract_dart_functions(content, file_path, &mut functions)?,
            "swift" if include_mobile_apps => self.extract_swift_functions(content, file_path, &mut functions)?,
            "kotlin" => self.extract_kotlin_functions(content, file_path, &mut functions)?,
            "java" => self.extract_java_android_functions(content, file_path, &mut functions)?,
            "tsx" => self.extract_react_functions(content, file_path, &mut functions)?,
            // 🚀 NEW LANGUAGES SUPPORT
            "go" => self.extract_go_functions(content, file_path, &mut functions)?,
            "cpp" => self.extract_cpp_functions(content, file_path, &mut functions)?,
            "c" => self.extract_c_functions(content, file_path, &mut functions)?,
            "mozjs" => self.extract_mozjs_functions(content, file_path, &mut functions)?,
            "jsx" => self.extract_react_functions(content, file_path, &mut functions)?, // JSX handled same as TSX
            _ => self.extract_generic_functions(content, file_path, &mut functions)?,
        }

        Ok(functions)
    }

    // 🐍 PYTHON EXTRACTION WITH INDENTATION ANALYSIS - Exactly like Ultra-Brain
    fn extract_python_functions(
        &self,
        content: &str,
        file_path: &str,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                if let Some(name_end) = trimmed.find('(') {
                    let name_start = if trimmed.starts_with("async def ") { 10 } else { 4 };
                    if let Some(name) = trimmed.get(name_start..name_end) {
                        // 🔍 INDENTATION ANALYSIS - Find function end by looking for next function or class at same indent level
                        let function_indent = line.len() - line.trim_start().len();
                        let mut end_line = i + 1;

                        for (j, next_line) in lines.iter().enumerate().skip(i + 1) {
                            let next_indent = next_line.len() - next_line.trim_start().len();
                            let next_trimmed = next_line.trim();

                            // Stop at same or lower indent level if it's a function, class, or non-empty line
                            if next_indent <= function_indent && !next_trimmed.is_empty() &&
                               (next_trimmed.starts_with("def ") ||
                                next_trimmed.starts_with("async def ") ||
                                next_trimmed.starts_with("class ") ||
                                (!next_trimmed.starts_with("#") && !next_trimmed.starts_with("\""))) {
                                end_line = j;
                                break;
                            }
                            end_line = j + 1;
                        }

                        // ULTRA-BRAIN: Use simplified analysis for Python
                        let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                            (ImplementationType::Full, true, false, false, false);

                        functions.push(FunctionInfo {
                            name: name.trim().to_string(),
                            signature: trimmed.to_string(),
                            file_path: file_path.to_string(),
                            start_line: i + 1,
                            end_line: end_line.max(i + 1), // Better end detection
                            language: "python".to_string(),
                            complexity: Some(1),
                            implementation_type: impl_type,
                            has_logic,
                            contains_todos,
                            returns_constant,
                            has_error_handling,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    // 🦀 RUST EXTRACTION - Exactly like Ultra-Brain
    fn extract_rust_functions(
        &self,
        content: &str,
        file_path: &str,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut function_start = 0;
        let mut current_function = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") || trimmed.starts_with("async fn ") {
                if let Some(name_end) = trimmed.find('(') {
                    let name_start = if trimmed.starts_with("pub fn ") {
                        7
                    } else if trimmed.starts_with("async fn ") {
                        9
                    } else {
                        3
                    };

                    if let Some(name) = trimmed.get(name_start..name_end) {
                        current_function = name.trim().to_string();
                        function_start = i + 1;
                        in_function = true;
                    }
                }
            } else if in_function && trimmed == "}" && !current_function.is_empty() {
                let function_lines = &lines[function_start-1..=i];
                let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                    self.analyze_function_type(function_lines, content);

                functions.push(FunctionInfo {
                    name: current_function.clone(),
                    signature: format!("fn {}(...)", current_function),
                    file_path: file_path.to_string(),
                    start_line: function_start,
                    end_line: i + 1,
                    language: "rust".to_string(),
                    complexity: Some(self.calculate_complexity(function_lines)),
                    implementation_type: impl_type,
                    has_logic,
                    contains_todos,
                    returns_constant,
                    has_error_handling,
                });
                in_function = false;
                current_function.clear();
            }
        }
        Ok(())
    }

    // 🌐 JAVASCRIPT EXTRACTION - Exactly like Ultra-Brain
    fn extract_js_functions(
        &self,
        content: &str,
        file_path: &str,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("function ") || trimmed.contains("=> ") || trimmed.contains("function(") {
                if let Some(name) = self.extract_js_function_name(trimmed) {
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        (ImplementationType::Full, true, false, false, false);

                    functions.push(FunctionInfo {
                        name,
                        signature: trimmed.to_string(),
                        file_path: file_path.to_string(),
                        start_line: i + 1,
                        end_line: i + 10, // Approximation like Ultra-Brain
                        language: "javascript".to_string(),
                        complexity: Some(1),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                }
            }
        }
        Ok(())
    }

    // 📱 DART/FLUTTER EXTRACTION - Exactly like Ultra-Brain
    fn extract_dart_functions(
        &self,
        content: &str,
        file_path: &str,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut brace_count = 0;
        let mut function_start = 0;
        let mut current_function = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect Dart function patterns: void/int/String/Future/Widget functionName() {
            if !in_function && (
                trimmed.contains("void ") || trimmed.contains("int ") ||
                trimmed.contains("String ") || trimmed.contains("Future") ||
                trimmed.contains("Widget ") || trimmed.contains("double ") ||
                trimmed.contains("bool ") || trimmed.contains("List<") ||
                trimmed.contains("Map<") || trimmed.starts_with("static ") ||
                trimmed.contains("@override")
            ) && trimmed.contains("(") && trimmed.contains(")") {

                if let Some(name) = self.extract_dart_function_name(trimmed) {
                    current_function = name;
                    function_start = i + 1;
                    in_function = true;
                    brace_count = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
                }
            } else if in_function {
                brace_count += trimmed.matches('{').count() as i32;
                brace_count -= trimmed.matches('}').count() as i32;

                if brace_count <= 0 {
                    let function_lines = &lines[function_start-1..=i];
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(function_lines, content);

                    functions.push(FunctionInfo {
                        name: current_function.clone(),
                        signature: lines[function_start-1].trim().to_string(),
                        file_path: file_path.to_string(),
                        start_line: function_start,
                        end_line: i + 1,
                        language: "dart".to_string(),
                        complexity: Some(self.calculate_complexity(function_lines)),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                    in_function = false;
                    current_function.clear();
                }
            }
        }
        Ok(())
    }

    // Stub implementations for mobile languages
    fn extract_swift_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut brace_count = 0;
        let mut function_start = 0;
        let mut current_function = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect Swift function patterns:
            // func functionName(), private func, @objc func, init(), etc.
            if !in_function && (
                trimmed.starts_with("func ") ||
                trimmed.starts_with("private func ") ||
                trimmed.starts_with("public func ") ||
                trimmed.starts_with("internal func ") ||
                trimmed.starts_with("fileprivate func ") ||
                trimmed.starts_with("open func ") ||
                trimmed.starts_with("static func ") ||
                trimmed.starts_with("class func ") ||
                trimmed.starts_with("@objc func ") ||
                trimmed.starts_with("@IBAction func ") ||
                trimmed.starts_with("override func ") ||
                trimmed.contains(" func ") || // for modifiers like "@objc private func"
                trimmed.starts_with("init(") || // Swift initializers
                trimmed.starts_with("convenience init(") ||
                trimmed.starts_with("required init(")
            ) && (trimmed.contains("(") || trimmed.starts_with("init")) {

                if let Some(name) = self.extract_swift_function_name(trimmed) {
                    current_function = name;
                    function_start = i + 1;
                    in_function = true;
                    brace_count = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;

                    // Handle single-expression functions (func name() -> Type { return value })
                    if brace_count == 0 && trimmed.contains("->") && trimmed.ends_with("}") {
                        let function_lines = &[trimmed];
                        let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                            self.analyze_function_type(function_lines, content);

                        functions.push(FunctionInfo {
                            name: current_function.clone(),
                            signature: trimmed.to_string(),
                            file_path: file_path.to_string(),
                            start_line: function_start,
                            end_line: i + 1,
                            language: "swift".to_string(),
                            complexity: Some(1),
                            implementation_type: impl_type,
                            has_logic,
                            contains_todos,
                            returns_constant,
                            has_error_handling,
                        });
                        in_function = false;
                        current_function.clear();
                    }
                }
            } else if in_function {
                brace_count += trimmed.matches('{').count() as i32;
                brace_count -= trimmed.matches('}').count() as i32;

                if brace_count <= 0 {
                    let function_lines = &lines[function_start-1..=i];
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(function_lines, content);

                    functions.push(FunctionInfo {
                        name: current_function.clone(),
                        signature: lines[function_start-1].trim().to_string(),
                        file_path: file_path.to_string(),
                        start_line: function_start,
                        end_line: i + 1,
                        language: "swift".to_string(),
                        complexity: Some(self.calculate_complexity(function_lines)),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                    in_function = false;
                    current_function.clear();
                }
            }
        }
        Ok(())
    }

    fn extract_kotlin_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut brace_count = 0;
        let mut function_start = 0;
        let mut current_function = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect Kotlin function patterns:
            // fun functionName(), private fun, suspend fun, inline fun, etc.
            if !in_function && (
                trimmed.starts_with("fun ") ||
                trimmed.starts_with("private fun ") ||
                trimmed.starts_with("public fun ") ||
                trimmed.starts_with("protected fun ") ||
                trimmed.starts_with("internal fun ") ||
                trimmed.starts_with("suspend fun ") ||
                trimmed.starts_with("inline fun ") ||
                trimmed.starts_with("override fun ") ||
                trimmed.contains(" fun ") // for modifiers like "private suspend fun"
            ) && trimmed.contains("(") {

                if let Some(name) = self.extract_kotlin_function_name(trimmed) {
                    current_function = name;
                    function_start = i + 1;
                    in_function = true;
                    brace_count = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;

                    // Handle single-expression functions (fun name() = expression)
                    if trimmed.contains(" = ") && !trimmed.contains("{") {
                        let function_lines = &[trimmed];
                        let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                            self.analyze_function_type(function_lines, content);

                        functions.push(FunctionInfo {
                            name: current_function.clone(),
                            signature: trimmed.to_string(),
                            file_path: file_path.to_string(),
                            start_line: function_start,
                            end_line: i + 1,
                            language: "kotlin".to_string(),
                            complexity: Some(1),
                            implementation_type: impl_type,
                            has_logic,
                            contains_todos,
                            returns_constant,
                            has_error_handling,
                        });
                        in_function = false;
                        current_function.clear();
                    }
                }
            } else if in_function {
                brace_count += trimmed.matches('{').count() as i32;
                brace_count -= trimmed.matches('}').count() as i32;

                if brace_count <= 0 {
                    let function_lines = &lines[function_start-1..=i];
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(function_lines, content);

                    functions.push(FunctionInfo {
                        name: current_function.clone(),
                        signature: lines[function_start-1].trim().to_string(),
                        file_path: file_path.to_string(),
                        start_line: function_start,
                        end_line: i + 1,
                        language: "kotlin".to_string(),
                        complexity: Some(self.calculate_complexity(function_lines)),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                    in_function = false;
                    current_function.clear();
                }
            }
        }
        Ok(())
    }

    fn extract_java_android_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_method = false;
        let mut brace_count = 0;
        let mut method_start = 0;
        let mut current_method = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect Java method patterns:
            // public void methodName(), private static int, @Override protected, etc.
            if !in_method && (
                (trimmed.contains("public ") ||
                 trimmed.contains("private ") ||
                 trimmed.contains("protected ") ||
                 trimmed.contains("static ") ||
                 trimmed.starts_with("@Override") ||
                 (trimmed.contains("void ") || trimmed.contains("int ") ||
                  trimmed.contains("String ") || trimmed.contains("boolean ") ||
                  trimmed.contains("double ") || trimmed.contains("float ") ||
                  trimmed.contains("long ") || trimmed.contains("List<") ||
                  trimmed.contains("Map<") || trimmed.contains("Set<") ||
                  trimmed.contains("Optional<") || trimmed.contains("Future<")))
                && trimmed.contains("(") && trimmed.contains(")") &&
                !trimmed.contains("class ") && !trimmed.contains("interface ") &&
                !trimmed.contains("enum ") && !trimmed.contains("=") // exclude field declarations
            ) ||
            // Handle constructor methods (same name as class)
            (trimmed.contains("public ") && trimmed.contains("(") &&
             !trimmed.contains(" class ") && !trimmed.contains("void ") &&
             !trimmed.contains("int ") && !trimmed.contains("String ")) {

                if let Some(name) = self.extract_java_method_name(trimmed) {
                    current_method = name;
                    method_start = i + 1;
                    in_method = true;
                    brace_count = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;

                    // Handle abstract methods (no body)
                    if trimmed.ends_with(";") {
                        let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                            (ImplementationType::Stub, false, false, false, false);

                        functions.push(FunctionInfo {
                            name: current_method.clone(),
                            signature: trimmed.to_string(),
                            file_path: file_path.to_string(),
                            start_line: method_start,
                            end_line: i + 1,
                            language: "java".to_string(),
                            complexity: Some(1),
                            implementation_type: impl_type,
                            has_logic,
                            contains_todos,
                            returns_constant,
                            has_error_handling,
                        });
                        in_method = false;
                        current_method.clear();
                    }
                }
            } else if in_method {
                brace_count += trimmed.matches('{').count() as i32;
                brace_count -= trimmed.matches('}').count() as i32;

                if brace_count <= 0 {
                    let method_lines = &lines[method_start-1..=i];
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(method_lines, content);

                    functions.push(FunctionInfo {
                        name: current_method.clone(),
                        signature: lines[method_start-1].trim().to_string(),
                        file_path: file_path.to_string(),
                        start_line: method_start,
                        end_line: i + 1,
                        language: "java".to_string(),
                        complexity: Some(self.calculate_complexity(method_lines)),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                    in_method = false;
                    current_method.clear();
                }
            }
        }
        Ok(())
    }

    fn extract_react_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // React function component patterns:
            // const ComponentName = () => {
            // function ComponentName() {
            // export const ComponentName = (props) => (
            // const useCustomHook = () => {
            if (trimmed.starts_with("const ") && (trimmed.contains("= () =>") || trimmed.contains("= (") && trimmed.contains(") =>"))) ||
               (trimmed.starts_with("export const ") && (trimmed.contains("= () =>") || trimmed.contains("= (") && trimmed.contains(") =>"))) ||
               (trimmed.starts_with("function ") && trimmed.contains("(")) ||
               (trimmed.starts_with("export function ") && trimmed.contains("(")) ||
               // React hooks pattern (useXxx)
               (trimmed.starts_with("const use") && trimmed.contains("= (")) ||
               // JSX return patterns
               (trimmed.contains("return (") && (i > 0 && lines[i-1].trim().contains("=>"))) {

                if let Some(name) = self.extract_tsx_function_name(trimmed) {
                    // Check if it's a React component (starts with uppercase) or hook (starts with 'use')
                    let is_component = name.chars().next().map_or(false, |c| c.is_uppercase()) || name.starts_with("use");

                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_tsx_function(trimmed, content);

                    functions.push(FunctionInfo {
                        name,
                        signature: trimmed.to_string(),
                        file_path: file_path.to_string(),
                        start_line: i + 1,
                        end_line: i + 10, // Approximation - TSX functions can be complex
                        language: "tsx".to_string(),
                        complexity: Some(if is_component { 2 } else { 1 }),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                }
            }
        }
        Ok(())
    }

    // 🐹 GO LANGUAGE EXTRACTION
    fn extract_go_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut brace_count = 0;
        let mut function_start = 0;
        let mut current_function = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect Go function patterns:
            // func functionName(), func (receiver Type) methodName(), func main()
            if !in_function && trimmed.starts_with("func ") && trimmed.contains("(") {
                if let Some(name) = self.extract_go_function_name(trimmed) {
                    current_function = name;
                    function_start = i + 1;
                    in_function = true;
                    brace_count = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
                }
            } else if in_function {
                brace_count += trimmed.matches('{').count() as i32;
                brace_count -= trimmed.matches('}').count() as i32;

                if brace_count <= 0 {
                    let function_lines = &lines[function_start-1..=i];
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(function_lines, content);

                    functions.push(FunctionInfo {
                        name: current_function.clone(),
                        signature: lines[function_start-1].trim().to_string(),
                        file_path: file_path.to_string(),
                        start_line: function_start,
                        end_line: i + 1,
                        language: "go".to_string(),
                        complexity: Some(self.calculate_complexity(function_lines)),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                    in_function = false;
                    current_function.clear();
                }
            }
        }
        Ok(())
    }

    // 🔧 C/C++ LANGUAGE EXTRACTION
    fn extract_cpp_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut brace_count = 0;
        let mut function_start = 0;
        let mut current_function = String::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Skip preprocessor directives, comments, and class/struct declarations
            if trimmed.starts_with("#") || trimmed.starts_with("//") ||
               trimmed.starts_with("/*") || trimmed.starts_with("class ") ||
               trimmed.starts_with("struct ") || trimmed.starts_with("namespace ") {
                continue;
            }

            // Detect C/C++ function patterns:
            // return_type function_name(params) { or return_type* function_name(params) {
            if !in_function && trimmed.contains("(") && trimmed.contains(")") &&
               (trimmed.contains("int ") || trimmed.contains("void ") || trimmed.contains("char ") ||
                trimmed.contains("float ") || trimmed.contains("double ") || trimmed.contains("bool ") ||
                trimmed.contains("string ") || trimmed.contains("std::") ||
                // C++ specific types
                trimmed.contains("vector<") || trimmed.contains("map<") || trimmed.contains("auto ") ||
                // Function pointers and templates
                trimmed.contains("*") || trimmed.contains("<") && trimmed.contains(">")) &&
               !trimmed.contains("=") && !trimmed.contains(";") { // exclude declarations and assignments

                if let Some(name) = self.extract_cpp_function_name(trimmed) {
                    current_function = name;
                    function_start = i + 1;
                    in_function = true;
                    brace_count = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
                }
            } else if in_function {
                brace_count += trimmed.matches('{').count() as i32;
                brace_count -= trimmed.matches('}').count() as i32;

                if brace_count <= 0 {
                    let function_lines = &lines[function_start-1..=i];
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(function_lines, content);

                    functions.push(FunctionInfo {
                        name: current_function.clone(),
                        signature: lines[function_start-1].trim().to_string(),
                        file_path: file_path.to_string(),
                        start_line: function_start,
                        end_line: i + 1,
                        language: "cpp".to_string(),
                        complexity: Some(self.calculate_complexity(function_lines)),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                    in_function = false;
                    current_function.clear();
                }
            }
        }
        Ok(())
    }

    // 🔧 C LANGUAGE EXTRACTION (separate from C++)
    fn extract_c_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Skip preprocessor directives and comments
            if trimmed.starts_with("#") || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Detect C function patterns (simpler than C++)
            if trimmed.contains("(") && trimmed.contains(")") &&
               (trimmed.contains("int ") || trimmed.contains("void ") || trimmed.contains("char ") ||
                trimmed.contains("float ") || trimmed.contains("double ") || trimmed.contains("static ")) &&
               !trimmed.contains("=") && !trimmed.contains(";") &&
               !trimmed.contains("struct ") && !trimmed.contains("typedef ") {

                if let Some(name) = self.extract_c_function_name(trimmed) {
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_function_type(&[trimmed], content);

                    functions.push(FunctionInfo {
                        name,
                        signature: trimmed.to_string(),
                        file_path: file_path.to_string(),
                        start_line: i + 1,
                        end_line: i + 10, // Approximation
                        language: "c".to_string(),
                        complexity: Some(1),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                }
            }
        }
        Ok(())
    }

    // 🦊 MOZILLA JAVASCRIPT EXTRACTION
    fn extract_mozjs_functions(&self, content: &str, file_path: &str, functions: &mut Vec<FunctionInfo>) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Mozilla-specific JavaScript patterns:
            // Components.utils.import, Cu.import, ChromeUtils.import
            // XPCOM interfaces, chrome:// URLs
            if trimmed.contains("function") || trimmed.contains("=>") ||
               trimmed.contains("Cu.import") || trimmed.contains("ChromeUtils.import") ||
               trimmed.contains("Components.utils") || trimmed.contains("XPCOM") ||
               (trimmed.contains("var ") && trimmed.contains(" = ")) ||
               (trimmed.contains("let ") && trimmed.contains(" = ")) ||
               (trimmed.contains("const ") && trimmed.contains(" = ")) {

                if let Some(name) = self.extract_mozjs_function_name(trimmed) {
                    let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                        self.analyze_mozjs_function(trimmed, content);

                    functions.push(FunctionInfo {
                        name,
                        signature: trimmed.to_string(),
                        file_path: file_path.to_string(),
                        start_line: i + 1,
                        end_line: i + 5, // Mozilla JS functions tend to be shorter
                        language: "mozjs".to_string(),
                        complexity: Some(1),
                        implementation_type: impl_type,
                        has_logic,
                        contains_todos,
                        returns_constant,
                        has_error_handling,
                    });
                }
            }
        }
        Ok(())
    }

    fn extract_generic_functions(
        &self,
        content: &str,
        file_path: &str,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if line.contains("function") || line.contains("def ") || line.contains("fn ") {
                let (impl_type, has_logic, contains_todos, returns_constant, has_error_handling) =
                    (ImplementationType::Placeholder, false, false, false, false);

                functions.push(FunctionInfo {
                    name: "function".to_string(),
                    signature: line.trim().to_string(),
                    file_path: file_path.to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    language: "unknown".to_string(),
                    complexity: Some(1),
                    implementation_type: impl_type,
                    has_logic,
                    contains_todos,
                    returns_constant,
                    has_error_handling,
                });
            }
        }
        Ok(())
    }

    // 🔍 HELPER FUNCTIONS
    fn extract_js_function_name(&self, line: &str) -> Option<String> {
        if line.trim().starts_with("function ") {
            if let Some(start) = line.find("function ") {
                if let Some(end) = line[start + 9..].find('(') {
                    return Some(line[start + 9..start + 9 + end].trim().to_string());
                }
            }
        } else if line.contains("const ") && line.contains("= ") && (line.contains("=> ") || line.contains("function")) {
            if let Some(start) = line.find("const ") {
                if let Some(end) = line[start + 6..].find(' ') {
                    return Some(line[start + 6..start + 6 + end].trim().to_string());
                }
            }
        }
        None
    }

    fn extract_dart_function_name(&self, line: &str) -> Option<String> {
        // For Dart: "Widget build(BuildContext context)" -> "build"
        if let Some(paren_pos) = line.find('(') {
            let before_paren = &line[..paren_pos];
            if let Some(space_pos) = before_paren.rfind(' ') {
                let name = before_paren[space_pos + 1..].trim();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn extract_kotlin_function_name(&self, line: &str) -> Option<String> {
        // For Kotlin: "fun functionName(params): ReturnType" -> "functionName"
        // Handle: "private suspend fun fetchData(id: Int): String" -> "fetchData"
        if let Some(fun_pos) = line.find("fun ") {
            let after_fun = &line[fun_pos + 4..];
            if let Some(paren_pos) = after_fun.find('(') {
                let name_part = after_fun[..paren_pos].trim();
                // Remove generic parameters like <T>
                let name = if let Some(generic_start) = name_part.find('<') {
                    name_part[..generic_start].trim()
                } else {
                    name_part
                };
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn extract_java_method_name(&self, line: &str) -> Option<String> {
        // For Java: "public void methodName(String param)" -> "methodName"
        // Handle: "private static int calculateValue(int x, int y)" -> "calculateValue"
        if let Some(paren_pos) = line.find('(') {
            let before_paren = &line[..paren_pos];

            // Split by spaces and find the method name (last word before parentheses)
            let parts: Vec<&str> = before_paren.split_whitespace().collect();
            if let Some(&method_name) = parts.last() {
                // Remove generic parameters if present
                let name = if let Some(generic_start) = method_name.find('<') {
                    &method_name[..generic_start]
                } else {
                    method_name
                };

                if !name.is_empty() &&
                   name.chars().all(|c| c.is_alphanumeric() || c == '_') &&
                   name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn extract_tsx_function_name(&self, line: &str) -> Option<String> {
        // For TSX: "const ComponentName = () =>" -> "ComponentName"
        // "export const useCustomHook = (param) =>" -> "useCustomHook"
        // "function ComponentName() {" -> "ComponentName"

        if line.starts_with("function ") || line.starts_with("export function ") {
            // Handle function declarations
            let start_pos = if line.starts_with("export function ") { 16 } else { 9 };
            if let Some(paren_pos) = line[start_pos..].find('(') {
                let name = line[start_pos..start_pos + paren_pos].trim();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(name.to_string());
                }
            }
        } else if line.contains("const ") && line.contains(" = ") {
            // Handle const declarations
            let const_start = if line.starts_with("export const ") { 13 } else { 6 };
            if let Some(equals_pos) = line[const_start..].find(" = ") {
                let name = line[const_start..const_start + equals_pos].trim();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn analyze_tsx_function(&self, line: &str, _content: &str) -> (ImplementationType, bool, bool, bool, bool) {
        let contains_todos = line.to_lowercase().contains("todo") || line.contains("TODO");
        let returns_constant = line.contains("return null") || line.contains("return false") || line.contains("return true");
        let has_error_handling = line.contains("try") || line.contains("catch") || line.contains("throw");
        let has_logic = line.contains("if") || line.contains("map") || line.contains("filter") || line.contains("useState") || line.contains("useEffect");

        let impl_type = if contains_todos {
            ImplementationType::Placeholder
        } else if returns_constant {
            ImplementationType::Stub
        } else if has_logic {
            ImplementationType::Full
        } else {
            ImplementationType::Partial
        };

        (impl_type, has_logic, contains_todos, returns_constant, has_error_handling)
    }

    fn extract_go_function_name(&self, line: &str) -> Option<String> {
        // For Go: "func functionName(params) returnType {" -> "functionName"
        // "func (receiver Type) methodName(params) {" -> "methodName"
        if let Some(func_pos) = line.find("func ") {
            let after_func = &line[func_pos + 5..];

            // Check if it's a method (has receiver)
            if after_func.starts_with("(") {
                // Method: func (r Type) methodName(params)
                if let Some(receiver_end) = after_func.find(") ") {
                    let after_receiver = &after_func[receiver_end + 2..];
                    if let Some(paren_pos) = after_receiver.find('(') {
                        let method_name = after_receiver[..paren_pos].trim();
                        if !method_name.is_empty() && method_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            return Some(method_name.to_string());
                        }
                    }
                }
            } else {
                // Function: func functionName(params)
                if let Some(paren_pos) = after_func.find('(') {
                    let func_name = after_func[..paren_pos].trim();
                    if !func_name.is_empty() && func_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return Some(func_name.to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_cpp_function_name(&self, line: &str) -> Option<String> {
        // For C++: "int functionName(params) {" -> "functionName"
        // "std::vector<int> processData(const Data& data) {" -> "processData"
        if let Some(paren_pos) = line.find('(') {
            let before_paren = &line[..paren_pos];

            // Split by spaces, colons, and other delimiters to find function name
            let parts: Vec<&str> = before_paren.split(&[' ', '*', '&', ':', '<', '>'][..]).collect();
            if let Some(&func_name) = parts.iter().rev().find(|&&part|
                !part.is_empty() &&
                part.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') &&
                part.chars().all(|c| c.is_alphanumeric() || c == '_') &&
                // Filter out common C++ keywords and types
                !matches!(part, "int" | "void" | "char" | "float" | "double" | "bool" |
                         "static" | "const" | "virtual" | "inline" | "auto" |
                         "public" | "private" | "protected" | "std" | "vector" |
                         "map" | "set" | "string" | "shared_ptr" | "unique_ptr")
            ) {
                return Some(func_name.to_string());
            }
        }
        None
    }

    fn extract_c_function_name(&self, line: &str) -> Option<String> {
        // For C: "int functionName(params) {" -> "functionName"
        if let Some(paren_pos) = line.find('(') {
            let before_paren = &line[..paren_pos];
            let parts: Vec<&str> = before_paren.split_whitespace().collect();
            if let Some(&func_name) = parts.last() {
                if !func_name.is_empty() &&
                   func_name.chars().all(|c| c.is_alphanumeric() || c == '_') &&
                   func_name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') &&
                   !matches!(func_name, "int" | "void" | "char" | "float" | "double" | "static" | "const") {
                    return Some(func_name.to_string());
                }
            }
        }
        None
    }

    fn extract_mozjs_function_name(&self, line: &str) -> Option<String> {
        // For Mozilla JS: handle XPCOM and Firefox-specific patterns
        if line.contains("function ") {
            // Standard function declaration
            if let Some(func_pos) = line.find("function ") {
                let after_func = &line[func_pos + 9..];
                if let Some(paren_pos) = after_func.find('(') {
                    let func_name = after_func[..paren_pos].trim();
                    if !func_name.is_empty() && func_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return Some(func_name.to_string());
                    }
                }
            }
        } else if line.contains(" = ") {
            // Variable assignment: var/let/const name = function
            if let Some(equals_pos) = line.find(" = ") {
                let before_equals = &line[..equals_pos];
                let parts: Vec<&str> = before_equals.split_whitespace().collect();
                if let Some(&var_name) = parts.last() {
                    if !var_name.is_empty() && var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return Some(var_name.to_string());
                    }
                }
            }
        }
        None
    }

    fn analyze_mozjs_function(&self, line: &str, _content: &str) -> (ImplementationType, bool, bool, bool, bool) {
        let contains_todos = line.to_lowercase().contains("todo") || line.contains("TODO");
        let returns_constant = line.contains("return null") || line.contains("return false") || line.contains("return true");
        let has_error_handling = line.contains("try") || line.contains("catch") || line.contains("throw");
        let has_logic = line.contains("if") || line.contains("Cu.import") || line.contains("Components") || line.contains("chrome://");

        let impl_type = if contains_todos {
            ImplementationType::Placeholder
        } else if returns_constant {
            ImplementationType::Stub
        } else if has_logic {
            ImplementationType::Full
        } else {
            ImplementationType::Partial
        };

        (impl_type, has_logic, contains_todos, returns_constant, has_error_handling)
    }

    fn extract_swift_function_name(&self, line: &str) -> Option<String> {
        // For Swift: "func functionName(params) -> ReturnType {" -> "functionName"
        // "private func methodName() {" -> "methodName"
        // "init(params) {" -> "init"
        // "@objc func buttonTapped(_ sender: UIButton)" -> "buttonTapped"

        if line.contains("init(") {
            // Swift initializer
            return Some("init".to_string());
        }

        if let Some(func_pos) = line.find("func ") {
            let after_func = &line[func_pos + 5..];
            if let Some(paren_pos) = after_func.find('(') {
                let name_part = after_func[..paren_pos].trim();
                // Remove generic parameters like <T> and handle Swift-specific patterns
                let name = if let Some(generic_start) = name_part.find('<') {
                    name_part[..generic_start].trim()
                } else {
                    name_part
                };

                // Handle Swift method names with parameters like "buttonTapped(_:"
                let clean_name = if let Some(underscore_pos) = name.find('(') {
                    &name[..underscore_pos]
                } else if let Some(underscore_pos) = name.find('_') {
                    &name[..underscore_pos]
                } else {
                    name
                };

                if !clean_name.is_empty() && clean_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(clean_name.to_string());
                }
            }
        }
        None
    }    // 📊 ANALYSIS FUNCTIONS - Exactly like Ultra-Brain
    fn analyze_function_type(
        &self,
        function_lines: &[&str],
        _content: &str,
    ) -> (ImplementationType, bool, bool, bool, bool) {
        let function_body = function_lines.join("\n");

        let contains_todos = function_body.to_lowercase().contains("todo")
            || function_body.to_lowercase().contains("fixme")
            || function_body.contains("TODO")
            || function_body.contains("FIXME");

        let returns_constant = function_body.contains("return ") &&
            (function_body.contains("return true") ||
             function_body.contains("return false") ||
             function_body.contains("return None") ||
             function_body.contains("return null") ||
             function_body.matches("return").count() == 1);

        let has_error_handling = function_body.contains("try") ||
            function_body.contains("catch") ||
            function_body.contains("except") ||
            function_body.contains("Result<") ||
            function_body.contains("?");

        let has_logic = function_lines.len() > 3 &&
            (function_body.contains("if ") ||
             function_body.contains("for ") ||
             function_body.contains("while ") ||
             function_body.contains("match ") ||
             function_body.contains("=") ||
             function_body.contains("."));

        let impl_type = if contains_todos {
            ImplementationType::Placeholder
        } else if returns_constant && function_lines.len() <= 3 {
            ImplementationType::Stub
        } else if !has_logic && function_lines.len() <= 5 {
            ImplementationType::Partial
        } else if function_body.contains("panic!") || function_body.contains("unimplemented!") {
            ImplementationType::Mock
        } else {
            ImplementationType::Full
        };

        (impl_type, has_logic, contains_todos, returns_constant, has_error_handling)
    }

    fn calculate_complexity(&self, function_lines: &[&str]) -> usize {
        let function_body = function_lines.join("\n");
        let mut complexity = 1; // Base complexity

        // Count complexity-increasing constructs
        complexity += function_body.matches("if ").count();
        complexity += function_body.matches("else if ").count();
        complexity += function_body.matches("for ").count();
        complexity += function_body.matches("while ").count();
        complexity += function_body.matches("match ").count();
        complexity += function_body.matches("case ").count();
        complexity += function_body.matches("&&").count();
        complexity += function_body.matches("||").count();

        complexity
    }

    pub fn detect_language_from_extension(file_path: &str) -> Option<&'static str> {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            "py" => Some("python"),
            "rs" => Some("rust"),
            "js" => Some("javascript"),
            "ts" => Some("typescript"),
            "dart" => Some("dart"),
            "swift" => Some("swift"),
            "kt" | "kts" => Some("kotlin"),
            "java" => Some("java"),
            "jsx" => Some("jsx"),
            "tsx" => Some("tsx"),
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" => Some("cpp"),
            "c" | "h" => Some("c"),
            "go" => Some("go"),
            "jsm" | "mjs" => Some("mozjs"),
            _ => None,
        }
    }
}
