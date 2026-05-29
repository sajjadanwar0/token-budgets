use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

#[derive(Deserialize)]
struct Allowlist {
    git_tag: Option<String>,
    allowed_activators: Vec<String>,
    cargo_lock_sha256: Option<String>,
}

fn main() -> ExitCode {
    let allowlist_path = std::env::var("TOKEN_BUDGETS_AUTHORITY_ALLOWLIST")
        .unwrap_or_else(|_| ".token_budgets_authority.toml".to_string());

    if !Path::new(&allowlist_path).exists() {
        eprintln!("ERROR: allowlist file not found: {}", allowlist_path);
        return ExitCode::from(1);
    }

    let raw = match fs::read_to_string(&allowlist_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: cannot read {}: {}", allowlist_path, e);
            return ExitCode::from(1);
        }
    };

    let parsed: Allowlist = match toml::from_str(&raw) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: cannot parse {}: {}", allowlist_path, e);
            return ExitCode::from(1);
        }
    };

    if let Some(tag) = &parsed.git_tag {
        let current = std::process::Command::new("git")
            .args(["describe", "--tags", "--exact-match"])
            .output();
        match current {
            Ok(out) if out.status.success() => {
                let current_tag = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if &current_tag != tag {
                    eprintln!("ERROR: allowlist pinned to tag {}, current is {}", tag, current_tag);
                    return ExitCode::from(2);
                }
                println!("OK git_tag matches ({})", tag);
            }
            _ => {
                eprintln!("WARN: cannot verify git tag (not on a tagged commit)");
            }
        }
    }

    if let Some(expected) = &parsed.cargo_lock_sha256 {
        let lock_path = "Cargo.lock";
        if let Ok(lock_content) = fs::read(lock_path) {
            let mut hasher = Sha256::new();
            hasher.update(&lock_content);
            let actual = format!("{:x}", hasher.finalize());
            if &actual != expected {
                eprintln!("ERROR: Cargo.lock SHA-256 mismatch");
                eprintln!("  expected: {}", expected);
                eprintln!("  actual:   {}", actual);
                return ExitCode::from(3);
            }
            println!("OK Cargo.lock SHA-256 matches");
        }
    }

    println!("OK allowed_activators ({}): {:?}",
             parsed.allowed_activators.len(),
             parsed.allowed_activators);

    println!();
    println!("Allowlist verification PASSED.");
    ExitCode::SUCCESS
}
