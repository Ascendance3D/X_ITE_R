#!/usr/bin/env bash
# R0.2-C1 reproducible consumer of the published crates.io 0.1.0 archives.
#
#   bash crates/r0-2-c-tests/published-consumer/published-consumer.sh [OUT_DIR]
#
# OUT_DIR must be outside any source tree. Default: /tmp/r02c1-published-consumer.
# The script refuses to reuse an existing OUT_DIR. It never edits the archives or the
# extracted crates: vrml and x3d reach the extracted vrml97 through an explicit
# [patch.crates-io] entry in the consumer manifest. Exit 0 only when every step passes.
#
# Since R0.2-D the local vrml97 is a facade over the engine crates. This script is now the
# one place that proves the 0.1.0 differences: main.rs prints them as checks, and the
# compile probes below prove the items 0.1.0 lacks.
set -u -o pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARCHIVES="${R02C1_ARCHIVES:?set R02C1_ARCHIVES to the directory of the published 0.1.0 crate tarballs}"
OUT="${1:-/tmp/r02c1-published-consumer}"
LOG="$OUT/commands.log"

# SHA-256 values recorded by R0.1A (R0.1A-BASELINE-REPORT.md, "Published crate tarballs").
declare -A PIN=(
  [vrml97]=80072178321e7b6ecf1281ca42e0dce9d71b07c6541dbabba3cd1d69c00efd5f
  [vrml]=93b9e21dfd89a1a8ef2d02e1e5305b8f7de65eff543855435c9d51584519c197
  [x3d]=f6a4e16b19888af702f395e589aefb58533e683cc30bd97d15332d9c50341a98
)

