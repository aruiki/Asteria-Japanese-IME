//! Asteria IME Windows TSF TIP COM DLL (`asteria_tip.dll`).
#![cfg_attr(not(windows), deny(unsafe_code))]
#![cfg_attr(windows, allow(unsafe_code))]

pub mod guids;
pub mod text;

#[cfg(windows)]
mod display_attribute;
#[cfg(windows)]
mod engine_bridge;
#[cfg(windows)]
mod mode_item;
#[cfg(windows)]
mod register;
#[cfg(windows)]
mod text_service;

#[cfg(windows)]
mod com_exports {
    use core::ffi::c_void;
    use std::sync::atomic::Ordering;

    use windows::Win32::Foundation::{
        BOOL, CLASS_E_CLASSNOTAVAILABLE, E_INVALIDARG, HINSTANCE, S_FALSE, S_OK, TRUE,
    };
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, IClassFactory, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
    use windows_core::{Interface, GUID, HRESULT};

    use crate::guids::CLSID_TEXT_SERVICE_U128;
    use crate::register::{register_all, unregister_all};
    use crate::text_service::{ClassFactory, G_DLL_REFS, G_MODULE};

    #[no_mangle]
    pub extern "system" fn DllMain(inst: HINSTANCE, reason: u32, _reserved: *const c_void) -> BOOL {
        if reason == DLL_PROCESS_ATTACH {
            // SAFETY: DllMain DLL_PROCESS_ATTACH 時に一度だけ HMODULE を記録する。
            unsafe {
                G_MODULE = inst.into();
            }
        }
        TRUE
    }

    #[no_mangle]
    pub unsafe extern "system" fn DllGetClassObject(
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        if ppv.is_null() || rclsid.is_null() || riid.is_null() {
            return E_INVALIDARG;
        }
        *ppv = core::ptr::null_mut();
        if *rclsid != GUID::from_u128(CLSID_TEXT_SERVICE_U128) {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = ClassFactory.into();
        factory.query(riid, ppv)
    }

    #[no_mangle]
    pub extern "system" fn DllCanUnloadNow() -> HRESULT {
        if G_DLL_REFS.load(Ordering::Relaxed) == 0 {
            S_OK
        } else {
            S_FALSE
        }
    }

    #[no_mangle]
    pub extern "system" fn DllRegisterServer() -> HRESULT {
        // SAFETY: 一時的な COM STA 初期化後にレジストリ/TSF 登録を実行する。
        unsafe {
            let co = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let res = register_all(G_MODULE);
            if co.is_ok() {
                CoUninitialize();
            }
            match res {
                Ok(()) => S_OK,
                Err(e) => e.code(),
            }
        }
    }

    #[no_mangle]
    pub extern "system" fn DllUnregisterServer() -> HRESULT {
        // SAFETY: 一時的な COM STA 初期化後にレジストリ/TSF 登録を解除する。
        unsafe {
            let co = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let res = unregister_all();
            if co.is_ok() {
                CoUninitialize();
            }
            match res {
                Ok(()) => S_OK,
                Err(e) => e.code(),
            }
        }
    }
}
