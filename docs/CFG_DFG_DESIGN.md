# CFG/DFG Architecture Design for Live Function Tree (Rust)

## 🎯 Goal
Add Control Flow Graph (CFG) and Data Flow Graph (DFG) analysis to Live Function Tree MCP server in pure Rust for maximum performance and Windows compatibility.

## 🔍 Reference Implementation Analysis

### Golang CFG (golang.org/x/tools/go/cfg)
```go
type Block struct {
    Index      int           // Position within CFG.Blocks
    Nodes      []ast.Node    // AST nodes in execution order
    Succs      []*Block      // Successor blocks
    Live       bool          // Reachable from entry?
}

type CFG struct {
    Blocks []*Block
}

func New(body *ast.BlockStmt, mayReturn func(call *ast.CallExpr) bool) *CFG
```

**Key Features:**
- **Pure AST-based**: Works directly with Go AST nodes
- **Reachability**: Live field tracks reachable blocks
- **Successor tracking**: Explicit successor edges
- **Special cases**: Handles panic, return, break, continue

### What We Learned from Plandex
1. **CFG is language-agnostic concept** - works for any structured code
2. **Unreachable code detection** - primary use case (dead code after return/panic)
3. **Visualization** - DOT graph generation is essential
4. **Performance** - Can analyze large projects (thousands of functions)

## 🏗️ Rust CFG/DFG Architecture

### Core Structures

```rust
// src/cfg_analyzer.rs

/// Control Flow Graph Block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgBlock {
    /// Block index within function CFG
    pub index: usize,

    /// Block kind/type
    pub kind: BlockKind,

    /// AST node IDs in this block (references to tree-sitter nodes)
    pub node_ids: Vec<usize>,

    /// Statements in this block (extracted text)
    pub statements: Vec<String>,

    /// Successor block indices
    pub successors: Vec<usize>,

    /// Is this block reachable from entry?
    pub is_reachable: bool,

    /// Source location
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlockKind {
    Entry,       // Function entry point
    Body,        // Regular sequential code
    IfThen,      // Then branch of if statement
    IfElse,      // Else branch of if statement
    LoopBody,    // Loop body
    LoopCond,    // Loop condition
    SwitchCase,  // Switch case
    TryCatch,    // Try block
    CatchHandler,// Catch handler
    FinallyBlock,// Finally block
    Return,      // Return statement
    Exit,        // Function exit point
}

/// Control Flow Graph for a single function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlFlowGraph {
    /// Function name
    pub function_name: String,

    /// File path
    pub file_path: String,

    /// All blocks in the CFG
    pub blocks: Vec<CfgBlock>,

    /// Entry block index (always 0)
    pub entry_block: usize,

    /// Exit block indices (functions can have multiple returns)
    pub exit_blocks: Vec<usize>,

    /// Summary statistics
    pub total_blocks: usize,
    pub reachable_blocks: usize,
    pub unreachable_blocks: usize,
}
```

### Data Flow Graph Structures

```rust
// src/dfg_analyzer.rs

/// Data Flow Node - represents a variable or value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DfgNode {
    /// Node ID
    pub id: usize,

    /// Variable/value name
    pub name: String,

    /// Node type
    pub node_type: DfgNodeType,

    /// Definition location
    pub definition_line: usize,

    /// Usage locations
    pub usage_lines: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DfgNodeType {
    Variable,      // Local variable
    Parameter,     // Function parameter
    Return,        // Return value
    Constant,      // Constant value
    FieldAccess,   // Object field access
    ArrayAccess,   // Array element access
}

/// Data Flow Edge - represents data dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DfgEdge {
    /// Source node ID (producer)
    pub from: usize,

    /// Target node ID (consumer)
    pub to: usize,

    /// Edge type
    pub edge_type: DfgEdgeType,

    /// Location where dependency occurs
    pub location_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DfgEdgeType {
    Definition,   // Variable definition
    Assignment,   // Value assignment
    Usage,        // Variable usage
    Return,       // Value returned
    Argument,     // Passed as argument
}

/// Data Flow Graph for a single function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowGraph {
    /// Function name
    pub function_name: String,

    /// File path
    pub file_path: String,

    /// All nodes (variables/values)
    pub nodes: Vec<DfgNode>,

    /// All edges (dependencies)
    pub edges: Vec<DfgEdge>,

    /// Unused variables (defined but never used)
    pub unused_variables: Vec<String>,
}
```

## 🔧 Implementation Strategy

### Phase 1: Core CFG Builder (Rust/Python/JavaScript)

