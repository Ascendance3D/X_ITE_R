#!/usr/bin/env bash
# Build and package an X_ITE_R browser release (Browser Preview or Canary): the browser module of crates/x3d-web, the
# preview page, its clean-room test worlds and the example, as a directory and a .tar.gz with SHA-256 checksums.
#
#   tools/package-web-preview.sh --release <ID> [OUT_DIR]
#
# ID is the release identity and Git tag, for example canary-0.2.0 or browser-preview-0.1.0. It gives the package name
# (x_ite_r-<ID>), the version, and the title on the pages. The tracked sources must carry the same version (browser.version,
# the x3d-viewer crate version of the menu title, the page tags and titles); a mismatch stops the package.
# OUT_DIR (default target/web-preview) must not hold the package yet. The working tree must be clean: the package
# records the commit it is built from (build-info.json), and the same commit and toolchain give the same files. The
# source files come from that commit (git archive), so an untracked or changed file never enters the package; pkg/ is
# generated new. Every file has mode 644 and every directory 755, whatever the umask.
# The build is tools/build-web.sh with --remap-path-prefix flags, so the module holds no path of this machine (a check
# below fails the package when one is left). Needs what tools/build-web.sh needs, and Node for the import check.
set -euo pipefail

cd "$(dirname "$0")/.."
repo=$(pwd)

if [[ "${1:-}" != --release || -z "${2:-}" ]]; then
  echo "usage: tools/package-web-preview.sh --release <ID> [OUT_DIR]   (ID: canary-X.Y.Z or browser-preview-X.Y.Z)" >&2
  exit 2
fi
release_id=$2
shift 2
# The ID gives the package name, the version and the page title (tools/release_identity.py).
ident=$(python3 -I -B tools/release_identity.py parse "$release_id") || exit 2
IFS=$'\t' read -r release version title <<<"$ident"
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

# The crate notices are generated; a stale file must not ship.
python3 -I -B tools/third_party_notices.py --check

commit=$(git rev-parse HEAD)

# The tracked sources at the commit must carry this identity: the version strings, and page labels of this channel and
# version only (tools/release_identity.py).
python3 -I -B tools/release_identity.py check "$release_id" "$commit"
web=crates/x3d-web/web
epoch=$(git log -1 --format=%ct HEAD)
cargo_home=${CARGO_HOME:-$HOME/.cargo}
sysroot=$(rustc --print sysroot)
build_dir=${CARGO_BUILD_BUILD_DIR:-$repo/target}

# Paths of this machine become fixed names in panic locations and debug strings.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$repo=/x_ite_r --remap-path-prefix=$build_dir=/build --remap-path-prefix=$sysroot=/rust"
# pkg/ is generated (gitignored): a file left by an older build must not enter the package.
rm -rf crates/x3d-web/web/pkg
tools/build-web.sh

# The decoder exclusions of the web CI (issue #90, docs/media-architecture.md 8.2).
python3 -I -B tools/check_no_aac.py --binary "$web"/pkg/*.wasm
python3 -I -B tools/check_no_opus.py --binary "$web"/pkg/*.wasm
python3 -I -B tools/check_no_native_video.py --cargo --binary "$web"/pkg/*.wasm

mkdir -p "$dir"

# The tracked files of `path` at the commit, into `dest` (a new directory).
from_commit() {
  mkdir -p "$2"
  git archive --format=tar "$commit" "$1" | tar -x -C "$2" --strip-components="$(( $(tr -cd / <<<"$1" | wc -c) + 1 ))" --no-same-permissions
}

# The runtime: the adapter, the audio and video mirrors, the WebXR session and layer (issue #83) and the generated module.
for f in x_ite_r.mjs audio_mirror.mjs video_mirror.mjs xr_session.mjs xr_layer.mjs; do
  git show "$commit:$web/$f" > "$dir/$f"
done
cp -r "$web/pkg" "$dir/pkg"

# The preview page and its worlds, the example, the API list, the licenses.
from_commit "$web/preview" "$dir/preview"
from_commit "$web/example" "$dir/example"
# The start page's style, module, logo and fonts (Geist, JetBrains Mono and Plus Jakarta Sans, SIL OFL 1.1: their
# notices are site/fonts/OFL*.txt).
from_commit "$web/site" "$dir/site"
mkdir -p "$dir/docs"
git show "$commit:$web/API.md" > "$dir/docs/API.md"
git show "$commit:LICENSE" > "$dir/LICENSE"
git show "$commit:THIRD_PARTY_NOTICES.md" > "$dir/THIRD_PARTY_NOTICES.md"
git show "$commit:THIRD_PARTY_CRATES.md" > "$dir/THIRD_PARTY_CRATES.md"
# The Source Code Form of the MPL-2.0 crates in the module (MPL-2.0 3.2 (a)): their published crate archives, each
# checked against the Cargo.lock checksum, with a README and the license text. The method is an owner decision after
# legal review (docs/codec-licensing-inventory.md); the package carries the archives until then.
python3 -I -B tools/mpl_sources.py --write "$dir"
python3 -I -B tools/mpl_sources.py --verify "$dir"
mv "$dir/preview/DEPLOY.md" "$dir/README.md"
mv "$dir/preview/COMPATIBILITY.md" "$dir/COMPATIBILITY.md"
mv "$dir/preview/htaccess.example" "$dir/htaccess.example"
cp "$dir/preview/root-index.html" "$dir/index.html"
rm "$dir/preview/root-index.html"

# HTML pages of the two documents, in the package's design; the Markdown files stay beside them.
python3 -I -B tools/md_page.py "$dir/docs/API.md" "$dir/docs/api.html" --root ../ --current API \
  --eyebrow "Documentation · JavaScript API" --tag "$title" --source-link API.md
python3 -I -B tools/md_page.py "$dir/COMPATIBILITY.md" "$dir/compatibility.html" --root "" --current Compatibility \
  --eyebrow "Compatibility · $title" --tag "$title" --source-link COMPATIBILITY.md

wasm_bindgen=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)

python3 -I - "$dir" "$commit" "$epoch" "$(rustc --version)" "$wasm_bindgen" "$release" "$version" <<'PY'
import hashlib, json, pathlib, sys, datetime
d, commit, epoch, rustc, bindgen, release, version = sys.argv[1:]
d = pathlib.Path(d)
wasm = (d / "pkg/x3d_web_bg.wasm").read_bytes()
info = {
    "release": release,
    "version": version,
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
# Modes after SHA256SUMS exists, so it gets them too (a mode is not part of a checksum).
find "$dir" -type d -exec chmod 755 {} +
find "$dir" -type f -exec chmod 644 {} +
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner --format=gnu -C "$out" -cf - "$release" | gzip -n -9 > "$out/$release.tar.gz"
(cd "$out" && sha256sum "$release.tar.gz" > "$release.tar.gz.sha256")

echo "package: $dir"
cat "$out/$release.tar.gz.sha256"
