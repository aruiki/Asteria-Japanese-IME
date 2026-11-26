//! Windows 実行時 `llama.dll` / `ggml*.dll` 動的ロードおよび GPU 選別 (`llama_runtime.{h,cc}` 移植)。
#![cfg_attr(windows, allow(unsafe_code))]

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const MIN_DISCRETE_GPU_BYTES: usize = 3 << 30; // 3 GiB (ADR0024)
const MIN_INTEGRATED_GPU_BYTES: usize = 2 << 30; // 2 GiB (ADR0078)

static ALLOW_INTEGRATED: AtomicBool = AtomicBool::new(false);

pub fn allow_integrated_gpu(allow: bool) {
    ALLOW_INTEGRATED.store(allow, Ordering::Relaxed);
}

pub fn integrated_gpu_allowed() -> bool {
    ALLOW_INTEGRATED.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuKind {
    NoneCpuOnly,
    Discrete,
    Integrated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuDeviceInfo {
    pub kind: GpuKind,
    pub name: String,
    pub total_vram_bytes: usize,
}

impl Default for GpuDeviceInfo {
    fn default() -> Self {
        Self {
            kind: GpuKind::NoneCpuOnly,
            name: String::new(),
            total_vram_bytes: 0,
        }
    }
}

fn pick_by_kind(
    devices: &[(GpuKind, String, usize)],
    target_kind: GpuKind,
    min_bytes: usize,
) -> Option<GpuDeviceInfo> {
    let mut best: Option<GpuDeviceInfo> = None;
    for (k, name, mem) in devices {
        if *k == target_kind
            && *mem >= min_bytes
            && best
                .as_ref()
                .map_or(true, |cur| *mem > cur.total_vram_bytes)
        {
            best = Some(GpuDeviceInfo {
                kind: target_kind,
                name: name.clone(),
                total_vram_bytes: *mem,
            });
        }
    }
    best
}

/// GPU 選定純粋ロジック (`SelectGpu` / `PickGpu` 移植)。テスト容易化のためデバイス表を受け取る。
pub fn pick_best_gpu(
    devices: &[(GpuKind, String, usize)],
    force_cpu: bool,
    allow_integrated: bool,
) -> GpuDeviceInfo {
    if force_cpu {
        return GpuDeviceInfo::default();
    }
    if let Some(d) = pick_by_kind(devices, GpuKind::Discrete, MIN_DISCRETE_GPU_BYTES) {
        return d;
    }
    if !allow_integrated {
        return GpuDeviceInfo::default();
    }
    pick_by_kind(devices, GpuKind::Integrated, MIN_INTEGRATED_GPU_BYTES).unwrap_or_default()
}

#[cfg(windows)]
mod platform {
    use super::{GpuDeviceInfo, GpuKind};
    use std::collections::HashMap;
    use std::ffi::{c_char, c_void, CStr};
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, SetDllDirectoryW, LOAD_WITH_ALTERED_SEARCH_PATH,
    };

    type CachedDevs = Vec<(GpuKind, String, usize)>;
    static PROBE_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedDevs>>> = OnceLock::new();

    const GGML_TYPE_GPU: i32 = 1;
    const GGML_TYPE_IGPU: i32 = 2;

    type FnDevCount = unsafe extern "C" fn() -> usize;
    type FnDevGet = unsafe extern "C" fn(usize) -> *mut c_void;
    type FnDevType = unsafe extern "C" fn(*mut c_void) -> i32;
    type FnDevMem = unsafe extern "C" fn(*mut c_void, *mut usize, *mut usize);
    type FnDevDesc = unsafe extern "C" fn(*mut c_void) -> *const c_char;
    type FnBackendLoadAllFromPath = unsafe extern "C" fn(*const c_char);

    fn load_dll(path: &Path) -> Option<HMODULE> {
        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: wide は NUL 終端 UTF-16 パスであり、失敗時は NULL を返す。
        let h = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        };
        if h.is_null() {
            None
        } else {
            Some(h)
        }
    }

    type ProbeResult = Result<(bool, Vec<(GpuKind, String, usize)>), String>;

    pub fn probe_runtime_gpus(runtime_dir: &Path) -> ProbeResult {
        let key = std::fs::canonicalize(runtime_dir).unwrap_or_else(|_| runtime_dir.to_path_buf());
        let cache = PROBE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(guard) = cache.lock() {
            if let Some(devs) = guard.get(&key) {
                return Ok((true, devs.clone()));
            }
        }

        let wide_dir: Vec<u16> = runtime_dir
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: NUL 終端パスを Win32 SetDllDirectoryW へ渡す。
        unsafe { SetDllDirectoryW(wide_dir.as_ptr()) };

        let ggml_base = load_dll(&runtime_dir.join("ggml-base.dll"));
        let ggml = load_dll(&runtime_dir.join("ggml.dll"));
        let llama = load_dll(&runtime_dir.join("llama.dll"));

        let Some(llama_mod) = llama else {
            // SAFETY: ロード済みモジュールが存在する場合のみ解放する。
            unsafe {
                if let Some(m) = ggml {
                    FreeLibrary(m);
                }
                if let Some(m) = ggml_base {
                    FreeLibrary(m);
                }
            }
            return Err("llama.dll を読み込めません".into());
        };

        let mut list = Vec::new();
        let sym_mod = ggml_base.unwrap_or(llama_mod);
        // SAFETY: ggml-base.dll / llama.dll から既知の C シグネチャシンボルを取得して検査する。
        unsafe {
            if let (Some(f_cnt), Some(f_get), Some(f_ty), Some(f_mem)) = (
                GetProcAddress(sym_mod, c"ggml_backend_dev_count".as_ptr().cast()),
                GetProcAddress(sym_mod, c"ggml_backend_dev_get".as_ptr().cast()),
                GetProcAddress(sym_mod, c"ggml_backend_dev_type".as_ptr().cast()),
                GetProcAddress(sym_mod, c"ggml_backend_dev_memory".as_ptr().cast()),
            ) {
                if let Some(f_load) =
                    GetProcAddress(sym_mod, c"ggml_backend_load_all_from_path".as_ptr().cast())
                {
                    if let Ok(cdir) =
                        std::ffi::CString::new(runtime_dir.to_string_lossy().as_bytes())
                    {
                        let load_fn: FnBackendLoadAllFromPath = std::mem::transmute(f_load);
                        load_fn(cdir.as_ptr());
                    }
                }
                let cnt: FnDevCount = std::mem::transmute(f_cnt);
                let get: FnDevGet = std::mem::transmute(f_get);
                let ty: FnDevType = std::mem::transmute(f_ty);
                let mem: FnDevMem = std::mem::transmute(f_mem);
                let desc_opt: Option<FnDevDesc> =
                    GetProcAddress(sym_mod, c"ggml_backend_dev_description".as_ptr().cast())
                        .map(|p| std::mem::transmute(p));

                for i in 0..cnt() {
                    let dev = get(i);
                    let t = ty(dev);
                    let kind = match t {
                        GGML_TYPE_GPU => GpuKind::Discrete,
                        GGML_TYPE_IGPU => GpuKind::Integrated,
                        _ => continue,
                    };
                    let mut free = 0usize;
                    let mut total = 0usize;
                    mem(dev, &mut free, &mut total);
                    let name = desc_opt
                        .and_then(|f| {
                            let p = f(dev);
                            if p.is_null() {
                                None
                            } else {
                                Some(CStr::from_ptr(p).to_string_lossy().into_owned())
                            }
                        })
                        .unwrap_or_default();
                    list.push((kind, name, total));
                }
            }
            // ggml.dll の static terminate ハンドラ再初期化 (`GGML_ASSERT(prev != ggml_uncaught_exception)`)
            // を防ぐため、一度ロードした ggml*/llama.dll はプロセス終了まで保持する。
            let _ = (llama_mod, ggml, ggml_base);
        }
        if let Ok(mut guard) = cache.lock() {
            guard.insert(key, list.clone());
        }
        Ok((true, list))
    }

    pub fn detect_gpu(runtime_dir: &Path, allow_integrated: bool) -> GpuDeviceInfo {
        let force_cpu = std::env::var("ASTERIA_LM_DEVICE").as_deref() == Ok("cpu");
        let Ok((_, devs)) = probe_runtime_gpus(runtime_dir) else {
            return GpuDeviceInfo::default();
        };
        super::pick_best_gpu(&devs, force_cpu, allow_integrated)
    }
}

