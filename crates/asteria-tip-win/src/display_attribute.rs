//! TSF 表示属性プロバイダ (`display_attribute.cpp` 移植)。
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::Foundation::{BOOL, FALSE, S_FALSE, S_OK, TRUE};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Impl, TF_ATTR_CONVERTED, TF_ATTR_INPUT, TF_ATTR_TARGET_CONVERTED,
    TF_DA_LINESTYLE, TF_DISPLAYATTRIBUTE, TF_LS_DOT, TF_LS_SOLID,
};
use windows_core::{implement, Result, BSTR, GUID};

use crate::guids::{GUID_ATTR_CONVERTED, GUID_ATTR_FOCUSED, GUID_ATTR_INPUT};

const ATTRIBUTES: [(u32, &str, TF_DA_LINESTYLE, BOOL, u32); 3] = [
    (
        0,
        "Asteria 入力中",
        TF_LS_DOT,
        FALSE,
        TF_ATTR_INPUT.0 as u32,
    ),
    (
        1,
        "Asteria 変換済み",
        TF_LS_SOLID,
        FALSE,
        TF_ATTR_CONVERTED.0 as u32,
    ),
    (
        2,
        "Asteria 注目文節",
        TF_LS_SOLID,
        TRUE,
        TF_ATTR_TARGET_CONVERTED.0 as u32,
    ),
];

pub fn guid_for_index(idx: u32) -> GUID {
    let bytes = match idx {
        1 => GUID_ATTR_CONVERTED,
        2 => GUID_ATTR_FOCUSED,
        _ => GUID_ATTR_INPUT,
    };
    GUID::from_u128(u128::from_be_bytes(bytes))
}

#[implement(ITfDisplayAttributeInfo)]
pub struct DisplayAttributeInfo {
    index: usize,
}

impl DisplayAttributeInfo {
    pub fn new(index: u32) -> Self {
        Self {
            index: (index as usize).min(ATTRIBUTES.len() - 1),
        }
    }
}

impl ITfDisplayAttributeInfo_Impl for DisplayAttributeInfo_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(guid_for_index(self.index as u32))
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from(ATTRIBUTES[self.index].1))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        if pda.is_null() {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        let (_, _, line, bold, attr_val) = ATTRIBUTES[self.index];
        // SAFETY: TSF が渡した非 NULL 出力ポインタへ初期化済み TF_DISPLAYATTRIBUTE を書き込む。
        unsafe {
            let mut da = TF_DISPLAYATTRIBUTE {
                lsStyle: line,
                fBoldLine: bold,
                ..Default::default()
            };
            da.bAttr.0 = attr_val as i32;
            *pda = da;
        }
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

#[implement(IEnumTfDisplayAttributeInfo)]
pub struct EnumDisplayAttributeInfo {
    cursor: AtomicUsize,
}

impl EnumDisplayAttributeInfo {
    pub fn new() -> Self {
        Self {
            cursor: AtomicUsize::new(0),
        }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for EnumDisplayAttributeInfo_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let cloned = EnumDisplayAttributeInfo::new();
        cloned
            .cursor
            .store(self.cursor.load(Ordering::Relaxed), Ordering::Relaxed);
        Ok(cloned.into())
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        if rginfo.is_null() && ulcount > 0 {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        let mut fetched = 0u32;
        while fetched < ulcount {
            let idx = self.cursor.fetch_add(1, Ordering::Relaxed);
            if idx >= ATTRIBUTES.len() {
                self.cursor.store(ATTRIBUTES.len(), Ordering::Relaxed);
                break;
            }
            let info: ITfDisplayAttributeInfo = DisplayAttributeInfo::new(idx as u32).into();
            // SAFETY: `rginfo` は `ulcount` 要素分の有効配列である。
            unsafe {
                std::ptr::write(rginfo.add(fetched as usize), Some(info));
            }
            fetched += 1;
        }
        if !pcfetched.is_null() {
            // SAFETY: pcfetched は有効な u32 出力ポインタ。
            unsafe {
                *pcfetched = fetched;
            }
        }
        if fetched == ulcount {
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }

    fn Reset(&self) -> Result<()> {
        self.cursor.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn Skip(&self, ulcount: u32) -> Result<()> {
        let cur = self.cursor.load(Ordering::Relaxed);
        let next = (cur + ulcount as usize).min(ATTRIBUTES.len());
        self.cursor.store(next, Ordering::Relaxed);
        if cur + ulcount as usize <= ATTRIBUTES.len() {
            S_OK.ok()
        } else {
            Err(S_FALSE.into())
        }
    }
}

pub fn find_attribute_info(guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
    if guid.is_null() {
        return Err(windows::Win32::Foundation::E_INVALIDARG.into());
    }
    // SAFETY: 非 NULL な GUID ポインタを読み取る。
    let target = unsafe { *guid };
    for idx in 0..ATTRIBUTES.len() as u32 {
        if target == guid_for_index(idx) {
            return Ok(DisplayAttributeInfo::new(idx).into());
        }
    }
    Err(windows::Win32::Foundation::E_INVALIDARG.into())
}

pub fn make_enum() -> IEnumTfDisplayAttributeInfo {
    EnumDisplayAttributeInfo::new().into()
}
