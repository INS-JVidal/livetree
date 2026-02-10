#![forbid(unsafe_code)]

use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use clap_mangen::Man;
use livetree::cli::Args;
use std::fs;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out_dir = PathBuf::from("dist");
    let completions_dir = out_dir.join("completions");
    let man_dir = out_dir.join("man");

    fs::create_dir_all(&completions_dir)?;
    fs::create_dir_all(&man_dir)?;

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        let mut cmd = Args::command();
        generate_to(shell, &mut cmd, "livetree", &completions_dir)?;
    }

    let man = Man::new(Args::command());
    let mut buffer = Vec::new();
    man.render(&mut buffer)?;
    fs::write(man_dir.join("livetree.1"), buffer)?;

    eprintln!(
        "generated shell completions and man page under {}",
        out_dir.display()
    );
    Ok(())
}
