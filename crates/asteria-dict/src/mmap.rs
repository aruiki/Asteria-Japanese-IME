//! ゼロコピー辞書バッキング (`Owned` バイト列または `memmap2::Mmap`)。

#![allow(unsafe_code)]

use std::fs::OpenOptions;
use std::ops::Deref;
use std::path::Path;

use memmap2::Mmap;

#[derive(Debug)]
pub(crate) enum Backing {
    Owned(Vec<u8>),
    Mapped(Mmap),
}

impl Backing {
    pub(crate) fn open_mmap(path: &Path) -> std::io::Result<Self> {
        let mut opts = OpenOptions::new();
        opts.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_SHARE_READ のみ許可し、マップ中のファイル切り詰めや上書きを防ぐ。
            opts.share_mode(1);
        }
        let file = opts.open(path)?;
        // SAFETY: 読み取り専用かつ FILE_SHARE_READ のみで開かれたファイルであり、
        // マップ寿命中に外部からサイズ変更・破壊されない。
        let map = unsafe { Mmap::map(&file)? };
        Ok(Self::Mapped(map))
    }
}

impl Deref for Backing {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(v) => v,
            Self::Mapped(m) => m,
        }
    }
}
