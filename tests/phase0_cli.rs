use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_help_flag() {
    Command::cargo_bin("livetree")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Real-time directory tree watcher"))
        .stdout(predicate::str::contains("--level"))
        .stdout(predicate::str::contains("--ignore"))
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--dirs-only"))
        .stdout(predicate::str::contains("--debounce"))
        .stdout(predicate::str::contains("--verbose"))
        .stdout(predicate::str::contains("--quiet"))
        .stdout(predicate::str::contains("Examples:"));
}

#[test]
fn test_version_flag() {
    Command::cargo_bin("livetree")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("livetree"));
}

#[test]
fn test_nonexistent_path_exits_with_error() {
    Command::cargo_bin("livetree")
        .unwrap()
        .arg("/this/path/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file"));
}

#[test]
fn test_file_path_exits_with_error() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("afile.txt");
    std::fs::write(&file, "hello").unwrap();

    Command::cargo_bin("livetree")
        .unwrap()
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a directory"));
}

// NOTE: test_valid_directory_prints_watching was removed because
// the binary now enters raw mode + event loop (requires a real terminal).
// Valid-directory behavior is covered by the integration test.

#[test]
fn test_default_debounce_is_200() {
    use clap::Parser;
    use livetree::cli::Args;
    let args = Args::parse_from(["livetree", "."]);
    assert_eq!(args.debounce_ms, 200);
}

#[test]
fn test_custom_debounce() {
    use clap::Parser;
    use livetree::cli::Args;
    let args = Args::parse_from(["livetree", "--debounce", "500", "."]);
    assert_eq!(args.debounce_ms, 500);
}

#[test]
fn test_debounce_floor_enforced() {
    use clap::Parser;
    use livetree::cli::Args;
    let args = Args::parse_from(["livetree", "--debounce", "10", "."]).validated();
    assert_eq!(args.debounce_ms, 50, "Debounce floor should be 50ms");
}

#[test]
fn test_multiple_ignore_patterns() {
    use clap::Parser;
    use livetree::cli::Args;
    let args = Args::parse_from(["livetree", "-I", "*.log", "-I", "node_modules", "."]);
    assert_eq!(args.ignore, vec!["*.log", "node_modules"]);
}

#[test]
fn test_verbose_count_levels() {
    use clap::Parser;
    use livetree::cli::Args;
    let args = Args::parse_from(["livetree", "-vv", "."]).validated();
    assert_eq!(args.verbose, 2);
}

#[test]
fn test_quiet_resets_verbose() {
    use clap::Parser;
    use livetree::cli::Args;
    let args = Args::parse_from(["livetree", "-vv", "--quiet", "."]).validated();
    assert!(args.quiet);
    assert_eq!(args.verbose, 0, "quiet should reset verbosity to 0");
}
