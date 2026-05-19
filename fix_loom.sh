#!/usr/bin/env bash
# fix_loom.sh — patch tests/loom_concurrent.rs to use Budget::mint
# Run from the token-budgets/ crate root.

set -uo pipefail

F=tests/loom_concurrent.rs
if [[ ! -f "$F" ]]; then
    echo "ERROR: $F not found. Run from the token-budgets crate root."
    exit 1
fi

# Restore from backup if a prior partial patch left things broken.
if [[ -f "${F}.bak" ]]; then
    echo "Restoring from ${F}.bak before re-patching..."
    cp "${F}.bak" "$F"
else
    cp "$F" "${F}.bak"
    echo "Saved original to ${F}.bak"
fi

echo
echo "=== Original Budget::new call sites ==="
grep -n "Budget::<[^>]*>::new(" "$F" || echo "(none found)"
echo

# ----------------------------------------------------------------------
# Edit 1: Add BudgetMint to the token_budgets imports.
# ----------------------------------------------------------------------
# We try three patterns in order:
#   (a) `use token_budgets::{Budget, ...}` (brace form)
#   (b) `use token_budgets::Budget;` (single import)
#   (c) Last `use` line — append a separate line
# After this, the file should have `use token_budgets::BudgetMint;` somewhere.

if ! grep -q 'BudgetMint' "$F"; then
    if grep -qE 'use\s+token_budgets::\{[^}]*\}' "$F"; then
        # Brace form: add BudgetMint inside the braces
        sed -i.tmp -E \
            's|(use\s+token_budgets::\{[^}]*)\}|\1, BudgetMint}|' \
            "$F"
        rm -f "${F}.tmp"
    elif grep -qE 'use\s+token_budgets::Budget\s*;' "$F"; then
        # Single Budget import: append BudgetMint after it
        sed -i.tmp -E \
            's|(use\s+token_budgets::Budget\s*;)|\1\nuse token_budgets::BudgetMint;|' \
            "$F"
        rm -f "${F}.tmp"
    else
        # Append a fresh line after any other use line
        awk '
            /^use / { last_use = NR }
            { lines[NR] = $0 }
            END {
                for (i = 1; i <= NR; i++) {
                    print lines[i]
                    if (i == last_use) {
                        print "use token_budgets::BudgetMint;"
                    }
                }
            }
        ' "$F" > "${F}.tmp" && mv "${F}.tmp" "$F"
    fi
fi

# ----------------------------------------------------------------------
# Edit 2: Add a loom_mint helper as a top-level fn.
# Gated by feature so it disappears when not using system-authority.
# ----------------------------------------------------------------------

if ! grep -q 'fn loom_mint' "$F"; then
    # Insert after the last use-line block
    awk '
        BEGIN { inserted = 0 }
        /^use / { last_use = NR }
        { lines[NR] = $0 }
        END {
            for (i = 1; i <= NR; i++) {
                print lines[i]
                if (i == last_use && !inserted) {
                    print ""
                    print "// Loom-test mint authority. Requires the `system-authority` feature."
                    print "#[cfg(feature = \"system-authority\")]"
                    print "fn loom_mint() -> BudgetMint { BudgetMint::take_authority() }"
                    inserted = 1
                }
            }
        }
    ' "$F" > "${F}.tmp" && mv "${F}.tmp" "$F"
fi

# ----------------------------------------------------------------------
# Edit 3: Replace every Budget::<X>::new(Y) with Budget::<X>::mint(&loom_mint(), Y).
# This is the load-bearing edit. Works for both Budget::<MAX>::new(...) and
# Budget::<TEST_MAX>::new(...).
# ----------------------------------------------------------------------

# Use Python because sed's regex is awkward for nested parens.
python3 <<'PY'
import re, pathlib
p = pathlib.Path("tests/loom_concurrent.rs")
src = p.read_text()
# Match Budget::<NAME>::new(expr)
# We grab the type param and the argument; the argument may have () inside
# but our existing call sites are simple (e.g., "100" or "0"), so simple
# non-greedy match works.
new_src = re.sub(
    r"Budget::<([A-Za-z0-9_]+)>::new\(([^()]*)\)",
    r"Budget::<\1>::mint(&loom_mint(), \2)",
    src,
)
p.write_text(new_src)
print("Python substitution complete.")
PY

# ----------------------------------------------------------------------
# Verification
# ----------------------------------------------------------------------
echo
echo "=== Post-patch state ==="
echo "BudgetMint imports:"
grep -n "BudgetMint" "$F"
echo
echo "loom_mint helper:"
grep -n "loom_mint" "$F"
echo
echo "Remaining Budget::<...>::new( call sites (should be 0):"
grep -n "Budget::<[^>]*>::new(" "$F" || echo "  (none — all patched)"
echo
echo "=== Diff ==="
diff -u "${F}.bak" "$F" | head -80