**File:** `src/cfg_analyzer.rs`

```rust
pub struct CfgAnalyzer {
    /// Tree-sitter parser for AST
    tree_sitter: TreeSitterExtractor,
}

impl CfgAnalyzer {
    pub fn new() -> Self {
        Self {
            tree_sitter: TreeSitterExtractor::new(),
        }
    }

    /// Build CFG from source code
    pub fn build_cfg(
        &self,
        content: &str,
        language: &str,
        function_name: &str,
    ) -> Result<ControlFlowGraph> {
        // 1. Parse AST with tree-sitter
        // 2. Find function node
        // 3. Walk AST and build blocks
        // 4. Connect blocks with successor edges
        // 5. Run reachability analysis
        // 6. Return CFG
    }

    /// Detect unreachable code in function
    pub fn detect_unreachable_code(&self, cfg: &ControlFlowGraph) -> Vec<UnreachableCodeIssue> {
        // Filter blocks where is_reachable == false
        // Create issue reports with location and reason
    }

    /// Generate DOT graph for visualization
    pub fn generate_dot(&self, cfg: &ControlFlowGraph) -> String {
        // Graphviz DOT format
    }
}
```

**CFG Building Algorithm:**

1. **Entry Block Creation**
   - Create block 0 as function entry
   - Mark as reachable

2. **Statement Scanning**
   - Walk through function body AST nodes
   - Group sequential statements into blocks
   - Split blocks on control flow statements:
     - `if`/`else` - create branch blocks
     - `while`/`for` - create loop blocks
     - `return` - create exit block
     - `break`/`continue` - jump blocks

3. **Successor Linking**
   - Sequential blocks: successor = next block
   - If statements: successors = [then_block, else_block]
   - Loops: successors = [body_block, exit_block]
   - Returns: successors = []

4. **Reachability Analysis**
   - BFS from entry block
   - Mark all reachable blocks
   - Blocks not visited = unreachable

### Phase 2: DFG Builder

**File:** `src/dfg_analyzer.rs`

```rust
pub struct DfgAnalyzer {
    tree_sitter: TreeSitterExtractor,
}

impl DfgAnalyzer {
    /// Build DFG from source code
    pub fn build_dfg(
        &self,
        content: &str,
        language: &str,
        function_name: &str,
    ) -> Result<DataFlowGraph> {
        // 1. Parse AST
        // 2. Extract all variable definitions
        // 3. Track all variable usages
        // 4. Build nodes and edges
        // 5. Detect unused variables
    }

    /// Track variable flow through function
    pub fn get_variable_flow(&self, dfg: &DataFlowGraph, variable: &str) -> VariableFlow {
        // Find all paths from definition to usage
    }
}
```

**DFG Building Algorithm:**

1. **Variable Discovery**
   - Scan for variable declarations
   - Track function parameters
   - Identify constant definitions

2. **Usage Tracking**
   - Find all identifier references
   - Match references to definitions
   - Build dependency edges

3. **Unused Detection**
   - Variables with zero usage edges = unused
   - Filter out intentional unused (_, _unused, etc.)

### Phase 3: Language-Specific Handlers

**Support Matrix:**

| Language | CFG Support | DFG Support | Priority |
|----------|-------------|-------------|----------|
| Rust | ✅ Full | ✅ Full | High |
| Python | ✅ Full | ✅ Full | High |
| JavaScript | ✅ Full | ✅ Full | High |
| TypeScript | ✅ Full | ✅ Full | High |
| Go | ✅ Full | ✅ Full | Medium |
| Java | ✅ Full | ⚠️ Partial | Medium |
| C/C++ | ✅ Full | ⚠️ Partial | Low |
| Kotlin | ✅ Full | ⚠️ Partial | Low |

**Language-specific considerations:**

- **Rust**: Handle `match` expressions, `?` operator, lifetimes
- **Python**: Handle `with` statements, async/await, decorators
- **JavaScript**: Handle promises, async/await, closures
- **Go**: Handle defer, goroutines, channels

## 🔗 Integration with LFT

### Extended FunctionInfo Structure

```rust
// src/psi_graph.rs - extend existing FunctionInfo

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    // ... existing fields ...

    /// Control Flow Graph (optional, computed on demand)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg: Option<ControlFlowGraph>,

    /// Data Flow Graph (optional, computed on demand)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dfg: Option<DataFlowGraph>,

    /// Has unreachable code?
    pub has_unreachable_code: bool,

    /// Has unused variables?
    pub has_unused_variables: bool,
}
```

