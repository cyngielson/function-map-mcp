// Ten plik jest narzędziem serwisowym: czyści bazę SQLite MCP (projects + functions) dla live-function-tree.
//
// Dlaczego: na Windows często nie ma dostępnego sqlite3/System.Data.SQLite w PowerShell, a my chcemy
// szybko i bezpiecznie wyczyścić indeks przed E2E reindexem.
//
// Użycie (PowerShell):
//   cargo run -q --bin clear_db
//
// Domyślna ścieżka DB jest taka sama jak w `PsiGraphManager::new()`:
//   %USERPROFILE%\.live-function-tree\function_tree.db

use std::path::PathBuf;

use live_function_tree_mcp_rust::db_maintenance::{
    default_db_path,
    clear_db,
    ClearDbOptions,
};

// Ten plik jest narzędziem serwisowym: czyści bazę SQLite MCP (projects + functions) dla live-function-tree.
//
// Rozszerzenia (production-grade):
// - Możesz czyścić CAŁĄ bazę albo tylko wskazany `repo_id`.
// - Jest `--dry-run` (nic nie usuwa), `--yes` (bez pytania), `--no-backup`.
//
// Użycie (PowerShell):
//   cargo run -q --bin clear_db -- --dry-run
//   cargo run -q --bin clear_db -- --repo-id system_taxi_e2e --yes
//   cargo run -q --bin clear_db -- --yes          # full wipe

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    let db_path: PathBuf = default_db_path()?;

    if !db_path.exists() {
        println!("DB not found: {}", db_path.display());
        return Ok(());
    }

    // Confirmation guard (only for non-dry-run)
    if !args.dry_run && !args.yes {
        let action = if let Some(repo_id) = &args.repo_id {
            format!("Usunac dane TYLKO dla repo_id='{}'", repo_id)
        } else {
            "Usunac CALE dane (projects + functions)".to_string()
        };

        if !prompt_yes_no(&format!("{}? [y/N]: ", action))? {
            println!("Anulowano.");
            return Ok(());
        }
    }

    let report = clear_db(
        &db_path,
        ClearDbOptions {
            repo_id: args.repo_id.clone(),
            dry_run: args.dry_run,
            no_backup: args.no_backup,
            vacuum: None,
        },
    )?;

    if report.dry_run {
        println!("DRY-RUN: DB path: {}", report.db_path.display());
        if let Some(repo_id) = &report.repo_id {
            println!("DRY-RUN: would clear only repo_id='{}'", repo_id);
        } else {
            println!("DRY-RUN: would clear ALL repositories (projects + functions)");
        }
        println!(
            "DRY-RUN: matching rows -> projects: {}, functions: {}",
            report.projects_before,
            report.functions_before
        );
        return Ok(());
    }

    if let Some(path) = report.backup_path {
        println!("Backup created: {}", path.display());
    } else if args.no_backup {
        println!("Backup skipped (--no-backup)");
    }

    if let Some(repo_id) = &report.repo_id {
        println!("Cleared repo_id='{}' in DB: {}", repo_id, report.db_path.display());
    } else {
        println!("DB cleared: {}", report.db_path.display());
    }

    println!(
        "Rows (projects/functions): {} / {}  ->  {} / {}",
        report.projects_before,
        report.functions_before,
        report.projects_after,
        report.functions_after
    );

    Ok(())
}

#[derive(Debug, Default)]
struct CliArgs {
    repo_id: Option<String>,
    yes: bool,
    dry_run: bool,
    no_backup: bool,
}

impl CliArgs {
    fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut out = CliArgs::default();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--repo-id" => {
                    out.repo_id = args.next();
                }
                "--yes" | "-y" => {
                    out.yes = true;
                }
                "--dry-run" => {
                    out.dry_run = true;
                }
                "--no-backup" => {
                    out.no_backup = true;
                }
                "--help" | "-h" => {
                    print_help_and_exit();
                }
                _ => {
                    eprintln!("Unknown argument: {}", arg);
                    print_help_and_exit();
                }
            }
        }

        out
    }
}

fn print_help_and_exit() -> ! {
    let exe = std::env::args().next().unwrap_or_else(|| "clear_db".to_string());
    eprintln!(
        "\nUzycie:\n  {} [--repo-id <id>] [--dry-run] [--yes|-y] [--no-backup]\n\nOpcje:\n  --repo-id <id>   Czyści tylko wskazany repo_id (usuwa z projects + functions)\n  --dry-run        Nic nie usuwa, tylko pokazuje co by zrobił\n  --yes, -y        Nie pyta o potwierdzenie\n  --no-backup      Nie tworzy kopii zapasowej .db.bak_TIMESTAMP\n",
        exe
    );
    std::process::exit(2);
}

fn prompt_yes_no(prompt: &str) -> anyhow::Result<bool> {
    use std::io::{self, Write};
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let v = input.trim().to_lowercase();
    Ok(matches!(v.as_str(), "y" | "yes" | "t" | "true" | "1"))
}

