#!/usr/bin/env bash
# R0.2-C1 future gate: the one command that runs every future-gate test.
#
#   bash crates/r0-2-c-tests/future-gate.sh
#
# Exit 0 only when all of these hold:
#   * no gate source contains an #[ignore] attribute,
#   * the gate test binaries compile with --features future_gate,
#   * the listed tests are exactly the EXPECTED set (no missing, no extra),
#   * every listed test ran and passed, and 0 tests were ignored.
# Before R0.2-D (Option B) the expected result is exit 1: the gate does not compile.
set -u -o pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="$HERE/Cargo.toml"
SOURCES=("$HERE/tests/future_gate.rs" "$HERE/tests/future_gate_chrome.rs")
CARGO_ARGS=(--manifest-path "$MANIFEST" --features future_gate --test future_gate --test future_gate_chrome)

# BEGIN EXPECTED (read by tests/future_gate_contract.rs)
EXPECTED=(
  gate_one_type_authority
  gate_all_public_paths_accessible
  gate_reexports_are_internal_types
  gate_matrix_factor_get_transform_present
  gate_node_kind_has_imported_variant
  gate_route_has_extra_fields
  gate_scene_has_extra_fields
  gate_execution_context_has_imported_and_exported
  gate_export_import_parses
  gate_orthoviewpoint_serializes_to_sfvec4f
  gate_float_format_matches_chrome
  gate_chrome_math_matches_chrome
)
# END EXPECTED

fail() { echo "future-gate: FAIL: $*"; exit 1; }

if grep -nE '^[[:space:]]*#\[ignore' "${SOURCES[@]}"; then
  fail "a gate source contains #[ignore]"
fi

LIST="$(cargo test "${CARGO_ARGS[@]}" -- --list 2>&1)" || { echo "$LIST" | tail -60; fail "the gate does not compile"; }
LISTED="$(echo "$LIST" | sed -n 's/: test$//p' | sort)"
WANT="$(printf '%s\n' "${EXPECTED[@]}" | sort)"
if [ "$LISTED" != "$WANT" ]; then
  diff <(echo "$WANT") <(echo "$LISTED")
  fail "listed gate tests differ from EXPECTED"
fi

OUT="$(cargo test "${CARGO_ARGS[@]}" 2>&1)"; RC=$?
echo "$OUT" | grep -E '^test |^test result:'
[ $RC -eq 0 ] || fail "cargo test exit $RC"

PASSED=0
while read -r line; do
  p="$(echo "$line" | sed -n 's/.* \([0-9]*\) passed.*/\1/p')"
  i="$(echo "$line" | sed -n 's/.* \([0-9]*\) ignored.*/\1/p')"
  [ "$i" = "0" ] || fail "ignored tests in: $line"
  PASSED=$((PASSED + p))
done < <(echo "$OUT" | grep '^test result:')

for t in "${EXPECTED[@]}"; do
  echo "$OUT" | grep -qx "test $t ... ok" || fail "$t did not run and pass"
done
[ "$PASSED" -eq "${#EXPECTED[@]}" ] || fail "passed $PASSED, expected ${#EXPECTED[@]}"

echo "future-gate: PASS (${#EXPECTED[@]} of ${#EXPECTED[@]} passed, 0 ignored)"
