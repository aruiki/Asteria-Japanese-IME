//! Asteria TSF COM GUID・定数定義。

pub const DESCRIPTION: &str = "Asteria";
pub const LANG_ID_JA: u16 = 0x0411;

/// テキストサービス CLSID: {797AD563-3368-4DCB-B3E7-8B0306B42B67}
pub const CLSID_TEXT_SERVICE_U128: u128 = 0x797ad563_3368_4dcb_b3e7_8b0306b42b67;
/// 日本語言語プロファイル GUID: {D8E58E42-4B5C-4E34-88D5-355F38FE91A4}
pub const GUID_PROFILE_U128: u128 = 0xd8e58e42_4b5c_4e34_88d5_355f38fe91a4;

pub const GUID_ATTR_INPUT: [u8; 16] = 0x7a1ece80_81f1_4304_845d_b78e92e7d75d_u128.to_be_bytes();
pub const GUID_ATTR_CONVERTED: [u8; 16] = 0x26898b1a_2acf_4c7d_b666_11ed66c55053_u128.to_be_bytes();
pub const GUID_ATTR_FOCUSED: [u8; 16] = 0x6786df7d_0858_4447_87d1_ed6710c2e21b_u128.to_be_bytes();
