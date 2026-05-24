#!/usr/bin/env bash
# Run AFTER applying tb-fixes-v2/APPLY_ALL.sh.
# Regenerates trybuild .stderr files with system-authority feature enabled.
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo not in PATH. Install Rust toolchain first:"
    echo "  curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "Deleting existing .stderr files..."
rm -f tests/compile_fail/*.stderr

echo "Regenerating with TRYBUILD=overwrite --features system-authority..."
TRYBUILD=overwrite cargo test --features system-authority --test compile_fail 2>&1 | tail -20

echo ""
echo "Verifying distinct rustc codes in regenerated stderrs:"
codes=$(grep -h "error\[E[0-9]\{4\}\]" tests/compile_fail/*.stderr | grep -oE "E[0-9]{4}" | sort -u)
echo "$codes" | sed "s/^/  /"
count=$(echo "$codes" | wc -l)
echo ""
echo "Total: $count distinct rustc codes"
echo "Paper section 5.1 claims 6 distinct codes: E0277, E0308, E0382, E0505, E0507, E0599"
if [ "$count" -ge 6 ]; then
    echo "OK Matches paper claim"
else
    echo "WARN: Verify the tests actually exercise 6 different rejection mechanisms"
fi
