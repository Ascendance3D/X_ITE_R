#!/usr/bin/env bash
# Fetch Microsoft's DXC shader compiler (dxcompiler.dll) for the Windows x3d-view and put it
# beside the executable, where x3d-view loads it for DX12 (crates/x3d-viewer/src/backend.rs).
# Local only: nothing here publishes or pushes. The DLL is not kept in Git.
#
# Usage: tools/fetch-dxc.sh <dir-of-x3d-view.exe>
#
# Source: the official GitHub release of microsoft/DirectXShaderCompiler, the version wgpu 30's
# own Windows CI uses. Both the archive and the DLL are checked against pinned SHA-256 values;
# a mismatch stops the script. The archive's license files are copied beside the DLL.
#
# Redistribution: dxcompiler.dll is built from the DXC sources (University of Illinois/NCSA
# license, LICENSE-LLVM.txt; MIT parts, LICENSE-MIT.txt). The archive also carries Microsoft
# license terms (LICENSE-MS.txt) that permit distributing only listed "distributables". Owner
# review of these terms is required before the DLL is shipped in a published package.
set -euo pipefail

release=v1.8.2505.1
archive=dxc_2025_07_14.zip
archive_sha256=9ad895a6b039e3a8f8c22a1009f866800b840a74b50db9218d13319e215ea8a4
dll_sha256=5888d3e590f5bfd8743484e7f31313996734adec5741506271a5c6368cbe8ba9

out=${1:?usage: tools/fetch-dxc.sh <dir-of-x3d-view.exe>}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

curl -fsSL --retry 3 -o "$work/$archive" "https://github.com/microsoft/DirectXShaderCompiler/releases/download/$release/$archive"
echo "$archive_sha256  $work/$archive" | sha256sum -c --quiet -

unzip -q -d "$work/x" "$work/$archive" bin/x64/dxcompiler.dll LICENSE-LLVM.txt LICENSE-MIT.txt LICENSE-MS.txt
echo "$dll_sha256  $work/x/bin/x64/dxcompiler.dll" | sha256sum -c --quiet -

mkdir -p "$out" "$out/dxc-licenses"
install -m 644 "$work/x/bin/x64/dxcompiler.dll" "$out/dxcompiler.dll"
install -m 644 "$work/x/LICENSE-LLVM.txt" "$work/x/LICENSE-MIT.txt" "$work/x/LICENSE-MS.txt" "$out/dxc-licenses/"
echo "dxcompiler.dll ($release, sha256 $dll_sha256) -> $out"
