// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Narzędzia serwisowe do utrzymania bazy SQLite dla Live Function Tree.
//!
//! Cel:
//! - Ten moduł jest jedynym miejscem, gdzie trzymamy logikę typu: backup DB, kasowanie danych,
//!   liczenie rekordów. Dzięki temu:
//!   - binarka `clear_db` i MCP tool `lft_clear_db` używają TEJ SAMEJ logiki,
//!   - unikamy rozjazdu zachowania i błędów specyficznych dla Windows.
//!
//! Wymagania / kontrakt:
//! - Operacje są deterministyczne i idempotentne (kolejne uruchomienie po czyszczeniu nie psuje nic).
//! - Bezpieczeństwo: backup jest domyślny, ale można go wyłączyć.
//! - VACUUM domyślnie robimy tylko przy pełnym wipe (nie przy pojedynczym repo).

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

/// Konfiguracja SQLite pod workload równoległy (Windows + wiele procesów).
///
/// Problem, który rozwiązujemy:
/// - MCP server może działać w tle, a w tym samym czasie benchmark/klient odpala indeksowanie.
/// - SQLite jest single-writer; bez ustawień typu `busy_timeout` dostajemy szybkie SQLITE_BUSY
///   („database is locked”) zamiast krótkiego poczekania.
///
/// Podejście:
/// - ustawiamy `busy_timeout` (czekamy aż writer zwolni lock),
/// - przełączamy na WAL (lepsze współdzielenie read/write),
/// - ustawiamy sensowne PRAGMA pod wydajność i stabilność.
fn open_sqlite_with_retry_pragmas(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    // Czekaj na locki zamiast wywalać SQLITE_BUSY.
    // 60s jest bezpieczne dla „one-scan” na większych projektach, a nadal nie wisi wiecznie.
    conn.busy_timeout(std::time::Duration::from_secs(60))?;

    // WAL pozwala na równoległe odczyty podczas zapisu (single-writer nadal obowiązuje).
    // Jeśli nie zadziała (np. ograniczenia FS), to wciąż działamy na default journal_mode.
    let _ = conn.execute_batch(
        "PRAGMA journal_mode = WAL;\n\
         PRAGMA synchronous = NORMAL;\n\
         PRAGMA temp_store = MEMORY;\n\
         PRAGMA foreign_keys = ON;\n",
    );

    Ok(conn)
}

#[derive(Debug, Clone)]
pub struct ClearDbOptions {
    pub repo_id: Option<String>,
    pub dry_run: bool,
    pub no_backup: bool,
    pub vacuum: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ClearDbReport {
    pub db_path: PathBuf,
    pub repo_id: Option<String>,
    pub backup_path: Option<PathBuf>,
    pub projects_before: i64,
    pub functions_before: i64,
    pub projects_after: i64,
    pub functions_after: i64,
    pub vacuum_ran: bool,
    pub dry_run: bool,
}

pub fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot resolve home_dir"))?;
    Ok(home.join(".live-function-tree").join("function_tree.db"))
}

pub fn get_counts(conn: &Connection, repo_id: Option<&str>) -> Result<(i64, i64)> {
    let (projects, functions) = if let Some(repo_id) = repo_id {
        let projects: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE repo_id = ?1",
            [repo_id],
            |row| row.get(0),
        )?;
        let functions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM functions WHERE repo_id = ?1",
            [repo_id],
            |row| row.get(0),
        )?;
        (projects, functions)
    } else {
        let projects: i64 = conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
        let functions: i64 = conn.query_row("SELECT COUNT(*) FROM functions", [], |row| row.get(0))?;
        (projects, functions)
    };

    Ok((projects, functions))
}

pub fn clear_db(db_path: &Path, opts: ClearDbOptions) -> Result<ClearDbReport> {
    if !db_path.exists() {
        return Err(anyhow::anyhow!("DB not found: {}", db_path.display()));
    }

    let repo_id = opts.repo_id.clone();

    // Open connection (mutable for transactions)
    let mut conn = open_sqlite_with_retry_pragmas(db_path)?;

    let (projects_before, functions_before) = get_counts(&conn, repo_id.as_deref())?;

    // Dry-run: nothing else.
    if opts.dry_run {
        return Ok(ClearDbReport {
            db_path: db_path.to_path_buf(),
            repo_id,
            backup_path: None,
            projects_before,
            functions_before,
            projects_after: projects_before,
            functions_after: functions_before,
            vacuum_ran: false,
            dry_run: true,
        });
    }

    // Backup (optional)
    let backup_path: Option<PathBuf> = if opts.no_backup {
        None
    } else {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = db_path.with_extension(format!("db.bak_{}", ts));
        std::fs::copy(db_path, &backup_path)?;
        Some(backup_path)
    };

    // Dla czyszczenia relacji nie ma znaczenia, ale wyłączenie FK przyspiesza DELETE'y.
    // Włączamy ścieżkę „OFF” lokalnie tylko na czas operacji.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let tx = conn.transaction()?;

    if let Some(repo_id) = repo_id.as_ref() {
        tx.execute("DELETE FROM functions WHERE repo_id = ?1", params![repo_id])?;
        tx.execute("DELETE FROM projects WHERE repo_id = ?1", params![repo_id])?;
    } else {
        tx.execute("DELETE FROM functions", [])?;
        tx.execute("DELETE FROM projects", [])?;
    }

    tx.commit()?;

    let should_vacuum = opts.vacuum.unwrap_or(repo_id.is_none());
    let vacuum_ran = if should_vacuum {
        conn.execute_batch("VACUUM;")?;
        true
    } else {
        false
    };

    let (projects_after, functions_after) = get_counts(&conn, repo_id.as_deref())?;

    Ok(ClearDbReport {
        db_path: db_path.to_path_buf(),
        repo_id,
        backup_path,
        projects_before,
        functions_before,
        projects_after,
        functions_after,
        vacuum_ran,
        dry_run: false,
    })
}
