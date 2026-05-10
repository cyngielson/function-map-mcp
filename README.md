# function-map-mcp

**MCP server that gives AI assistants instant, structured access to every function in your codebase.**

Built in Rust. No PostgreSQL, no pgvector - just a local SQLite file. Point it at your project, index once, then let your AI navigate the code like a human senior dev would.

---

## Why does this exist?

When you work with Claude, Copilot, or any AI agent on a large codebase, the AI has a problem: it cannot see your code structure. It reads files one by one, guesses what functions exist, misses dependencies, and hallucinates implementations.

**function-map-mcp** solves this by giving the AI a real map:
- Every function, method, and class extracted with Tree-sitter AST (not grep)
- File paths and line numbers, so the AI can jump straight to what it needs
- Complexity scores, so it knows what is worth reading
- Junk filtered out (getters, setters, boilerplate) so AI does not waste context on noise

The result: AI assistants navigate your codebase like they wrote it.

---

## How is it different from vector search MCP?

|  | vector search (pgvector) | function-map-mcp (SQLite) |
|---|---|---|
| **Finds** | Code by semantic meaning | Functions by name/signature/file |
| **Index type** | 1024-dim embeddings | AST-parsed function graph |
| **Query** | find auth logic | show me all functions in UserService |
| **Setup** | PostgreSQL + pgvector required | Zero deps, one SQLite file |
| **Speed** | ~50ms embedding lookup | less than 5ms indexed query |
| **Best for** | Semantic code discovery | Structural navigation + AI context |

They complement each other. Vector search finds where the code is. Function map shows what is there.

---

## Setup

### Requirements

- Rust 1.70+ (for building from source)
- Windows, Linux, or macOS

### Build

```
git clone https://github.com/cyngielson/function-map-mcp
cd function-map-mcp
cargo build --release
```

Binary: target/release/function-map-mcp (or .exe on Windows)

### Add to MCP config

**Claude Desktop** (claude_desktop_config.json):

```json
{
  "mcpServers": {
    "function-map": {
      "command": "C:/path/to/function-map-mcp.exe",
      "args": []
    }
  }
}
```

**VS Code Copilot** (.vscode/mcp.json):

```json
{
  "servers": {
    "function-map": {
      "type": "stdio",
      "command": "C:/path/to/function-map-mcp.exe",
      "args": []
    }
  }
}
```

---

## MCP Tools Reference

### lft_index_project - Index a codebase

```json
{
  "project_path": "C:/my-project",
  "repo_id": "my-project",
  "languages": ["rust", "python", "typescript"],
  "max_files": 5000
}
```

Indexes ~4,000 functions/second. A 1000-file project takes ~200ms.

---

### lft_get_hierarchical_tree - Get full function map

```json
{
  "project_path": "C:/my-project",
  "filter_junk": true,
  "max_depth": 5
}
```

Returns a structured tree: module -> file -> functions, with line numbers and complexity scores. Primary tool for giving AI a project overview.

---

### lft_query_functions - Search by name or pattern

```json
{
  "repo_id": "my-project",
  "query_type": "by_name",
  "symbol": "authenticate",
  "max_results": 20
}
```

Instant lookup. Finds authenticate, authenticate_user, authenticateRequest across all files.

---

### lft_get_call_graph - See who calls what

```json
{
  "repo_id": "my-project",
  "function_name": "process_payment",
  "direction": "both"
}
```

Shows callers and callees. Helps AI understand impact before suggesting changes.

---

### lft_index_incremental - Re-index only changed files

```json
{
  "repo_id": "my-project",
  "project_path": "C:/my-project"
}
```

Uses file modification timestamps. After initial index, updates take milliseconds.

---

### lft_search_functions - Regex/pattern search

```json
{
  "project_path": "C:/my-project",
  "query": "handle.*request"
}
```

---

### lft_get_stats - Project statistics

```json
{
  "repo_id": "my-project"
}
```

Returns: file count, function count, language breakdown, avg complexity, last indexed timestamp.

---

### lft_watch_project - Real-time reindexing

```json
{
  "repo_id": "my-project",
  "project_path": "C:/my-project",
  "debounce_ms": 500
}
```

Watches for file changes and re-indexes automatically. The AI always has a fresh map.

---

## Supported Languages

### Tree-sitter AST (precise, 14 languages)

| Category | Languages |
|----------|-----------|
| Systems | Rust, C, C++, Go |
| JVM | Java, Kotlin, Scala |
| Scripting | Python, Ruby |
| Web | JavaScript, TypeScript |
| Mobile | Swift |
| .NET | C# |
| Shell | Bash |

### Regex fallback (6+ additional)

PHP (Laravel/WordPress), Dart (Flutter), HTML (event handlers), JSON (scripts/endpoints)

---

## Smart Junk Filter

Not every function is worth showing to an AI. The filter removes noise by default:

- Getters/setters (getName, setAge, property accessors)
- Trivial constructors (__init__ with just self.x = x)
- One-liners (return statements, simple assignments)
- Test helpers (mock_*, setup_*, teardown_*)
- Auto-generated code
- Standard boilerplate (toString, equals, hashCode)

The AI gets the 20% of functions that actually matter, not 500 lines of noise.

Filter aggressiveness is configurable:

```bash
LFT_FILTER_MODE=development   # aggressive (default)
LFT_FILTER_MODE=review        # moderate
LFT_FILTER_MODE=analysis      # minimal
```

---

## Performance

| Operation | Time |
|-----------|------|
| Index 1,000 files | ~200ms |
| Query by function name | less than 5ms |
| Incremental re-index (10 changed files) | less than 20ms |
| Full hierarchical tree | less than 10ms |

---

## Database

Stored in a local SQLite file (~/.function-map-mcp/index.db by default). No server, no cloud, no internet required.

```bash
LFT_DB_PATH=H:/my-indexes/
```

---

## Practical usage with AI

Ask your AI assistant:

Use function-map to index my project, then show me all functions with complexity > 10.

Find every function that handles authentication in this codebase.

Before you edit PaymentService, show me its call graph so we know what breaks.

This is the difference between an AI that guesses and an AI that navigates.

---

## License

MIT