#[cfg(not(windows))]
mod platform {
    use super::GpuDeviceInfo;
    use std::path::Path;

    pub fn detect_gpu(_runtime_dir: &Path, _allow_integrated: bool) -> GpuDeviceInfo {
        GpuDeviceInfo::default()
    }
}

/// 実行時フォルダの `llama.dll` / `ggml*.dll` を検出し、規定に従い利用 GPU を決定する。
pub fn detect_runtime_gpu(runtime_dir: &Path) -> GpuDeviceInfo {
    platform::detect_gpu(runtime_dir, integrated_gpu_allowed())
}

/// 推論バックエンドとして実行時 `llama.dll` / GPU を初期化し、実デバイス情報を返す。
pub fn init_runtime_backend(
    runtime_dir: &Path,
    gpu_enabled: bool,
    allow_integrated: bool,
) -> (crate::RuntimeBackendInfo, GpuDeviceInfo) {
    allow_integrated_gpu(gpu_enabled && allow_integrated);
    let backend = crate::init_runtime(Some(runtime_dir), gpu_enabled, allow_integrated);
    let gpu = if backend.has_gpu {
        GpuDeviceInfo {
            kind: if backend.is_integrated {
                GpuKind::Integrated
            } else {
                GpuKind::Discrete
            },
            name: if backend.is_cuda && !backend.gpu_name.contains("CUDA") {
                format!("{} (CUDA)", backend.gpu_name)
            } else {
                backend.gpu_name.clone()
            },
            total_vram_bytes: backend.gpu_vram_bytes,
        }
    } else {
        GpuDeviceInfo::default()
    };
    (backend, gpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_selection_respects_thresholds_and_flags() {
        let devs = vec![
            (GpuKind::Integrated, "Intel iGPU".into(), 4 << 30),
            (GpuKind::Discrete, "RTX Small".into(), 2 << 30),
            (GpuKind::Discrete, "RTX 3060".into(), 12 << 30),
        ];
        let picked = pick_best_gpu(&devs, false, false);
        assert_eq!(picked.kind, GpuKind::Discrete);
        assert_eq!(picked.name, "RTX 3060");

        let only_igpu = vec![(GpuKind::Integrated, "Radeon iGPU".into(), 4 << 30)];
        assert_eq!(
            pick_best_gpu(&only_igpu, false, false).kind,
            GpuKind::NoneCpuOnly
        );
        assert_eq!(
            pick_best_gpu(&only_igpu, false, true).kind,
            GpuKind::Integrated
        );
        assert_eq!(pick_best_gpu(&devs, true, true).kind, GpuKind::NoneCpuOnly);
    }
}