### SQLite Schema Extension

```sql
-- New tables for CFG/DFG storage

CREATE TABLE IF NOT EXISTS cfg_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_id INTEGER NOT NULL,
    block_index INTEGER NOT NULL,
    block_kind TEXT NOT NULL,
    is_reachable BOOLEAN NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    statements TEXT, -- JSON array
    successors TEXT, -- JSON array of indices
    FOREIGN KEY (function_id) REFERENCES functions(id)
);

CREATE TABLE IF NOT EXISTS dfg_nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_id INTEGER NOT NULL,
    node_name TEXT NOT NULL,
    node_type TEXT NOT NULL,
    definition_line INTEGER NOT NULL,
    usage_lines TEXT, -- JSON array
    FOREIGN KEY (function_id) REFERENCES functions(id)
);

CREATE TABLE IF NOT EXISTS unreachable_code_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_id INTEGER NOT NULL,
    block_index INTEGER NOT NULL,
    reason TEXT NOT NULL,
    detected_at INTEGER NOT NULL,
    FOREIGN KEY (function_id) REFERENCES functions(id)
);
```

### Indexing Pipeline Extension

```rust
// src/psi_graph.rs

impl PsiGraphManager {
    pub async fn index_project_with_cfg_dfg(
        &self,
        project_path: &str,
        languages: &[String],
        build_cfg: bool,
        build_dfg: bool,
    ) -> Result<IndexStats> {
        // 1. Normal indexing (existing code)
        let functions = self.index_project(project_path, languages, ...)?;

        // 2. Build CFG/DFG for each function (if requested)
        if build_cfg || build_dfg {
            let cfg_analyzer = CfgAnalyzer::new();
            let dfg_analyzer = DfgAnalyzer::new();

            for func in &mut functions {
                if build_cfg {
                    let cfg = cfg_analyzer.build_cfg(
                        &file_content,
                        &func.language,
                        &func.name,
                    )?;
                    func.cfg = Some(cfg);
                    func.has_unreachable_code = !cfg.unreachable_blocks.is_empty();
                }

                if build_dfg {
                    let dfg = dfg_analyzer.build_dfg(
                        &file_content,
                        &func.language,
                        &func.name,
                    )?;
                    func.dfg = Some(dfg);
                    func.has_unused_variables = !dfg.unused_variables.is_empty();
                }
            }
        }

        // 3. Store in database
        self.store_functions_with_graphs(&functions)?;

        Ok(stats)
    }
}
```

## 🛠️ New MCP Tools

### 1. `lft_analyze_cfg`
Analyze control flow graph for a specific function.

```json
{
  "repo_id": "my-project",
  "function_name": "process_payment",
  "file_path": "src/payment.rs",
  "include_visualization": true
}
```

**Response:**
```json
{
  "cfg": {
    "function_name": "process_payment",
    "total_blocks": 8,
    "reachable_blocks": 7,
    "unreachable_blocks": 1
  },
  "unreachable_issues": [
    {
      "block_index": 5,
      "reason": "Code after return statement",
      "location": "lines 45-47"
    }
  ],
  "dot_graph": "digraph CFG { ... }"
}
```

### 2. `lft_analyze_dfg`
Analyze data flow graph for a specific function.

```json
{
  "repo_id": "my-project",
  "function_name": "calculate_total"
}
```

**Response:**
```json
{
  "dfg": {
    "function_name": "calculate_total",
    "total_variables": 12,
    "unused_variables": 2
  },
  "unused": [
    {"name": "temp_result", "line": 23, "reason": "Defined but never used"},
    {"name": "debug_flag", "line": 15, "reason": "Defined but never used"}
  ]
}
```

### 3. `lft_detect_unreachable_code`
Scan entire project for unreachable code.

```json
{
  "repo_id": "my-project",
  "languages": ["rust", "python"],
  "severity": "high"
}
```

**Response:**
```json
{
  "total_functions_analyzed": 1247,
  "functions_with_unreachable_code": 23,
  "issues": [
    {
      "function": "process_payment",
      "file": "src/payment.rs",
      "line": 45,
      "severity": "high",
      "description": "Code after return statement will never execute"
    }
  ]
}
```

### 4. `lft_get_variable_flow`
Trace variable data flow through function.

```json
{
  "repo_id": "my-project",
  "function_name": "authenticate_user",
  "variable_name": "user_token"
}
```

