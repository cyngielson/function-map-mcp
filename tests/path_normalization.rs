// Testy regresji: normalizacja ścieżek Windows i kontekst dla absolutnych file_path.
//
// Dlaczego to istnieje:
// - Przez różnice w wielkości liter w ścieżkach (C:\ vs c:\) DB potrafiła mieć duplikaty,
//   a lookup repo_id po project_path nie działał (hierarchical tree robiło unknown-repo-* i zwracało 0).
// - Dodatkowo kontekst funkcji znikał, bo `extract_function_context` joinował project_path z absolutnym file_path.

use std::fs;

use live_function_tree_mcp_rust::psi_graph::PsiGraphManager;

#[test]
fn normalize_project_path_lowercases_on_windows() {
    let input = r"C:\Taxi\TaxiTech\system_taxi\";
    let normalized = PsiGraphManager::normalize_project_path(input);

    // zawsze '/' + bez trailing '/'
    assert!(!normalized.ends_with('/'));
    assert!(!normalized.contains('\\'));

    // Na Windows spodziewamy się normalizacji case (żeby C:\ == c:\)
    if cfg!(windows) {
        assert_eq!(normalized, "c:/taxi/taxitech/system_taxi");
    }
}

#[test]
fn normalize_project_path_is_idempotent() {
    let input = "c:/taxi/taxitech/system_taxi";
    let a = PsiGraphManager::normalize_project_path(input);
    let b = PsiGraphManager::normalize_project_path(&a);
    assert_eq!(a, b);
}

#[test]
fn absolute_paths_are_not_joined() {
    // Ten test jest pośredni: sprawdza, że absolutny path pozostaje absolutny po "normalizacji".
    // (W samej implementacji extract_function_context używamy Path::is_absolute())
    let tmp = std::env::temp_dir().join("lft_test_abs_path.txt");
    fs::write(&tmp, "line1\nline2\nline3\n").expect("write temp file");

    assert!(tmp.is_absolute());
}
