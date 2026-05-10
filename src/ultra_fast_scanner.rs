// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! Ultra-Fast File System Scanner
//! Extracted from Ultra-Brain for maximum performance
//!
//! Key optimizations:
//! - Intelligent directory skipping
//! - File pattern exclusions
//! - Parallel processing with Rayon
//! - Memory-efficient processing

use std::path::{Path, PathBuf};
use walkdir::{WalkDir, DirEntry};
use rayon::prelude::*;
use anyhow::Result;
use log::{debug, info, warn};

/// Ultra-fast directory skipping based on Ultra-Brain patterns
pub fn should_skip_directory(entry: &DirEntry) -> bool {
    if let Some(file_name) = entry.path().file_name() {
        let name = file_name.to_string_lossy();
        let name_lower = name.to_lowercase();

        // Check venv-related patterns first (most common)
        if name_lower.contains("venv") || name_lower.contains("virtualenv") ||
           name_lower.contains("site-packages") || name_lower.contains("__pycache__") {
            return true;
        }

        let should_skip = matches!(name.as_ref(),
            // ═══════════════════════════════════════════════════════════════════
            // UNIVERSAL - Package managers & dependencies (all languages)
            // ═══════════════════════════════════════════════════════════════════
            "node_modules" | "venv" | ".venv" | "env" | ".env" | "vendor" |
            "bower_components" | "jspm_packages" | "packages" | "deps" | ".deps" |
            "third_party" | "3rdparty" | "external" | "externals" |

            // ═══════════════════════════════════════════════════════════════════
            // VERSION CONTROL
            // ═══════════════════════════════════════════════════════════════════
            ".git" | ".svn" | ".hg" | ".bzr" | "CVS" | "_darcs" | ".fossil" |

            // ═══════════════════════════════════════════════════════════════════
            // RUST - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            "target" | ".cargo" | "cargo-home" |
            // Rust documentation output & registry cache
            "doc" | "docs" | "registry" |

            // ═══════════════════════════════════════════════════════════════════
            // PYTHON - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            "__pycache__" | ".pytest_cache" | ".coverage" | ".mypy_cache" |
            ".tox" | ".nox" | "site-packages" | "lib64" | "include" | "Scripts" |
            ".eggs" | "pip-wheel-metadata" | ".Python" |
            "htmlcov" | ".hypothesis" | ".ruff_cache" | ".pdm-build" |

            // ═══════════════════════════════════════════════════════════════════
            // JAVASCRIPT/TYPESCRIPT/NODE - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            ".next" | ".nuxt" | ".cache" | "coverage" | "nyc_output" |
            ".parcel-cache" | ".turbo" | ".vercel" | ".netlify" |
            ".svelte-kit" | ".angular" | ".storybook-static" |
            "storybook-static" | ".docusaurus" | ".output" |

            // ═══════════════════════════════════════════════════════════════════
            // JAVA/KOTLIN/SCALA - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            ".gradle" | "gradle" | ".m2" | "maven" |
            "classes" | "generated" | "generated-sources" | "generated-test-sources" |
            ".apt_generated" | ".apt_generated_tests" |

            // ═══════════════════════════════════════════════════════════════════
            // GO - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            "pkg" |

            // ═══════════════════════════════════════════════════════════════════
            // DART/FLUTTER - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            ".dart_tool" | ".pub-cache" | ".pub" |
            "build" | "ios" | "android" | "linux" | "macos" | "windows" | "web" |
            ".fvm" | "ephemeral" |

            // ═══════════════════════════════════════════════════════════════════
            // SWIFT/iOS/macOS - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            "Pods" | "DerivedData" | ".xcode" | "xcuserdata" |
            "Carthage" | ".build" | "SourcePackages" |

            // ═══════════════════════════════════════════════════════════════════
            // C/C++ - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            "cmake-build-debug" | "cmake-build-release" | "CMakeFiles" |
            "Debug" | "Release" | "x64" | "x86" | "ARM" | "ARM64" |
            ".vs" | "ipch" | "obj" | "lib" | "bin" |
            "_deps" | "vcpkg_installed" | "conan" |

            // ═══════════════════════════════════════════════════════════════════
            // .NET/C# - Non-code directories (bin/obj/packages already above)
            // ═══════════════════════════════════════════════════════════════════
            ".nuget" | "TestResults" | "BenchmarkDotNet.Artifacts" |

            // ═══════════════════════════════════════════════════════════════════
            // RUBY - Non-code directories (vendor already above)
            // ═══════════════════════════════════════════════════════════════════
            ".bundle" |

            // ═══════════════════════════════════════════════════════════════════
            // ELIXIR/ERLANG - Non-code directories (deps already above)
            // ═══════════════════════════════════════════════════════════════════
            "_build" | ".elixir_ls" | "cover" |

            // ═══════════════════════════════════════════════════════════════════
            // TERRAFORM/INFRASTRUCTURE - Non-code directories
            // ═══════════════════════════════════════════════════════════════════
            ".terraform" | ".terragrunt-cache" | "cdktf.out" |

            // ═══════════════════════════════════════════════════════════════════
            // SYSTEM & IDE FILES (some already above like .vs)
            // ═══════════════════════════════════════════════════════════════════
            "logs" | "tmp" | "temp" | ".tmp" | ".temp" | "cache" |
            ".idea" | ".vscode" | ".DS_Store" | "Thumbs.db" |
            ".settings" | ".project" | ".classpath" | ".metadata" |

            // ═══════════════════════════════════════════════════════════════════
            // DOCUMENTATION (generated/external) - doc/docs already above
            // ═══════════════════════════════════════════════════════════════════
            "documentation" | "javadoc" | "rustdoc" |
            "apidoc" | "apidocs" | "doxygen" | "sphinx" |
            "site" | "_site" | "public" |

            // ═══════════════════════════════════════════════════════════════════
            // TESTING & DEMO (Ultra-Brain AI optimization)
            // ═══════════════════════════════════════════════════════════════════
            "test" | "tests" | "__tests__" | "spec" | "specs" | ".tests" |
            "demo" | "demos" | "example" | "examples" | "samples" | "fixtures" |
            "benchmarks" | "performance" | "e2e" | "integration" | "mocks" |
            "testdata" | "test_data" | "test-data" | "mock_data" | "mockdata" |
            "__mocks__" | "__snapshots__" | "__fixtures__" |

            // ═══════════════════════════════════════════════════════════════════
            // BUILD OUTPUT (universal) - build already above (Flutter)
            // ═══════════════════════════════════════════════════════════════════
            "dist" | "out" | "output" | "outputs" |
            "artifacts" | "release" | "releases" |

            // ═══════════════════════════════════════════════════════════════════
            // BACKUP, ARCHIVE & LEGACY
            // ═══════════════════════════════════════════════════════════════════
            "backup" | "backups" | "copy" | "library" | "libraries" |
            "archive" | "archives" | "old" | "legacy" | "deprecated" |
            "unused" | "obsolete" | "retired" | "trash" | ".trash" |

            // ═══════════════════════════════════════════════════════════════════
            // ASSETS & RESOURCES (non-code) - public already above
            // ═══════════════════════════════════════════════════════════════════
            "assets" | "images" | "img" | "icons" | "fonts" |
            "media" | "videos" | "audio" | "sounds" |
            "resources" | "res" | "static" |
            "locales" | "translations" | "i18n" | "l10n"
        );

        if should_skip {
            debug!("🚫 SKIPPING DIRECTORY: {} -> {}", entry.path().display(), name);
        }

        should_skip
    } else {
        false
    }
}

