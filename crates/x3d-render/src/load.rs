//! Load a world like the render oracle (oracle/x_ite-pin/src/shot.html): parse, set up, load
//! url objects, run the frozen-time frame. Same steps as x3d_script::compare::run_world, but the
//! runtime is returned so the renderer can read the scene state.

use std::path::Path;
use x3d_runtime::{LocalFetcher, Runtime};
use x3d_script::compare::{ORACLE_PREFIX, encode_path};

/// `http://oracle/<path relative to the oracle dir>` for a file inside the oracle directory.
pub fn oracle_url(oracle_root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(oracle_root).unwrap_or(file);
    format!(
        "{ORACLE_PREFIX}{}",
        encode_path(&rel.to_string_lossy().replace('\\', "/"))
    )
}

pub fn load_world(oracle_root: &Path, file: &Path, scripts: bool) -> Result<Runtime, String> {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

    let url = oracle_url(oracle_root, file);
    let data = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let fetcher = Box::new(LocalFetcher {
        root: oracle_root.to_path_buf(),
        prefix: ORACLE_PREFIX.to_string(),
    });
    let scene = x3d_runtime::parse_bytes_with_fetcher(&*fetcher, &data, &url)
        .scene
        .map_err(|e| format!("{e:?}"))?;
    let mut rt = Runtime::new(scene, fetcher);
    crate::shader_gpu::install(&mut rt);

    if scripts {
        rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
    }

    rt.load_world();

    // The oracle keeps rendering frames at the frozen time before it reads the pixels.
    for _ in 0..3 {
        rt.frame();
        rt.run_tasks();
    }

    // Font loads take a few frames (a `sleep (0)` timer, then the fetch); x_ite-shot.mjs waits for them in its settle time, so
    // the still has the fonts. One more frame processes the rebuild they end with.
    if rt.fonts_loading() {
        rt.finish_font_loads();
        rt.frame();
        rt.run_tasks();
    }

    Ok(rt)
}
