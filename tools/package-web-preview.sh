#!/usr/bin/env bash
# Build and package the X_ITE_R Browser Preview: the browser module of crates/x3d-web, the preview page, its clean-room
# test worlds and the example, as a directory and a .tar.gz with SHA-256 checksums.
#
#   tools/package-web-preview.sh [OUT_DIR]
#
# OUT_DIR (default target/web-preview) must not hold the package yet. The working tree must be clean: the package
# records the commit it is built from (build-info.json), and the same commit and toolchain give the same files.
# The build is tools/build-web.sh with --remap-path-prefix flags, so the module holds no path of this machine (a check
# below fails the package when one is left). Needs what tools/build-web.sh needs, and Node for the import check.
set -euo pipefail

cd "$(dirname "$0")/.."
repo=$(pwd)

release=x_ite_r-browser-preview-0.1.0
out=${1:-target/web-preview}
dir=$out/$release

if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "package-web-preview: the working tree has changes; commit them first" >&2
  exit 1
fi

if [[ -e "$dir" || -e "$out/$release.tar.gz" ]]; then
  echo "package-web-preview: $dir or its .tar.gz exists already" >&2
  exit 1
fi

commit=$(git rev-parse HEAD)
epoch=$(git log -1 --format=%ct HEAD)
cargo_home=${CARGO_HOME:-$HOME/.cargo}
sysroot=$(rustc --print sysroot)
build_dir=${CARGO_BUILD_BUILD_DIR:-$repo/target}

# Paths of this machine become fixed names in panic locations and debug strings.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$repo=/x_ite_r --remap-path-prefix=$build_dir=/build --remap-path-prefix=$sysroot=/rust"
tools/build-web.sh

web=crates/x3d-web/web

# The decoder exclusions of the web CI (issue #90, docs/media-architecture.md 8.2).
python3 -I -B tools/check_no_aac.py --binary "$web"/pkg/*.wasm
python3 -I -B tools/check_no_native_video.py --cargo --binary "$web"/pkg/*.wasm

mkdir -p "$dir"

# The runtime: the adapter, the audio and video mirrors and the generated module. xr_session.mjs is not part of it
# (x_ite_r.mjs does not import it; WebXR is not supported, issue #83).
cp "$web/x_ite_r.mjs" "$web/audio_mirror.mjs" "$web/video_mirror.mjs" "$dir/"
cp -r "$web/pkg" "$dir/pkg"

# The preview page and its worlds, the example, the API list, the licenses.
cp -r "$web/preview" "$dir/preview"
cp -r "$web/example" "$dir/example"
mkdir -p "$dir/docs"
cp "$web/API.md" "$dir/docs/API.md"
cp LICENSE "$dir/LICENSE"
cp THIRD_PARTY_NOTICES.md "$dir/THIRD_PARTY_NOTICES.md"
mv "$dir/preview/DEPLOY.md" "$dir/README.md"
mv "$dir/preview/COMPATIBILITY.md" "$dir/COMPATIBILITY.md"
mv "$dir/preview/htaccess.example" "$dir/htaccess.example"
cp "$dir/preview/root-index.html" "$dir/index.html"
rm "$dir/preview/root-index.html"

wasm_bindgen=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)

python3 -I - "$dir" "$commit" "$epoch" "$(rustc --version)" "$wasm_bindgen" <<'PY'
import hashlib, json, pathlib, sys, datetime
d, commit, epoch, rustc, bindgen = sys.argv[1:]
d = pathlib.Path(d)
wasm = (d / "pkg/x3d_web_bg.wasm").read_bytes()
info = {
    "release": "x_ite_r-browser-preview-0.1.0",
    "version": "0.1.0",
    "kind": "pre-release",
    "commit": commit,
    "commit_time": datetime.datetime.fromtimestamp(int(epoch), datetime.timezone.utc).isoformat(),
    "rustc": rustc.split()[1],
    "rustc_full": rustc,
    "wasm_bindgen": bindgen,
    "target": "wasm32-unknown-unknown",
    "profile": "release",
    "wasm_sha256": hashlib.sha256(wasm).hexdigest(),
    "wasm_bytes": len(wasm),
    "scripts_default": "off",
    "reference": "X_ITE 16.5.0 (34cdf9f97)",
    "build": "tools/package-web-preview.sh (tools/build-web.sh with --remap-path-prefix)",
}
(d / "build-info.json").write_text(json.dumps(info, indent=1) + "\n")
PY

# No path or user name of this machine in the package.
if grep -rlaF -e "$HOME" -e "$repo" -e "$cargo_home" "$dir"; then
  echo "package-web-preview: the files above hold a path of this machine" >&2
  exit 1
fi

# Every module import resolves inside the package (static imports of the .mjs and .js files).
node --input-type=module - "$dir" <<'JS'
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join, dirname, resolve, relative } from "node:path";
const root = resolve (process .argv [2]);
const walk = d => readdirSync (d) .flatMap (n => statSync (join (d, n)) .isDirectory () ? walk (join (d, n)) : [ join (d, n) ]);
let bad = 0, count = 0;
for (const file of walk (root) .filter (f => /\.(m?js)$/ .test (f)))
{
   const text = readFileSync (file, "utf8");
   for (const m of text .matchAll (/(?:^|\n)\s*(?:import|export)\s[^;]*?from\s*["']([^"']+)["']|import\s*\(\s*["']([^"']+)["']\s*\)|(?:^|\n)\s*import\s*["']([^"']+)["']/g))
   {
      const spec = m [1] ?? m [2] ?? m [3];
      if (!/^\.\.?\//.test (spec)) { console .error (`bare import ${spec} in ${relative (root, file)}`); ++ bad; continue; }
      const target = resolve (dirname (file), spec);
      ++ count;
      if (!target .startsWith (root) || !existsSync (target)) { console .error (`unresolved ${spec} in ${relative (root, file)}`); ++ bad; }
   }
}
console .log (`imports: ${count} resolved, ${bad} bad`);
process .exit (bad ? 1 : 0);
JS

# Checksums of every file, then the archive with fixed owners, order and times.
(cd "$dir" && find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS)
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner --format=gnu -C "$out" -cf - "$release" | gzip -n -9 > "$out/$release.tar.gz"
(cd "$out" && sha256sum "$release.tar.gz" > "$release.tar.gz.sha256")

echo "package: $dir"
cat "$out/$release.tar.gz.sha256"
