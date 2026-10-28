"""導入済みの Asteria に、実際のアプリ(EDIT)で読みを打って Space で確定した文字を見る(調査用。2026-10-28)。

利用者の PC で今の設定のまま打つ。打った文は学習に残るので、設定と学習のファイルを控えて最後に戻す。

使い方: python mozc/tools/installed_probe.py --out <フォルダ> ローマ字 [ローマ字 ...] [--wait 3000] [--live]
"""
import argparse
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import asteria_paths  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--wait", type=int, default=3000)
    ap.add_argument("--live", action="store_true", help="Space を押さずに Enter(ライブ変換)")
    ap.add_argument("keys", nargs="+")
    args = ap.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
    source = Path(__file__).resolve().parents[2] / "eval" / "imebench" / "ImeBench.cs"
    exe = out / "ImeBench.exe"
    subprocess.run([str(compiler), "/nologo", "/target:winexe", "/platform:x64",
                    "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
                    "/out:" + str(exe), str(source)], check=True, capture_output=True)
    cases = out / "input.tsv"
    cases.write_text("warm\taiueo\n" + "".join(f"{i}\t{k}\n" for i, k in enumerate(args.keys)),
                     encoding="utf-8")
    output = out / "output.tsv"
    profile = Path(asteria_paths.profile_dir())
    saved = out / "profile-backup"
    if saved.exists():
        shutil.rmtree(saved, onexc=lambda f, path, _: (os.chmod(path, stat.S_IWRITE), f(path)))
    saved.mkdir()
    for f in profile.iterdir():
        if f.is_file() and f.suffix in (".db", ".txt", ".tsv", ".ini"):
            shutil.copy2(f, saved / f.name)
    try:
        subprocess.run([str(exe), "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
                        "{455A87E7-275F-4582-92CB-6600917A436D}", str(cases), str(output),
                        str(args.wait), *(["live"] if args.live else [])], check=True, timeout=600)
    finally:
        subprocess.run(["taskkill", "/F", "/IM", "mozc_server.exe"], capture_output=True)
        time.sleep(2)
        for f in saved.iterdir():
            target = profile / f.name
            if target.exists() and target.read_bytes() == f.read_bytes():
                continue
            if target.exists():
                os.chmod(target, stat.S_IREAD | stat.S_IWRITE)
            shutil.copy2(f, target)
    print(output.read_text(encoding="utf-8-sig"))


if __name__ == "__main__":
    main()