if [ -e "$OUT" ]; then echo "published-consumer: REFUSED: $OUT exists" >&2; exit 2; fi
ROOT="$(cd "$HERE/../../.." && pwd)"
case "$OUT" in "$ROOT"|"$ROOT"/*) echo "published-consumer: REFUSED: $OUT is inside the source tree" >&2; exit 2;; esac
mkdir -p "$OUT/published" "$OUT/consumer/src" || exit 2

FAIL=0
step() { # step <name> <command...>
  local name="$1"; shift
  "$@" >>"$LOG.out" 2>&1
  local rc=$?
  printf '%s\texit=%s\t%s\n' "$name" "$rc" "$*" | tee -a "$LOG"
  [ $rc -eq 0 ] || FAIL=1
  return $rc
}

tree_hash() { (cd "$1" && find . -type f | LC_ALL=C sort | xargs -d '\n' sha256sum | sha256sum | cut -c1-64); }

for c in vrml97 vrml x3d; do
  a="$ARCHIVES/$c-0.1.0.crate"
  got="$(sha256sum "$a" | cut -c1-64)"
  if [ "$got" = "${PIN[$c]}" ]; then rc=0; else rc=1; FAIL=1; fi
  printf 'sha256 %s\texit=%s\t%s\n' "$c" "$rc" "$got" | tee -a "$LOG"
  [ $rc -eq 0 ] || { echo "published-consumer: archive hash mismatch for $c" >&2; exit 1; }
  step "extract $c" tar -xzf "$a" -C "$OUT/published"
done
chmod -R a-w "$OUT/published"
BEFORE="$(tree_hash "$OUT/published")"
echo "extracted-tree-sha256-before $BEFORE" | tee -a "$LOG"

cat >"$OUT/consumer/Cargo.toml" <<TOML
[package]
name = "r02c1-published-consumer"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
vrml97 = { path = "$OUT/published/vrml97-0.1.0" }
vrml = { path = "$OUT/published/vrml-0.1.0" }
x3d = { path = "$OUT/published/x3d-0.1.0" }

# vrml and x3d declare vrml97 = "0.1" from crates.io. Point that at the extracted archive.
[patch.crates-io]
vrml97 = { path = "$OUT/published/vrml97-0.1.0" }
TOML
cp "$HERE/main.rs" "$OUT/consumer/src/main.rs"
# The published vrml97 archive ships its own Cargo.lock; use it so dependency versions are the released ones.
cp "$OUT/published/vrml97-0.1.0/Cargo.lock" "$OUT/consumer/Cargo.lock"
chmod u+w "$OUT/consumer/Cargo.lock"

export CARGO_TARGET_DIR="$OUT/target"
step "cargo build" cargo build --offline --manifest-path "$OUT/consumer/Cargo.toml"
step "cargo run" cargo run -q --offline --manifest-path "$OUT/consumer/Cargo.toml"
step "patch used, not registry" bash -c "cargo tree --offline --manifest-path '$OUT/consumer/Cargo.toml' -i vrml97 | head -1 | grep -F '$OUT/published/vrml97-0.1.0'"

# Historical absence records (moved from the local suite in R0.2-D). Each probe names an
# item that vrml97 0.1.0 lacks and must fail `cargo check` with exactly the given rustc
# error code. A probe that compiles, or fails another way, fails the script.
R6='source_node: 0, source_field: String::new(), destination_node: 0, destination_field: String::new(), source_loc: vrml97::scene::FieldLoc::Pre(0), destination_loc: vrml97::scene::FieldLoc::Pre(0),'
C7='parent: None, outer: vrml97::scene::Outer::Scene, root_nodes: Vec::new(), named_nodes: Default::default(), protos: Default::default(), externprotos: Default::default(), routes: Vec::new(),'
PARSE='let s = vrml97::parse_bytes(b"#VRML V2.0 utf8\n", "a.wrl").unwrap();'
PROBES=(
  "matrix4_lacks_factor|E0599|let m = vrml97::fields::math::Matrix4::IDENTITY; m.factor(None, None, None, None);"
  "matrix4_lacks_get_transform|E0599|let m = vrml97::fields::math::Matrix4::IDENTITY; m.get_transform(None, None, None, None, None);"
  "node_kind_lacks_imported|E0599|let _ = vrml97::scene::scene::NodeKind::Imported(0);"
  "route_lacks_four_engine_fields|E0560|let _ = vrml97::scene::scene::Route { $R6 id: 0, conn: None, raw_source_field: String::new(), raw_destination_field: String::new(), };"
  "ctx_lacks_imported_exported|E0560|let _ = vrml97::scene::scene::ExecutionContext { $C7 imported: Default::default(), exported: Default::default(), };"
  "scene_lacks_scene_infos|E0609|$PARSE let _ = &s.scene_infos;"
  "scene_lacks_imported|E0609|$PARSE let _ = &s.imported;"
  "scene_lacks_exported|E0609|$PARSE let _ = &s.exported;"
  "scene_lacks_route_counter|E0609|$PARSE let _ = &s.route_counter;"
)
probe() { # probe <name> <code> <body>
  local d="$OUT/probes/$1"
  mkdir -p "$d/src"
  printf '[package]\nname = "%s"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n\n[workspace]\n\n[dependencies]\nvrml97 = { path = "%s" }\n' \
    "$1" "$OUT/published/vrml97-0.1.0" > "$d/Cargo.toml"
  cp "$OUT/published/vrml97-0.1.0/Cargo.lock" "$d/Cargo.lock" && chmod u+w "$d/Cargo.lock"
  printf '#![allow(unused)]\nfn main() {\n%s\n}\n' "$3" > "$d/src/main.rs"
  local err codes
  if err="$(CARGO_TARGET_DIR="$OUT/probe-target" cargo check --offline --quiet --color never --manifest-path "$d/Cargo.toml" 2>&1)"; then
    echo "$1: compiles against 0.1.0" >>"$LOG.out"; return 1
  fi
  codes="$(echo "$err" | grep -o 'error\[E[0-9]*\]' | sort -u | tr -d '\n')"
  echo "$1: $codes" >>"$LOG.out"
  [ "$codes" = "error[$2]" ]
}
for p in "${PROBES[@]}"; do
  IFS='|' read -r name code body <<<"$p"
  step "absent in 0.1.0: $name ($code)" probe "$name" "$code" "$body"
done

AFTER="$(tree_hash "$OUT/published")"
echo "extracted-tree-sha256-after $AFTER" | tee -a "$LOG"
if [ "$BEFORE" != "$AFTER" ]; then echo "published-consumer: extracted tree changed" | tee -a "$LOG"; FAIL=1; fi

if [ $FAIL -eq 0 ]; then echo "published-consumer: PASS" | tee -a "$LOG"; else echo "published-consumer: FAIL" | tee -a "$LOG"; fi
exit $FAIL
