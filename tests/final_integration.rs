//! Final integration test for LiveTree.
//!
//! Exercises the full pipeline:
//! 1. Creates a realistic directory structure
//! 2. Tests tree building, rendering, watcher, and ratatui Line output
//! 3. Performs filesystem mutations and verifies correctness
//! 4. Validates config combinations
//! 5. Performance smoke test
//!
//! Run with tracing output:
//!   RUST_LOG=debug cargo test --test final_integration -- --nocapture

mod common;

use common::default_tree_config;
use livetree::render::{line_to_plain_text, status_bar_line, tree_to_lines, RenderConfig};
use livetree::tree::{build_ignore_set, build_tree, TreeConfig};
use livetree::watcher::{start_watcher, WatchEvent};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tracing::{debug, error, info, span, warn, Level};
use tracing_subscriber::EnvFilter;

// ───────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init();
}

/// Create a realistic project fixture.
fn create_project_fixture(root: &Path) {
    info!("Creating project fixture at {}", root.display());

    let dirs = [
        "src",
        "src/components",
        "src/utils",
        "tests",
        "docs",
        ".git",
    ];
    let files = [
        ("src/main.rs", "fn main() { }"),
        ("src/lib.rs", "pub mod components;\npub mod utils;"),
        ("src/components/mod.rs", "pub mod button;"),
        ("src/components/button.rs", "pub struct Button;"),
        ("src/utils/mod.rs", "pub mod helpers;"),
        ("src/utils/helpers.rs", "pub fn help() {}"),
        ("tests/integration.rs", "#[test] fn it_works() {}"),
        ("docs/README.md", "# My Project"),
        ("Cargo.toml", "[package]\nname = \"myproject\""),
        ("Cargo.lock", "# auto-generated"),
        (".gitignore", "target/\n"),
        (".git/config", "[core]"),
    ];

    for dir in &dirs {
        fs::create_dir_all(root.join(dir)).unwrap();
        debug!("  Created dir:  {}", dir);
    }
    for (file, content) in &files {
        fs::write(root.join(file), content).unwrap();
        debug!("  Created file: {}", file);
    }

    info!(
        "Fixture created: {} dirs, {} files",
        dirs.len(),
        files.len()
    );
}

// ───────────────────────────────────────────────────
// Test 1: Full Lifecycle
// ───────────────────────────────────────────────────