/// Ultra-fast file skipping based on Ultra-Brain patterns
pub fn should_skip_file(file_path: &str) -> bool {
    let path_lower = file_path.to_lowercase();

    // Skip test/demo files by name patterns
    path_lower.starts_with("test-") ||
    path_lower.starts_with("demo-") ||
    path_lower.starts_with("large-test") ||
    path_lower.starts_with("mock-") ||
    path_lower.starts_with("fake-") ||
    path_lower.starts_with("sample-") ||
    path_lower.starts_with("example-") ||
    path_lower.starts_with("backup-") ||
    path_lower.starts_with("copy-") ||
    path_lower.contains("test_") ||
    path_lower.contains("_test") ||
    path_lower.contains(".test.") ||
    path_lower.contains("spec.") ||
    path_lower.contains(".spec.") ||
    path_lower.contains("mock") ||
    path_lower.contains("fake") ||
    path_lower.contains("fixture") ||
    path_lower.contains("benchmark") ||
    path_lower.contains("backup") ||
    path_lower.contains("_backup") ||
    path_lower.contains("copy") ||
    path_lower.contains("_copy") ||
    // Backup and temporary files
    path_lower.ends_with(".backup") ||
    path_lower.ends_with(".bak") ||
    path_lower.ends_with(".tmp") ||
    path_lower.ends_with(".temp") ||
    path_lower.ends_with("~") ||
    path_lower.contains(".swp") ||
    // Broken/test files
    path_lower.ends_with("broken.js") ||
    path_lower.ends_with("broken.py") ||
    path_lower.ends_with("broken.dart") ||
    path_lower.ends_with("broken.go") ||
    path_lower.ends_with("broken.css") ||
    path_lower.ends_with("broken.html") ||
    path_lower.ends_with("broken.ts") ||
    path_lower.ends_with("broken.jsx") ||
    // Large test files that waste analysis time
    path_lower.contains("large-") ||
    path_lower.contains("huge-") ||
    path_lower.contains("big-test") ||
    // Configuration and lock files
    path_lower.ends_with(".lock") ||
    path_lower.ends_with("-lock.json") ||
    path_lower.ends_with(".log") ||
    // Python compiled files and caches - CRITICAL for TaxiTech performance!
    path_lower.ends_with(".pyc") ||
    path_lower.ends_with(".pyo") ||
    path_lower.ends_with(".pyd") ||
    path_lower.ends_with(".so") ||
    path_lower.ends_with(".whl") ||
    path_lower.ends_with(".egg") ||
    // Compiled binaries and executables
    path_lower.ends_with(".class") ||
    path_lower.ends_with(".o") ||
    path_lower.ends_with(".obj") ||
    path_lower.ends_with(".exe") ||
    path_lower.ends_with(".dll") ||
    path_lower.ends_with(".dylib") ||
    path_lower.ends_with(".a") ||
    path_lower.ends_with(".lib") ||
    // Media files that shouldn't be analyzed as code
    path_lower.ends_with(".png") ||
    path_lower.ends_with(".jpg") ||
    path_lower.ends_with(".jpeg") ||
    path_lower.ends_with(".gif") ||
    path_lower.ends_with(".svg") ||
    path_lower.ends_with(".ico") ||
    path_lower.ends_with(".webp") ||
    path_lower.ends_with(".mp4") ||
    path_lower.ends_with(".avi") ||
    path_lower.ends_with(".mov") ||
    path_lower.ends_with(".mp3") ||
    path_lower.ends_with(".wav") ||
    path_lower.ends_with(".ogg") ||
    // Archive files
    path_lower.ends_with(".zip") ||
    path_lower.ends_with(".tar") ||
    path_lower.ends_with(".gz") ||
    path_lower.ends_with(".rar") ||
    path_lower.ends_with(".7z") ||
    // Database and binary data files
    path_lower.ends_with(".db") ||
    path_lower.ends_with(".sqlite") ||
    path_lower.ends_with(".sqlite3") ||
    path_lower.ends_with(".mdb") ||
    path_lower.ends_with(".accdb") ||
    // PDF and document files
    path_lower.ends_with(".pdf") ||
    path_lower.ends_with(".doc") ||
    path_lower.ends_with(".docx") ||
    path_lower.ends_with(".xls") ||
    path_lower.ends_with(".xlsx") ||
    path_lower.ends_with(".ppt") ||
    path_lower.ends_with(".pptx") ||
    path_lower == "composer.lock" ||
    path_lower == "pipfile.lock" ||
    path_lower == "poetry.lock" ||
    // Virtual environments and dependency directories - CRITICAL for clean analysis!
    path_lower.contains("/venv/") ||
    path_lower.contains("\\venv\\") ||
    path_lower.contains("/node_modules/") ||
    path_lower.contains("\\node_modules\\")
}

