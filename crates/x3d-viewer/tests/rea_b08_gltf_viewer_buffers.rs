//! Batch 08 integration: a top-level glTF world loaded by Viewer3d (and so EmbeddedViewer) fetches
//! its external `.bin` buffer through the runtime fetcher, like x3d-render's load.rs. Before the
//! integration fix the viewer parsed with plain `x3d_parse::parse_bytes`, whose default fetch only
//! resolves `file://` bases, so the `http://oracle/` world failed. No GPU is needed.

use std::path::PathBuf;
use x3d_runtime::loader::LocalFetcher;
use x3d_script::compare::ORACLE_PREFIX;
use x3d_viewer::viewer::Viewer3d;

#[test]
fn top_level_gltf_with_external_buffer_loads_in_the_viewer() {
    let oracle = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let world = oracle.join("synthetic-rea-b08-gltf/buffer/external-bin.gltf");
    let url = format!("{ORACLE_PREFIX}synthetic-rea-b08-gltf/buffer/external-bin.gltf");
    let data = std::fs::read(&world).unwrap();

    // The parse that gltf_rea compares with X_ITE: same fetcher, same URL.
    let fetcher = LocalFetcher {
        root: oracle.clone(),
        prefix: ORACLE_PREFIX.to_string(),
    };
    let want = x3d_runtime::parse_bytes_with_fetcher(&fetcher, &data, &url)
        .expect("parse with fetcher")
        .to_vrml_string();
    assert!(
        want.contains("point"),
        "reference parse has no coordinates:\n{want}"
    );
    // The old viewer path: without the fetcher the buffer is not found.
    assert_ne!(
        x3d_parse::parse_bytes(&data, &url)
            .ok()
            .map(|s| s.to_vrml_string()),
        Some(want.clone())
    );

    let v = Viewer3d::load(&world, Some(&oracle), 946684800.0, 64, 64).expect("viewer load");
    assert_eq!(v.rt.scene.to_vrml_string(), want);
}
