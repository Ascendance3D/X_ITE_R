//! Graphics backend selection and GPU error reporting for the native viewer.
//!
//! Windows: DX12 is the default backend, with the DXC shader compiler loaded from
//! `dxcompiler.dll` beside the executable. wgpu's own default on Windows tries Vulkan first and
//! compiles DX12 shaders with FXC, which needs minutes for the uber shader on a small CPU.
//!
//! - `WGPU_BACKEND` (`dx12`, `vulkan`, `gl`, … comma-separated, as wgpu names them) replaces the
//!   default. An unknown name is an error, not a silent fallback.
//! - `WGPU_DX12_COMPILER`: `dxc` / `dynamicdxc` (the default: the DLL beside the executable),
//!   `fxc` (wgpu's built-in, slow) or `auto` (wgpu's search: DXC on the DLL search path, else
//!   FXC). `staticdxc` is not built in and is an error.
//! - When DX12 is selected and `dxcompiler.dll` is missing, startup fails and says what to do;
//!   there is no automatic switch to FXC or to another backend.
//!
//! Other platforms keep wgpu's selection from the environment unchanged.
//!
//! Renderer creation runs inside wgpu error scopes ([`create_viewer`]), so a backend that cannot
//! translate the renderer's shaders (GL: depth `textureLoad` in the scatter-depth pass) is an
//! error, not a panic. Later GPU errors are collected by [`GpuErrors`] and end the run.

use crate::embed::EmbeddedViewer;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The DXC compiler DLL, beside the executable.
pub const DXC_DLL: &str = "dxcompiler.dll";

/// The oldest DXC release wgpu 30 supports.
pub const DXC_MIN_VERSION: &str = "v1.8.2502";

/// The backend names `WGPU_BACKEND` accepts (wgpu's `Backends::from_comma_list`).
const BACKEND_NAMES: &str = "vulkan (vk), dx12 (d3d12), metal (mtl), gl (gles, opengl), webgpu, noop";

/// Backends for Windows: DX12 without `WGPU_BACKEND`, otherwise exactly the listed ones.
pub fn windows_backends(env: Option<&str>) -> Result<wgpu::Backends, String> {
    let Some(list) = env else { return Ok(wgpu::Backends::DX12) };
    let mut backends = wgpu::Backends::empty();

    for name in list.split(',').map(|s| s.trim().to_lowercase()) {
        backends |= match name.as_str() {
            "vulkan" | "vk" => wgpu::Backends::VULKAN,
            "dx12" | "d3d12" => wgpu::Backends::DX12,
            "metal" | "mtl" => wgpu::Backends::METAL,
            "opengl" | "gles" | "gl" => wgpu::Backends::GL,
            "webgpu" => wgpu::Backends::BROWSER_WEBGPU,
            "noop" => wgpu::Backends::NOOP,
            _ => return Err(format!("WGPU_BACKEND={list:?}: unknown backend {name:?}; expected a comma-separated list of {BACKEND_NAMES}")),
        };
    }

    Ok(backends)
}

/// The DX12 shader compiler for `WGPU_DX12_COMPILER` (`env`); DXC from beside the executable
/// `exe` by default. `exists` checks the DLL path (a parameter for tests).
pub fn windows_dx12_compiler(env: Option<&str>, exe: Option<&Path>, exists: impl Fn(&Path) -> bool) -> Result<wgpu::Dx12Compiler, String> {
    match env.map(|s| s.trim().to_lowercase()).as_deref() {
        None | Some("dxc" | "dynamicdxc") => {}
        Some("fxc") => return Ok(wgpu::Dx12Compiler::Fxc),
        Some("auto") => return Ok(wgpu::Dx12Compiler::Auto),
        Some("staticdxc") => return Err("WGPU_DX12_COMPILER=staticdxc: this build has no statically linked DXC; use dxc (the default) or fxc".to_string()),
        Some(v) => return Err(format!("WGPU_DX12_COMPILER={v:?}: expected dxc (the default), fxc or auto")),
    }

    let path = dxc_path(exe).ok_or("DX12: the executable's directory is unknown, so dxcompiler.dll cannot be found beside it")?;

    if !exists(&path) {
        return Err(format!(
            "DX12 needs the DXC shader compiler, but {} does not exist. Put dxcompiler.dll ({DXC_MIN_VERSION} or newer, from Microsoft's DirectXShaderCompiler release) beside the executable, or choose another option: WGPU_DX12_COMPILER=fxc (slow first frame) or WGPU_BACKEND=vulkan",
            path.display()
        ));
    }

    let dxc_path = path.to_str().ok_or_else(|| format!("DX12: the DXC path {} is not valid Unicode", path.display()))?.to_string();
    Ok(wgpu::Dx12Compiler::DynamicDxc { dxc_path })
}

