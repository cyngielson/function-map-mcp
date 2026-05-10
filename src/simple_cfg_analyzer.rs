// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Simple Control Flow Graph (CFG) Analyzer
//!
//! Lightweight CFG analysis using line-based parsing for unreachable code detection.
//! Focus: Detect code after return/panic statements.
//!
//! Phase 1: Simple line-based approach (this file)
//! Phase 2: Full AST-based CFG with branches (future)

use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};
use log::debug;

/// Control Flow Analysis Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgAnalysisResult {
    pub function_name: String,
    pub file_path: String,
    pub total_lines: usize,
    pub has_unreachable_code: bool,
    pub unreachable_issues: Vec<UnreachableCodeIssue>,
    pub return_points: Vec<ReturnPoint>,
}

/// Unreachable code issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreachableCodeIssue {
    pub reason: String,
    pub location: String,
    pub line_number: usize,
    pub code: String,
    pub severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_before: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_after: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    High,    // Code after return/panic
    Medium,  // Code after break/continue
    Low,     // Other potential issues
}

/// Return point information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnPoint {
    pub line: usize,
    pub expression: String,
    pub is_implicit: bool,
}

/// Simple CFG Analyzer
pub struct SimpleCfgAnalyzer;

impl SimpleCfgAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze function for unreachable code with scope awareness
    ///
    /// Enhanced algorithm:
    /// 1. Track scope levels (Python indentation, {} blocks, try/except)
    /// 2. Find return statements within their scope context
    /// 3. Only report unreachable code in the SAME scope level
    /// 4. Handle decorator patterns and nested functions properly
    pub fn analyze_function(
        &self,
        function_name: &str,
        file_path: &str,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> Result<CfgAnalysisResult> {
        debug!("Analyzing CFG for {} (lines {}-{})", function_name, start_line, end_line);

        let lines: Vec<&str> = content.lines().collect();
        let mut unreachable_issues = Vec::new();
        let mut return_points = Vec::new();

        // Enhanced scope tracking
        let mut scope_stack = Vec::new();
        let mut return_scopes = Vec::new(); // Track returns with their scope levels
        let mut conditional_returns = Vec::new(); // Track conditional returns (if/while/for)
        let is_python = file_path.ends_with(".py");

        // Multi-line statement tracking
        let mut in_multiline_return = false;
        let mut multiline_return_start = 0;

        // Analyze lines within function
        for line_idx in start_line..=end_line.min(lines.len()) {
            if line_idx == 0 || line_idx > lines.len() {
                continue;
            }

            let line = lines[line_idx - 1];
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#") {
                continue;
            }

            // Calculate current scope level
            let current_scope = if is_python {
                self.calculate_python_indent_level(line)
            } else {
                self.calculate_brace_scope_level(&lines, line_idx - 1, start_line - 1, end_line - 1)
            };

            // Handle scope changes and exception blocks
            if self.is_new_scope_start(trimmed) {
                scope_stack.push(current_scope);
            }

            // Reset return tracking if this is exception handler (except/catch/finally)
            if self.is_exception_handler(trimmed) {
                // Exception handlers are completely new execution paths - clear ALL returns
                return_scopes.clear();
                conditional_returns.clear();
                debug!("Exception handler detected at line {}: clearing all return tracking", line_idx);
            }

            // Handle multi-line statement continuation
            if in_multiline_return {
                let is_complete_now = self.is_complete_statement(trimmed);
                if is_complete_now {
                    // Multi-line return is now complete
                    in_multiline_return = false;

                    // Process the return logic now
                    let current_scope = if is_python {
                        self.calculate_python_indent_level(lines[multiline_return_start - 1])
                    } else {
                        self.calculate_brace_scope_level(&lines, multiline_return_start - 1, start_line - 1, end_line - 1)
                    };

                    let is_conditional = self.is_inside_conditional_block(&lines, multiline_return_start - 1, start_line - 1);

                    if is_conditional {
                        conditional_returns.push((multiline_return_start, current_scope));
                    } else {
                        return_scopes.push((multiline_return_start, current_scope));
                    }
                }
                continue;
            }

            // Check for return statement
            if self.is_return_statement(trimmed) {
                // Check if this is a complete return statement or multi-line
                let is_complete_return = self.is_complete_statement(trimmed);

                let expr = self.extract_return_expression(trimmed);
                return_points.push(ReturnPoint {
                    line: line_idx,
                    expression: expr,
                    is_implicit: false,
                });

                if is_complete_return {
                    // Complete single-line return - process immediately
                    let is_conditional = self.is_inside_conditional_block(&lines, line_idx - 1, start_line - 1);

                    if is_conditional {
                        conditional_returns.push((line_idx, current_scope));
                    } else {
                        return_scopes.push((line_idx, current_scope));
                    }
                } else {
                    // Multi-line return - start tracking
                    in_multiline_return = true;
                    multiline_return_start = line_idx;
                }
                continue;
            }            // Check for panic/unreachable
            if self.is_panic_statement(trimmed) {
                // Similar logic for panic - check if conditional
                let is_conditional = self.is_inside_conditional_block(&lines, line_idx - 1, start_line - 1);

                if is_conditional {
                    conditional_returns.push((line_idx, current_scope));
                } else {
                    return_scopes.push((line_idx, current_scope));
                }
                continue;
            }

            // Check for unreachable code - only within SAME scope as return
            for (return_line, return_scope) in &return_scopes {
                if current_scope == *return_scope && line_idx > *return_line {
                    // Make sure this isn't a new scope (try/except/def/class)
                    if !self.is_new_scope_start(trimmed) && !self.is_scope_end(trimmed) {
                        unreachable_issues.push(UnreachableCodeIssue {
                            reason: format!("Code after return/panic on line {} (same scope level)", return_line),
                            location: format!("line {}", line_idx),
                            line_number: line_idx,
                            code: trimmed.to_string(),
                            severity: Severity::High,
                            context_before: None,
                            context_after: None,
                        });
                        break; // Only report once per line
                    }
                }
            }

            // Handle scope ends
            if self.is_scope_end(trimmed) && !scope_stack.is_empty() {
                scope_stack.pop();
            }
        }

        // Check for implicit return
        if return_points.is_empty() {
            return_points.push(ReturnPoint {
                line: end_line,
                expression: "()".to_string(),
                is_implicit: true,
            });
        }

        Ok(CfgAnalysisResult {
            function_name: function_name.to_string(),
            file_path: file_path.to_string(),
            total_lines: end_line - start_line + 1,
            has_unreachable_code: !unreachable_issues.is_empty(),
            unreachable_issues,
            return_points,
        })
    }

    /// Check if a statement is complete (not multi-line)
    /// Looks for unclosed parentheses, brackets, or braces
    fn is_complete_statement(&self, line: &str) -> bool {
        let mut paren_count = 0;
        let mut bracket_count = 0;
        let mut brace_count = 0;
        let mut in_string = false;
        let mut string_char = '\0';
        let mut escaped = false;

        for ch in line.chars() {
            if escaped {
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if in_string {
                if ch == string_char {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' | '\'' => {
                    in_string = true;
                    string_char = ch;
                }
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                '[' => bracket_count += 1,
                ']' => bracket_count -= 1,
                '{' => brace_count += 1,
                '}' => brace_count -= 1,
                _ => {}
            }
        }

        // Statement is complete if all brackets are balanced
        paren_count == 0 && bracket_count == 0 && brace_count == 0
    }

    /// Check if line is a return statement (multi-language support)
    fn is_return_statement(&self, line: &str) -> bool {
        // Rust, Go, C, C++, Java, Kotlin, Swift, Scala: return x;
        // Python, Ruby: return x
        // JavaScript, TypeScript: return x;
        // C#: return x;
        // Bash: return n
        line.starts_with("return ") ||
        line.starts_with("return;") ||
        line == "return" ||
        line.starts_with("return(") || // Some languages: return(value)
        // Kotlin: "value" (last expression)
        // Swift: "return value" or implicit
        // Ruby: "return value" or implicit
        line.ends_with("return")
    }

    /// Check if line is panic/unreachable (multi-language support)
    fn is_panic_statement(&self, line: &str) -> bool {
        // Rust: panic!(...), unreachable!(), todo!(), unimplemented!()
        // Python: raise Exception(...), sys.exit()
        // JavaScript/TypeScript: throw new Error(...), throw ...
        // Go: panic(...), log.Fatal(...), os.Exit()
        // Java: throw new Exception(...), System.exit()
        // Kotlin: throw Exception(...), error(...), TODO()
        // Swift: fatalError(...), preconditionFailure(...)
        // C/C++: abort(), exit(), assert(false)
        // C#: throw new Exception(...), Environment.Exit()
        // Ruby: raise Exception, exit, abort
        // Scala: throw new Exception(...), sys.exit, ???
        // Bash: exit n

        line.contains("panic!(") ||
        line.contains("unreachable!(") ||
        line.contains("todo!(") ||
        line.contains("unimplemented!(") ||
        line.starts_with("raise ") ||
        line.contains("raise ") ||
        line.starts_with("throw ") ||
        line.contains("throw ") ||
        line.contains("panic(") ||
        line.contains("log.Fatal") ||
        line.contains("os.Exit") ||
        line.contains("System.exit") ||
        line.contains("error(") || // Kotlin
        line.contains("TODO()") ||
        line.contains("fatalError") ||
        line.contains("preconditionFailure") ||
        line.contains("abort()") ||
        line.contains("exit(") ||
        line.contains("Environment.Exit") ||
        line.contains("sys.exit") ||
        line.contains("assert(false)") ||
        line.contains("assert false") ||
        line.starts_with("exit ") // Bash/Ruby
    }

    /// Extract return expression
    fn extract_return_expression(&self, line: &str) -> String {
        let expr = line.strip_prefix("return").unwrap_or(line).trim();
        let expr = expr.trim_end_matches(';').trim();

        if expr.is_empty() {
            "()".to_string()
        } else {
            expr.to_string()
        }
    }

    /// Calculate Python indentation level (number of spaces/tabs)
    fn calculate_python_indent_level(&self, line: &str) -> usize {
        let mut indent = 0;
        for ch in line.chars() {
            if ch == ' ' {
                indent += 1;
            } else if ch == '\t' {
                indent += 4; // Treat tab as 4 spaces
            } else {
                break;
            }
        }
        indent
    }

    /// Calculate brace-based scope level for {} languages
    fn calculate_brace_scope_level(&self, lines: &[&str], current_idx: usize, start_idx: usize, end_idx: usize) -> usize {
        let mut level = 0;
        for i in start_idx..=current_idx.min(end_idx) {
            if i >= lines.len() {
                break;
            }
            let line = lines[i];
            level += line.matches('{').count();
            level = level.saturating_sub(line.matches('}').count());
        }
        level
    }

    /// Check if line starts a new scope (try/except/def/class/if/for/while/with)
    fn is_new_scope_start(&self, line: &str) -> bool {
        line.starts_with("try:") ||
        line.starts_with("except") ||
        line.starts_with("finally:") ||
        line.starts_with("def ") ||
        line.starts_with("class ") ||
        line.starts_with("if ") ||
        line.starts_with("elif ") ||
        line.starts_with("else:") ||
        line.starts_with("for ") ||
        line.starts_with("while ") ||
        line.starts_with("with ") ||
        line.contains("{") || // Brace languages
        line.starts_with("async def ") ||
        line.starts_with("@") // Decorator start
    }

    /// Check if line is exception handling (resets return tracking)
    fn is_exception_handler(&self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("except") ||
        trimmed.starts_with("finally:") ||
        trimmed.starts_with("catch") || // Java/C++/C#/JavaScript
        trimmed.starts_with("} catch") || // JavaScript/Java
        trimmed.contains("catch (") || // Various languages
        trimmed.starts_with("rescue") || // Ruby
        trimmed.starts_with("ensure") // Ruby
    }

    /// Check if line ends a scope (closing brace, dedent)
    fn is_scope_end(&self, line: &str) -> bool {
        line == "}" ||
        line.starts_with("}") ||
        line == "end" || // Ruby, some others
        false // Python scope ends are handled by indentation level
    }

    /// Check if return is inside conditional block (if/while/for)
    fn is_inside_conditional_block(&self, lines: &[&str], current_idx: usize, start_idx: usize) -> bool {
        if current_idx >= lines.len() || current_idx == 0 {
            return false;
        }

        let current_indent = self.calculate_python_indent_level(lines[current_idx]);

        // Look backwards to find the most recent control structure at lower indent
        for i in (start_idx..current_idx).rev() {
            if i >= lines.len() {
                continue;
            }

            let line = lines[i].trim();
            if line.is_empty() || line.starts_with("#") {
                continue;
            }

            let line_indent = self.calculate_python_indent_level(lines[i]);

            // For Python: look for control structures at lower indent level
            if line_indent < current_indent {
                if line.starts_with("if ") || line.starts_with("elif ") ||
                   line.starts_with("while ") || line.starts_with("for ") ||
                   line.starts_with("with ") {
                    return true;
                }

                // If we hit function/class definition, stop looking
                if line.starts_with("def ") || line.starts_with("class ") ||
                   line.starts_with("async def ") {
                    break;
                }
            }
        }

        // For brace languages, look for conditional keywords before opening brace
        for i in (start_idx..current_idx).rev() {
            if i >= lines.len() {
                continue;
            }

            let line = lines[i].trim();
            if line.is_empty() {
                continue;
            }

            // Simple heuristic for brace languages
            if (line.contains("if (") || line.contains("while (") ||
                line.contains("for (") || line.contains("switch (")) && line.contains("{") {
                return true;
            }

            // If we see function definition, stop
            if line.contains("function ") || line.contains("fn ") ||
               line.contains("func ") || line.contains("def ") {
                break;
            }
        }

        false
    }
}

impl Default for SimpleCfgAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_unreachable() {
        let code = r#"
fn example() {
    let x = 5;
    return x;
    println!("unreachable");
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("example", "test.rs", code, 1, 6).unwrap();

        assert!(result.has_unreachable_code);
        assert_eq!(result.unreachable_issues.len(), 1);
        assert_eq!(result.unreachable_issues[0].severity, Severity::High);
        assert!(result.unreachable_issues[0].code.contains("println"));
    }

    #[test]
    fn test_no_unreachable() {
        let code = r#"
fn example() {
    let x = 5;
    let y = x + 10;
    return y;
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("example", "test.rs", code, 1, 6).unwrap();

        assert!(!result.has_unreachable_code);
        assert_eq!(result.unreachable_issues.len(), 0);
    }

    #[test]
    fn test_panic_unreachable() {
        let code = r#"
fn example() {
    if x < 0 {
        panic!("negative");
        cleanup(); // unreachable!
    }
    return x;
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("example", "test.rs", code, 1, 8).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues.iter().any(|i| i.code.contains("cleanup")));
    }

    #[test]
    fn test_python_unreachable() {
        let code = r#"
def calculate(x):
    if x < 0:
        raise ValueError("negative")
        print("unreachable")
    return x * 2
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("calculate", "test.py", code, 1, 6).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues[0].code.contains("print"));
    }

    #[test]
    fn test_javascript_unreachable() {
        let code = r#"
function process(data) {
    if (!data) {
        throw new Error("invalid");
        console.log("unreachable");
    }
    return data.value;
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("process", "test.js", code, 1, 8).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues[0].code.contains("console.log"));
    }

    #[test]
    fn test_go_unreachable() {
        let code = r#"
func handler(w http.ResponseWriter) {
    if err != nil {
        panic(err)
        fmt.Println("unreachable")
    }
    return
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("handler", "test.go", code, 1, 8).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues[0].code.contains("fmt.Println"));
    }

    #[test]
    fn test_java_unreachable() {
        let code = r#"
public void validate(String input) {
    if (input == null) {
        throw new IllegalArgumentException();
        System.out.println("unreachable");
    }
    process(input);
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("validate", "Test.java", code, 1, 8).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues[0].code.contains("System.out"));
    }

    #[test]
    fn test_kotlin_unreachable() {
        let code = r#"
fun process(value: Int?) {
    value ?: error("null value")
    println("unreachable")
    return value * 2
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("process", "test.kt", code, 1, 6).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues[0].code.contains("println"));
    }

    #[test]
    fn test_swift_unreachable() {
        let code = r#"
func validate(input: String?) {
    guard let value = input else {
        fatalError("missing input")
        print("unreachable")
    }
    return value
}
"#;
        let analyzer = SimpleCfgAnalyzer::new();
        let result = analyzer.analyze_function("validate", "test.swift", code, 1, 8).unwrap();

        assert!(result.has_unreachable_code);
        assert!(result.unreachable_issues[0].code.contains("print"));
    }
}
