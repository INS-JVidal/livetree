use assert_cmd::Command;

#[test]
fn test_generate_assets_binary_runs() {
    Command::cargo_bin("generate-assets")
        .unwrap()
        .assert()
        .success();
}