/// Check if file extension is supported for analysis
pub fn is_supported_extension(file_path: &Path) -> bool {
    if let Some(extension) = file_path.extension() {
        if let Some(ext_str) = extension.to_str() {
            let ext_lower = ext_str.to_lowercase();
            matches!(ext_lower.as_str(),
                // Supported source code extensions
                "rs" | "py" | "js" | "ts" | "jsx" | "tsx" |
                "dart" | "go" | "java" | "c" | "cpp" | "cc" | "cxx" |
                "h" | "hpp" | "php" | "rb" | "swift" | "kt" | "scala" |
                "cs" | "fs" | "vb" | "m" | "mm" | "r" | "R" |
                // Configuration and markup files with functions
                "json" | "toml" | "yaml" | "yml" | "xml"
            )
        } else {
            false
        }
    } else {
        false
    }
}

/// Ultra-fast parallel file collection with Ultra-Brain optimizations
pub fn collect_source_files_parallel(
    project_path: &str,
    max_files: Option<usize>,
    languages: &[String]
) -> Result<Vec<PathBuf>> {
    let start_time = std::time::Instant::now();
    info!("🚀 Starting ULTRA-FAST parallel file collection");
    info!("📂 Project: {}", project_path);
    info!("🎯 Max files: {:?}", max_files);
    info!("🗣️ Languages: {:?}", languages);

    let path = Path::new(project_path);
    if !path.exists() {
        return Err(anyhow::anyhow!("Project path does not exist: {}", project_path));
    }

    // Step 1: Fast directory traversal with intelligent filtering
    let entries: Vec<_> = WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            // 🚀 ULTRA-FAST directory filtering like Ultra-Brain
            if e.path().is_dir() {
                !should_skip_directory(e)
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            if e.path().is_file() {
                let file_path = e.path().to_string_lossy().to_string();
                // 🚀 ULTRA-FAST file filtering like Ultra-Brain
                !should_skip_file(&file_path) && is_supported_extension(e.path())
            } else {
                false
            }
        })
        .collect();

    info!("📁 Found {} potential files after filtering", entries.len());

    // Step 2: Parallel processing with Rayon (Ultra-Brain style)
    let mut filtered_files: Vec<PathBuf> = entries
        .par_iter()
        .filter_map(|entry| {
            let path = entry.path();

            // Additional language filtering if specified
            if !languages.is_empty() {
                if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        let ext_lower = ext_str.to_lowercase();
                        let matches_language = languages.iter().any(|lang| {
                            match lang.to_lowercase().as_str() {
                                "rust" => ext_lower == "rs",
                                "python" => ext_lower == "py",
                                "javascript" => ext_lower == "js",
                                "typescript" => matches!(ext_lower.as_str(), "ts" | "tsx"),
                                "dart" => ext_lower == "dart",
                                "go" => ext_lower == "go",
                                "java" => ext_lower == "java",
                                "c" => matches!(ext_lower.as_str(), "c" | "h"),
                                "cpp" | "c++" => matches!(ext_lower.as_str(), "cpp" | "cc" | "cxx" | "hpp"),
                                _ => ext_lower == lang.to_lowercase(),
                            }
                        });

                        if !matches_language {
                            return None;
                        }
                    }
                }
            }

            Some(path.to_path_buf())
        })
        .collect();

    // Apply max_files limit after parallel processing
    if let Some(max) = max_files {
        filtered_files.truncate(max);
    }

    let files = filtered_files;    let duration = start_time.elapsed();
    info!("✨ Ultra-fast file collection completed in {:.2}ms", duration.as_millis());
    info!("📊 Collected {} source files", files.len());

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_skip_directory() {
        // Should skip common directories (with full path separator)
        assert!(should_skip_file("src/node_modules/package/test.js"));
        assert!(should_skip_file("project/venv/lib/python.py"));

        // Should not skip regular directories
        assert!(!should_skip_file("src/main.rs"));
        assert!(!should_skip_file("lib/utils.dart"));
    }

    #[test]
    fn test_should_skip_file() {
        // Should skip test files
        assert!(should_skip_file("test-example.js"));
        assert!(should_skip_file("demo-app.py"));
        assert!(should_skip_file("src/utils_test.rs"));

        // Should skip binary files
        assert!(should_skip_file("image.png"));
        assert!(should_skip_file("archive.zip"));
        assert!(should_skip_file("compiled.pyc"));

        // Should not skip source files
        assert!(!should_skip_file("src/main.rs"));
        assert!(!should_skip_file("lib/api.dart"));
        assert!(!should_skip_file("utils/helper.py"));
    }

    #[test]
    fn test_is_supported_extension() {
        // Should support common source extensions
        assert!(is_supported_extension(Path::new("main.rs")));
        assert!(is_supported_extension(Path::new("app.py")));
        assert!(is_supported_extension(Path::new("component.tsx")));
        assert!(is_supported_extension(Path::new("widget.dart")));

        // Should not support binary extensions
        assert!(!is_supported_extension(Path::new("image.png")));
        assert!(!is_supported_extension(Path::new("video.mp4")));
        assert!(!is_supported_extension(Path::new("archive.zip")));
    }
}
