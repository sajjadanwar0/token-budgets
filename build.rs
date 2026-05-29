use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const ALLOWLIST_ENV: &str = "TOKEN_BUDGETS_AUTHORITY_ALLOWLIST";
const DEFAULT_ALLOWLIST: &str = ".token_budgets_authority.toml";

fn main() {
    let feature_enabled = env::var("CARGO_FEATURE_SYSTEM_AUTHORITY").is_ok();
    if !feature_enabled {
        println!("cargo:rerun-if-env-changed={}", ALLOWLIST_ENV);
        return;
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set during build");
    let workspace_root = locate_workspace_root(&PathBuf::from(&manifest_dir));

    let allowlist_path = env::var(ALLOWLIST_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join(DEFAULT_ALLOWLIST));

    println!("cargo:rerun-if-env-changed={}", ALLOWLIST_ENV);
    println!("cargo:rerun-if-changed={}", allowlist_path.display());

    if !allowlist_path.exists() {
        eprintln!();
        eprintln!("ERROR: token-budgets feature `system-authority` is enabled,");
        eprintln!("       but no allowlist file was found at:");
        eprintln!("         {}", allowlist_path.display());
        eprintln!();
        eprintln!("       This file MUST exist when system-authority is enabled.");
        eprintln!("       It declares which workspace members are authorised to");
        eprintln!("       mint Budget values via BudgetMint::take_authority().");
        eprintln!();
        eprintln!("       Create the file with the following minimal content:");
        eprintln!();
        eprintln!("         # .token_budgets_authority.toml");
        eprintln!("         git_tag = \"v1.0.0\"          # pin to a tagged commit");
        eprintln!("         allowed_activators = [        # crate names allowed");
        eprintln!("           \"my-agent-runtime\",         # to enable the feature");
        eprintln!("         ]");
        eprintln!();
        eprintln!("       Or override the path with:");
        eprintln!("         export {}=path/to/allowlist.toml", ALLOWLIST_ENV);
        eprintln!();
        eprintln!("       See `docs/trust-boundary.md` for the full threat model.");
        eprintln!();
        exit(1);
    }

    let raw = fs::read_to_string(&allowlist_path)
        .expect("failed to read allowlist file");
    let parsed = parse_allowlist(&raw, &allowlist_path);
    let primary_package = env::var("CARGO_PKG_NAME").unwrap_or_default();
    let allowed = primary_package == "token-budgets"
        || parsed.allowed_activators.iter().any(|a| a == &primary_package);

    if !allowed {
        eprintln!();
        eprintln!("ERROR: token-budgets feature `system-authority` enabled by");
        eprintln!("       non-allowlisted crate:");
        eprintln!("         activator: {}", primary_package);
        eprintln!("         allowlist: {}", allowlist_path.display());
        eprintln!("         allowed:   {:?}", parsed.allowed_activators);
        eprintln!();
        eprintln!("       To authorise this crate, add its name to");
        eprintln!("       `allowed_activators` in the allowlist file and");
        eprintln!("       re-tag the workspace (the git_tag field MUST be");
        eprintln!("       updated to a new tagged commit).");
        eprintln!();
        exit(1);
    }
    
    println!("cargo:rustc-env=TOKEN_BUDGETS_ALLOWLIST_TAG={}", parsed.git_tag);
}

#[derive(Debug)]
struct Allowlist {
    git_tag: String,
    allowed_activators: Vec<String>,
}

fn parse_allowlist(raw: &str, path: &Path) -> Allowlist {
    let mut git_tag = None;
    let mut activators = Vec::new();
    let mut in_array = false;
    let mut array_buf = String::new();

    for line_no in 0..raw.lines().count() {
        let line = raw.lines().nth(line_no).unwrap();
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() { continue; }

        if in_array {
            array_buf.push_str(trimmed);
            if trimmed.ends_with(']') { in_array = false; }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("git_tag") {
            let v = rest.trim_start_matches('=').trim()
                .trim_matches('"').trim_matches('\'').to_string();
            git_tag = Some(v);
        } else if trimmed.starts_with("allowed_activators") {
            let after_eq = trimmed.split('=').nth(1)
                .unwrap_or("").trim();
            array_buf.push_str(after_eq);
            if !after_eq.ends_with(']') { in_array = true; }
        }
    }

    let inner = array_buf.trim()
        .trim_start_matches('[').trim_end_matches(']');
    for entry in inner.split(',') {
        let s = entry.trim().trim_matches('"').trim_matches('\'');
        if !s.is_empty() { activators.push(s.to_string()); }
    }

    let git_tag = git_tag.unwrap_or_else(|| {
        eprintln!("ERROR: allowlist file {} missing required `git_tag` field",
                  path.display());
        exit(1);
    });

    Allowlist { git_tag, allowed_activators: activators }
}

fn locate_workspace_root(start: &Path) -> PathBuf {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("Cargo.lock").exists() { return cur; }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None    => return start.to_path_buf(),
        }
    }
}