#[test]
fn test_full_lifecycle() {
    init_tracing();
    let _span = span!(Level::INFO, "full_lifecycle_test").entered();

    info!("========================================");
    info!("  LiveTree Full Integration Test");
    info!("========================================");

    // --- Step 1: Create fixture ---
    let tmp = TempDir::new().unwrap();
    create_project_fixture(tmp.path());

    // --- Step 2: Validate tree builder ---
    {
        let _span = span!(Level::INFO, "tree_builder_validation").entered();
        info!("Validating tree builder with fixture...");

        let cfg = default_tree_config();
        let entries = build_tree(tmp.path(), &cfg);
        info!("Tree has {} entries", entries.len());

        // .git is ignored by default
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&".git"), "FAIL: .git should be ignored");
        info!("  [PASS] .git is ignored by default");

        // Expected files present
        assert!(names.contains(&"src"), "FAIL: src/ missing");
        assert!(names.contains(&"Cargo.toml"), "FAIL: Cargo.toml missing");
        assert!(names.contains(&"button.rs"), "FAIL: button.rs missing");
        info!("  [PASS] Expected files present");

        // Directories sort before files
        let src_idx = entries.iter().position(|e| e.name == "src").unwrap();
        let cargo_idx = entries.iter().position(|e| e.name == "Cargo.toml").unwrap();
        assert!(
            src_idx < cargo_idx,
            "FAIL: Directories should sort before files"
        );
        info!("  [PASS] Sort order correct (dirs before files)");

        // All entries at depth > 0 have non-empty prefixes
        for entry in entries.iter() {
            if entry.depth > 0 {
                assert!(
                    !entry.prefix.is_empty(),
                    "FAIL: Entry '{}' at depth {} has empty prefix",
                    entry.name,
                    entry.depth
                );
            }
        }
        info!("  [PASS] All entries have valid prefixes");

        // Render to ratatui Lines for visual inspection
        let lines = tree_to_lines(
            &entries,
            &RenderConfig {
                use_color: false,
                terminal_width: 80,
            },
            &HashSet::new(),
        );
        let output: String = lines
            .iter()
            .map(|l| line_to_plain_text(l))
            .collect::<Vec<_>>()
            .join("\n");
        info!("Rendered tree:\n{}", output);
    }

    // --- Step 3: Filesystem mutations ---
    {
        let _span = span!(Level::INFO, "fs_mutation_tests").entered();
        let cfg = default_tree_config();

        // Mutation 1: Add a new file
        info!("Mutation 1: Adding new_feature.rs...");
        let before_count = build_tree(tmp.path(), &cfg).len();
        fs::write(tmp.path().join("src/new_feature.rs"), "pub fn feature() {}").unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert_eq!(after.len(), before_count + 1);
        assert!(after.iter().any(|e| e.name == "new_feature.rs"));
        info!(
            "  [PASS] New file appears ({} -> {} entries)",
            before_count,
            after.len()
        );

        // Mutation 2: Delete a file
        info!("Mutation 2: Deleting docs/README.md...");
        fs::remove_file(tmp.path().join("docs/README.md")).unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(!after.iter().any(|e| e.name == "README.md"));
        info!("  [PASS] Deleted file removed from tree");

        // Mutation 3: Add directory with files
        info!("Mutation 3: Adding config/ directory...");
        fs::create_dir(tmp.path().join("config")).unwrap();
        fs::write(tmp.path().join("config/settings.toml"), "key = \"value\"").unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(after.iter().any(|e| e.name == "config"));
        assert!(after.iter().any(|e| e.name == "settings.toml"));
        info!("  [PASS] New directory and files appear");

        // Mutation 4: Rename a file
        info!("Mutation 4: Renaming Cargo.lock -> Cargo.lock.bak...");
        fs::rename(
            tmp.path().join("Cargo.lock"),
            tmp.path().join("Cargo.lock.bak"),
        )
        .unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(!after.iter().any(|e| e.name == "Cargo.lock"));
        assert!(after.iter().any(|e| e.name == "Cargo.lock.bak"));
        info!("  [PASS] Renamed file reflected correctly");

        // Mutation 5: Delete entire directory tree
        info!("Mutation 5: Deleting tests/ directory tree...");
        fs::remove_dir_all(tmp.path().join("tests")).unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(!after.iter().any(|e| e.name == "tests"));
        assert!(!after.iter().any(|e| e.name == "integration.rs"));
        info!("  [PASS] Deleted directory tree removed completely");

        info!("All filesystem mutation tests passed.");
    }

    // --- Step 4: Watcher integration ---
    {
        let _span = span!(Level::INFO, "watcher_integration").entered();
        info!("Testing watcher integration...");

        let watch_tmp = TempDir::new().unwrap();
        fs::create_dir(watch_tmp.path().join("src")).unwrap();
        fs::write(watch_tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

        let (watcher, rx) = start_watcher(watch_tmp.path(), 100).expect("Watcher should start");

        // Let watcher settle
        std::thread::sleep(Duration::from_millis(200));

        // Operation 1: Create file
        info!("  Watcher op 1: Create file");
        let start = Instant::now();
        fs::write(watch_tmp.path().join("new.txt"), "content").unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(WatchEvent::Changed(_)) => {
                info!("  [PASS] Detected creation in {:?}", start.elapsed());
            }
            other => {
                error!("  [FAIL] Expected Changed, got: {:?}", other);
                panic!("Watcher failed to detect file creation");
            }
        }

        // Operation 2: Modify file
        info!("  Watcher op 2: Modify file");
        let start = Instant::now();
        fs::write(watch_tmp.path().join("new.txt"), "modified").unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(WatchEvent::Changed(_)) => {
                info!("  [PASS] Detected modification in {:?}", start.elapsed());
            }
            other => warn!("  [WARN] Unexpected event on modify: {:?}", other),
        }

        // Operation 3: Delete file
        info!("  Watcher op 3: Delete file");
        let start = Instant::now();
        fs::remove_file(watch_tmp.path().join("new.txt")).unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(WatchEvent::Changed(_)) => {
                info!("  [PASS] Detected deletion in {:?}", start.elapsed());
            }
            other => warn!("  [WARN] Unexpected event on delete: {:?}", other),
        }

        drop(watcher);
        info!("Watcher integration test complete.");
    }

    // --- Step 5: Render pipeline end-to-end ---
    {
        let _span = span!(Level::INFO, "render_pipeline").entered();
        info!("Testing render pipeline end-to-end...");

        let render_tmp = TempDir::new().unwrap();
        create_project_fixture(render_tmp.path());

        let tree_cfg = default_tree_config();
        let entries = build_tree(render_tmp.path(), &tree_cfg);
        let render_cfg = RenderConfig {
            use_color: false,
            terminal_width: 80,
        };

        // Render to ratatui Lines
        let lines = tree_to_lines(&entries, &render_cfg, &HashSet::new());
        info!("Rendered {} lines", lines.len());

        assert!(lines.len() >= 3, "Should have at least 3 lines");

        let all_text: String = lines
            .iter()
            .map(|l| line_to_plain_text(l))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all_text.contains("src"));
        assert!(all_text.contains("main.rs"));
        assert!(all_text.contains("README.md"));
        info!("  [PASS] Tree lines contain expected entries");

        // Status bar
        let bar = status_bar_line(
            &render_tmp.path().to_string_lossy(),
            &format!("{} entries", lines.len()),
            Some("12:34:56"),
        );
        let bar_text = line_to_plain_text(&bar);
        assert!(bar_text.contains("entries"));
        assert!(bar_text.contains("12:34:56"));
        info!("  [PASS] Status bar renders correctly");

        info!("Render pipeline test complete.");
    }

    // --- Step 6: Config combinations ---
    {
        let _span = span!(Level::INFO, "config_combinations").entered();
        info!("Testing configuration combinations...");

        let combo_tmp = TempDir::new().unwrap();
        create_project_fixture(combo_tmp.path());

        let configs: Vec<(&str, TreeConfig)> = vec![
            ("default", default_tree_config()),
            (
                "depth=1",
                TreeConfig {
                    max_depth: Some(1),
                    ..default_tree_config()
                },
            ),
            (
                "dirs_only",
                TreeConfig {
                    dirs_only: true,
                    ..default_tree_config()
                },
            ),
            (
                "show_hidden",
                TreeConfig {
                    show_hidden: true,
                    ..default_tree_config()
                },
            ),
            (
                "ignore *.rs",
                TreeConfig {
                    ignore_patterns: build_ignore_set(&["*.rs".to_string()]),
                    ..default_tree_config()
                },
            ),
            (
                "depth=2 + dirs_only",
                TreeConfig {
                    max_depth: Some(2),
                    dirs_only: true,
                    ..default_tree_config()
                },
            ),
        ];

        for (label, cfg) in &configs {
            let entries = build_tree(combo_tmp.path(), cfg);
            info!("  Config '{}': {} entries", label, entries.len());

            // Validate invariants
            if cfg.dirs_only {
                assert!(
                    entries.iter().filter(|e| e.depth >= 1).all(|e| e.is_dir),
                    "FAIL: dirs_only config '{}' has non-dir entries",
                    label
                );
            }
            if let Some(max_depth) = cfg.max_depth {
                assert!(
                    entries.iter().all(|e| e.depth <= max_depth),
                    "FAIL: max_depth config '{}' exceeded",
                    label
                );
            }

            info!("  [PASS] Config '{}' invariants hold", label);
        }

        info!("Configuration combination tests complete.");
    }

    info!("========================================");
    info!("  ALL INTEGRATION TESTS PASSED");
    info!("========================================");
}

