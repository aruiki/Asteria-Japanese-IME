//! COM サーバーおよび TSF プロファイル・カテゴリ登録 (`register.cpp` 移植)。
#![allow(unsafe_code)]

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HMODULE, WIN32_ERROR};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CLASSES_ROOT,
    KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIP_KEYBOARD,
};
use windows_core::{GUID, HRESULT, HSTRING, PCWSTR};

use crate::guids::{CLSID_TEXT_SERVICE_U128, DESCRIPTION, GUID_PROFILE_U128, LANG_ID_JA};

const CATEGORIES: &[GUID] = &[
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];

fn clsid_guid() -> GUID {
    GUID::from_u128(CLSID_TEXT_SERVICE_U128)
}

fn profile_guid() -> GUID {
    GUID::from_u128(GUID_PROFILE_U128)
}

fn format_guid(g: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7],
    )
}

fn module_path(module: HMODULE) -> Option<String> {
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: DllMain で取得した HMODULE から自身の DLL パスを取得する。
        let n = unsafe { GetModuleFileNameW(module, &mut buf) } as usize;
        if n == 0 {
            return None;
        }
        if n < buf.len() {
            return Some(String::from_utf16_lossy(&buf[..n]));
        }
        buf.resize(buf.len() * 2, 0);
    }
}

fn write_clsid_key(subkey: &str, name: Option<&str>, value: &str) -> windows_core::Result<()> {
    let subkey_w = HSTRING::from(subkey);
    let mut key = HKEY::default();
    // SAFETY: NUL 終端 UTF-16 subkey に対して RegCreateKeyExW を呼ぶ。
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(subkey_w.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(WIN32_ERROR(status.0).into());
    }
    let val_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = std::mem::size_of_val(val_w.as_slice());
    let name_w = name.map(HSTRING::from);
    let name_ptr = name_w
        .as_ref()
        .map_or(PCWSTR::null(), |s| PCWSTR(s.as_ptr()));
    // SAFETY: val_w は有効な NUL 終端 UTF-16 バッファであり、`align_to`/`from_raw_parts` が安全。
    let bytes = unsafe { std::slice::from_raw_parts(val_w.as_ptr().cast::<u8>(), byte_len) };
    // SAFETY: key は有効なオープン済み HKEY。
    let write_status = unsafe { RegSetValueExW(key, name_ptr, 0, REG_SZ, Some(bytes)) };
    // SAFETY: RegCreateKeyExW が返した HKEY を閉じる。
    unsafe {
        let _ = RegCloseKey(key);
    }
    if write_status != ERROR_SUCCESS {
        return Err(WIN32_ERROR(write_status.0).into());
    }
    Ok(())
}

pub fn register_all(module: HMODULE) -> windows_core::Result<()> {
    let clsid_str = format_guid(&clsid_guid());
    let clsid_key = format!("CLSID\\{clsid_str}");
    let inproc = format!("{clsid_key}\\InprocServer32");
    let path =
        module_path(module).ok_or_else(|| windows_core::Error::from(HRESULT(-2147467259)))?;

    write_clsid_key(&clsid_key, None, DESCRIPTION)?;
    write_clsid_key(&inproc, None, &path)?;
    write_clsid_key(&inproc, Some("ThreadingModel"), "Apartment")?;

    // SAFETY: COM 初期化済みのスレッド上で TSF ProfileMgr / CategoryMgr を取得し登録する。
    unsafe {
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let desc_w: Vec<u16> = DESCRIPTION.encode_utf16().collect();
        let path_w: Vec<u16> = path.encode_utf16().collect();
        profile_mgr.RegisterProfile(
            &clsid_guid(),
            LANG_ID_JA,
            &profile_guid(),
            &desc_w,
            &path_w,
            0,
            None,
            0,
            true,
            0,
        )?;

        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for cat in CATEGORIES {
            cat_mgr.RegisterCategory(&clsid_guid(), cat, &clsid_guid())?;
        }
    }
    Ok(())
}

pub fn unregister_all() -> windows_core::Result<()> {
    // SAFETY: 可能な範囲でプロファイル・カテゴリ・CLSID を削除する。
    unsafe {
        if let Ok(cat_mgr) =
            CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
        {
            for cat in CATEGORIES {
                let _ = cat_mgr.UnregisterCategory(&clsid_guid(), cat, &clsid_guid());
            }
        }
        if let Ok(profile_mgr) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profile_mgr.UnregisterProfile(&clsid_guid(), LANG_ID_JA, &profile_guid(), 0);
        }
        let clsid_key = HSTRING::from(format!("CLSID\\{}", format_guid(&clsid_guid())));
        let status = RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(clsid_key.as_ptr()));
        if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
            return Err(WIN32_ERROR(status.0).into());
        }
    }
    Ok(())
}