**Response:**
```json
{
  "variable": "user_token",
  "definition_line": 12,
  "usage_locations": [15, 18, 23],
  "flow_path": [
    {"line": 12, "operation": "assignment", "value": "get_token()"},
    {"line": 15, "operation": "validation", "expression": "validate_token(user_token)"},
    {"line": 18, "operation": "usage", "expression": "decode_token(user_token)"},
    {"line": 23, "operation": "return", "expression": "return user_token"}
  ]
}
```

### 5. `lft_get_cfg_visualization`
Generate visualization for CFG.

```json
{
  "repo_id": "my-project",
  "function_name": "process_payment",
  "format": "dot"
}
```

**Response:**
```json
{
  "format": "dot",
  "content": "digraph CFG {\n  block_0 [label=\"Entry\"];\n  block_1 [label=\"if (amount > 0)\"];\n  ...\n}"
}
```

## 📊 Performance Considerations

### Memory Usage
- **CFG**: ~500 bytes per block, typical function = 5-10 blocks = **2-5 KB per function**
- **DFG**: ~200 bytes per node + 100 bytes per edge = **1-3 KB per function**
- **Total**: **3-8 KB per function**
- **Project**: 10,000 functions = **30-80 MB** (negligible with 256GB RAM)

### Computation Time
- **CFG Building**: ~0.5-2ms per function (tree-sitter parsing + graph building)
- **DFG Building**: ~1-3ms per function (variable tracking + edge construction)
- **Large Project**: 10,000 functions = **10-50 seconds** for full analysis
- **Incremental**: Only analyze changed functions = **milliseconds**

### Caching Strategy
1. **SQLite Storage**: Persist CFG/DFG to database
2. **File Hash Tracking**: Only rebuild when file changes
3. **Lazy Loading**: Load CFG/DFG only when requested
4. **In-Memory Cache**: Keep hot functions in RAM

## 🧪 Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_simple_function() {
        let code = r#"
        fn example() {
            let x = 5;
            return x;
            println!("unreachable"); // Should be detected
        }
        "#;

        let analyzer = CfgAnalyzer::new();
        let cfg = analyzer.build_cfg(code, "rust", "example").unwrap();

        assert_eq!(cfg.unreachable_blocks, 1);
    }

    #[test]
    fn test_dfg_unused_variable() {
        let code = r#"
        fn example() {
            let unused = 42; // Should be detected
            let used = 10;
            return used;
        }
        "#;

        let analyzer = DfgAnalyzer::new();
        let dfg = analyzer.build_dfg(code, "rust", "example").unwrap();

        assert_eq!(dfg.unused_variables.len(), 1);
        assert_eq!(dfg.unused_variables[0], "unused");
    }
}
```

### Integration Tests
- Real-world code samples from Rust, Python, JavaScript projects
- Edge cases: nested loops, complex conditionals, error handling
- Performance benchmarks: 1000+ function analysis

## 📖 Documentation Plan

### 1. CFG/DFG User Guide
- What is CFG and DFG?
- Use cases: dead code detection, variable tracking, refactoring safety
- Examples with real code

### 2. API Documentation
- All MCP tools with examples
- Request/response schemas
- Integration examples (VS Code extension, CLI)

### 3. Developer Guide
- Architecture overview
- How to extend for new languages
- Performance optimization tips

## 🚀 Implementation Roadmap

### Week 1: Core Infrastructure
- ✅ Review Plandex CFG implementation
- ⏳ Design Rust structures (this document)
- ⏳ Create `cfg_analyzer.rs` skeleton
- ⏳ Create `dfg_analyzer.rs` skeleton

### Week 2: CFG Implementation
- Basic CFG builder for Rust
- Reachability analysis
- Unreachable code detection
- Unit tests

### Week 3: DFG Implementation
- Basic DFG builder for Rust
- Variable tracking
- Unused variable detection
- Unit tests

### Week 4: Integration & Tools
- Extend PSI Graph
- SQLite schema
- MCP tools implementation
- Integration tests

### Week 5: Multi-language Support
- Python CFG/DFG
- JavaScript/TypeScript CFG/DFG
- Language-specific edge cases

### Week 6: Polish & Documentation
- Performance optimization
- Documentation
- Examples
- VS Code extension integration

## 🎯 Success Metrics

1. **Accuracy**: >95% detection rate for unreachable code
2. **Performance**: <5ms per function analysis
3. **Coverage**: Support for 5+ languages
4. **Usability**: Simple MCP API, clear results
5. **Compatibility**: Works on Windows/Linux/macOS

---

**Status**: 📋 Design Complete - Ready for Implementation
**Next**: Implement `cfg_analyzer.rs` core module