/// `dxcompiler.dll` in the directory of the executable `exe`; `None` unless `exe` is absolute,
/// so the path never depends on the working directory.
pub fn dxc_path(exe: Option<&Path>) -> Option<PathBuf> {
    let exe = exe.filter(|p| p.is_absolute())?;
    Some(exe.parent()?.join(DXC_DLL))
}

/// An environment variable; `None` when unset, an error when it is not Unicode.
#[cfg_attr(not(windows), allow(dead_code))]
fn env_var(key: &str) -> Result<Option<String>, String> {
    match std::env::var(key) {
        Ok(v) => Ok(Some(v)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{key} is not valid Unicode")),
    }
}

/// The instance options of the viewer: wgpu's from the environment, and on Windows the backend
/// and DX12 compiler above.
pub fn instance_descriptor() -> Result<wgpu::InstanceDescriptor, String> {
    #[allow(unused_mut)]
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();

    #[cfg(windows)]
    {
        desc.backends = windows_backends(env_var("WGPU_BACKEND")?.as_deref())?;

        if desc.backends.contains(wgpu::Backends::DX12) {
            let exe = std::env::current_exe().map_err(|e| format!("DX12: the executable's path is unknown: {e}"))?;
            desc.backend_options.dx12.shader_compiler = windows_dx12_compiler(env_var("WGPU_DX12_COMPILER")?.as_deref(), Some(&exe), Path::is_file)?;
        }
    }

    Ok(desc)
}

/// Why `request_adapter` found nothing, with what to try on Windows.
pub fn adapter_error(backends: wgpu::Backends, e: &wgpu::RequestAdapterError) -> String {
    if cfg!(windows) && backends.contains(wgpu::Backends::DX12) {
        return format!(
            "no {backends:?} adapter: {e}. DX12 needs a Direct3D 12 GPU and driver; if dxcompiler.dll beside the executable did not load, it may be older than {DXC_MIN_VERSION} or damaged. Set WGPU_BACKEND to choose another backend"
        );
    }

    format!("no adapter for the surface: {e}")
}

/// The viewer on `device`, with its renderer's shaders and pipelines checked: a backend that
/// cannot create them is an error naming the backend.
pub fn create_viewer(device: &wgpu::Device, queue: &wgpu::Queue, size: (u32, u32), adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Result<EmbeddedViewer, String> {
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let mut viewer = EmbeddedViewer::new(device.clone(), queue.clone(), size);
    viewer.configure_samples(adapter, format);
    let internal = pollster::block_on(internal.pop());
    let validation = pollster::block_on(validation.pop());

    match internal.or(validation) {
        Some(e) => Err(format!("the {:?} backend cannot run this renderer: {e}", adapter.get_info().backend)),
        None => Ok(viewer),
    }
}

/// GPU errors outside an error scope. wgpu's default handler panics; this one keeps the first
/// error for the application to report.
#[derive(Clone, Default)]
pub struct GpuErrors(Arc<Mutex<Option<String>>>);

impl GpuErrors {
    /// Collect the uncaptured errors of `device`.
    pub fn install(device: &wgpu::Device) -> GpuErrors {
        let errors = GpuErrors::default();
        let sink = errors.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| sink.record(e.to_string())));
        errors
    }

    /// Keep `message` unless an earlier error is kept.
    pub fn record(&self, message: String) {
        let mut first = self.0.lock().unwrap_or_else(|p| p.into_inner());

        if first.is_none() {
            eprintln!("GPU error: {message}");
            *first = Some(message);
        }
    }

    /// The first error since the last call.
    pub fn take(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_default_backend_is_dx12() {
        assert_eq!(windows_backends(None), Ok(wgpu::Backends::DX12));
    }

    #[test]
    fn explicit_backends_are_kept() {
        assert_eq!(windows_backends(Some("vulkan")), Ok(wgpu::Backends::VULKAN));
        assert_eq!(windows_backends(Some("VK")), Ok(wgpu::Backends::VULKAN));
        assert_eq!(windows_backends(Some("dx12")), Ok(wgpu::Backends::DX12));
        assert_eq!(windows_backends(Some("d3d12")), Ok(wgpu::Backends::DX12));
        assert_eq!(windows_backends(Some("gl")), Ok(wgpu::Backends::GL));
        assert_eq!(windows_backends(Some(" dx12 , vulkan ")), Ok(wgpu::Backends::DX12 | wgpu::Backends::VULKAN));
    }

    #[test]
    fn invalid_backends_are_errors() {
        for v in ["", "  ", "directx", "dx12,foo", "vulkan,"] {
            let e = windows_backends(Some(v)).expect_err(v);
            assert!(e.contains("WGPU_BACKEND") && e.contains("dx12 (d3d12)"), "{v:?}: {e}");
        }
    }

    /// An absolute executable path on the platform the tests run on.
    #[cfg(windows)]
    const EXE: &str = r"C:\x3d\x3d-view.exe";
    #[cfg(not(windows))]
    const EXE: &str = "/opt/x3d/x3d-view";

    fn dxc_at(path: &str) -> wgpu::Dx12Compiler {
        wgpu::Dx12Compiler::DynamicDxc { dxc_path: path.to_string() }
    }

    fn compiler(env: Option<&str>, present: bool) -> Result<String, String> {
        windows_dx12_compiler(env, Some(Path::new(EXE)), |_| present).map(|c| format!("{c:?}"))
    }

    #[test]
    fn dxc_is_loaded_from_beside_the_executable() {
        let expected = dxc_path(Some(Path::new(EXE))).unwrap();
        assert_eq!(expected, Path::new(EXE).parent().unwrap().join("dxcompiler.dll"));
        let want = format!("{:?}", dxc_at(expected.to_str().unwrap()));

        for env in [None, Some("dxc"), Some("DynamicDxc"), Some(" DXC ")] {
            let seen = std::cell::RefCell::new(vec![]);
            let c = windows_dx12_compiler(env, Some(Path::new(EXE)), |p| {
                seen.borrow_mut().push(p.to_path_buf());
                true
            });
            assert_eq!(c.map(|c| format!("{c:?}")), Ok(want.clone()), "{env:?}");
            assert_eq!(*seen.borrow(), vec![expected.clone()], "{env:?}: the checked path");
        }
    }

    #[test]
    fn dxc_path_needs_an_absolute_executable_path() {
        assert_eq!(dxc_path(Some(Path::new("x3d-view.exe"))), None);
        assert_eq!(dxc_path(Some(Path::new("bin/x3d-view.exe"))), None);
        assert_eq!(dxc_path(None), None);
        assert!(dxc_path(Some(Path::new(EXE))).unwrap().is_absolute());
        assert!(windows_dx12_compiler(None, Some(Path::new("x3d-view.exe")), |_| true).unwrap_err().contains("directory is unknown"));
    }

    #[test]
    fn missing_dxc_is_a_clear_error() {
        let e = compiler(None, false).unwrap_err();
        assert!(e.contains("dxcompiler.dll") && e.contains("does not exist"), "{e}");
        assert!(e.contains("WGPU_DX12_COMPILER=fxc") && e.contains("WGPU_BACKEND=vulkan"), "{e}");
        assert!(windows_dx12_compiler(None, None, |_| true).unwrap_err().contains("dxcompiler.dll"));
    }

    #[test]
    fn explicit_compilers_are_kept() {
        assert_eq!(compiler(Some("fxc"), false), Ok(format!("{:?}", wgpu::Dx12Compiler::Fxc)));
        assert_eq!(compiler(Some("Auto"), false), Ok(format!("{:?}", wgpu::Dx12Compiler::Auto)));
        assert!(compiler(Some("staticdxc"), true).unwrap_err().contains("no statically linked DXC"));
        assert!(compiler(Some("dcx"), true).unwrap_err().contains("expected dxc"));
    }

    #[test]
    fn gpu_errors_keep_the_first_until_taken() {
        let errors = GpuErrors::default();
        assert_eq!(errors.take(), None);
        errors.record("first".to_string());
        errors.clone().record("second".to_string());
        assert_eq!(errors.take().as_deref(), Some("first"));
        assert_eq!(errors.take(), None);
    }
}
