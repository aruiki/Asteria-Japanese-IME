"""導入先・保存先と IME の DLL の場所(v3.0.0 で Kotori → Asteria、DLL は版ごとの名前。docs/adr/0072)。

検査の道具は、旧版(Program Files の Kotori、mozc_tip64.dll)から新しい版へ更新する前後の両方を扱う。
"""
import os
from pathlib import Path

PROGRAM_FILES = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"))
LOCAL_LOW = Path(os.environ.get("USERPROFILE", "~")).expanduser() / "AppData" / "LocalLow"


def _new_or_legacy(base):
    new, legacy = base / "Asteria", base / "Kotori"
    # 移した後の旧名の場所はジャンクション(新しい保存先を指す)なので、新しい方を優先する。
    return new if new.exists() or not legacy.exists() else legacy


def install_dir():
    return _new_or_legacy(PROGRAM_FILES)


def profile_dir():
    return _new_or_legacy(LOCAL_LOW)


def tip_path(directory, bits="64"):
    """directory の IME の DLL。版ごとの名前(mozc_tip64-<版>.dll)か、旧版の mozc_tip64.dll。

    退けた DLL(*.asteria-old)は数えない。複数あれば名前の最も新しいもの。見つからなければ旧名のパス。
    """
    directory = Path(directory)
    prefix = f"mozc_tip{bits}"
    found = sorted(p for p in directory.glob(prefix + "*.dll")
                   if p.name == prefix + ".dll" or p.name.startswith(prefix + "-"))
    return found[-1] if found else directory / (prefix + ".dll")


def is_tip_module(module_path, install, bits="64"):
    """読み込まれたモジュールが、install にある IME の DLL かどうか。"""
    module_path = Path(module_path)
    prefix = f"mozc_tip{bits}"
    named = module_path.name.lower() == prefix + ".dll" or (
        module_path.name.lower().startswith(prefix + "-") and module_path.suffix.lower() == ".dll")
    return named and module_path.parent.resolve() == Path(install).resolve()