// ───────────────────────────────────────────────────
// Test 2: Performance Smoke Test
// ───────────────────────────────────────────────────

#[test]
fn test_performance_large_directory() {
    init_tracing();
    let _span = span!(Level::INFO, "performance_test").entered();

    let tmp = TempDir::new().unwrap();

    // Create 1000 files across 50 subdirectories
    info!("Creating 1000-file fixture...");
    for dir_idx in 0..50 {
        let dir = tmp.path().join(format!("dir{:03}", dir_idx));
        fs::create_dir(&dir).unwrap();
        for file_idx in 0..20 {
            fs::write(dir.join(format!("file{:03}.txt", file_idx)), "content").unwrap();
        }
    }

    let cfg = default_tree_config();

    // Benchmark tree building
    let start = Instant::now();
    let entries = build_tree(tmp.path(), &cfg);
    let build_duration = start.elapsed();
    info!(
        "Tree build: {} entries in {:?}",
        entries.len(),
        build_duration
    );
    assert!(
        build_duration < Duration::from_millis(500),
        "Tree build took {:?}, should be < 500ms for 1000 entries",
        build_duration
    );

    // Benchmark rendering to ratatui Lines
    let start = Instant::now();
    let lines = tree_to_lines(
        &entries,
        &RenderConfig {
            use_color: true,
            terminal_width: 120,
        },
        &HashSet::new(),
    );
    let render_duration = start.elapsed();
    info!("Render: {} lines in {:?}", lines.len(), render_duration);
    assert!(
        render_duration < Duration::from_millis(100),
        "Render took {:?}, should be < 100ms",
        render_duration
    );

    let total = build_duration + render_duration;
    info!("  [PASS] Total pipeline: {:?}", total);
    info!(
        "  Target for smooth 5 FPS: < 200ms total. Actual: {:?}",
        total
    );
}
