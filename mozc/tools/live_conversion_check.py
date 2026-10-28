"""導入済みの Asteria で、ライブ変換(docs/adr/0077)を実際のアプリ(EDIT)で確かめる(2026-10-28)。

利用者の PC で、設定の kotori_live_conversion を一時的にオンにして(config1.db に項目を足し、変換サーバーを
起動し直す)、ImeBench の live モードで打つ。Space を押さずに待ち、Enter で確定した文字を見る
(EDIT では未確定の文字は Text に入らないので、Enter の前の表示は記録だけで判定に使わない)。
終わったらプロファイル(設定と学習)を元に戻し、サーバーを起動し直す。

使い方: python mozc/tools/live_conversion_check.py --out <フォルダ> [--wait 1500]
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import asteria_paths  # noqa: E402

CASES = [("0", "nihonngo", "日本語"), ("1", "toukyou", "東京"),
         ("2", "kyouhaiitennkidesune", "今日はいい天気ですね"),
         ("3", "kaiginosiryouwookurimasu", "会議の資料を送ります")]
# config.proto の kotori_live_conversion = 208(varint の true)。protobuf は後ろに足した項目で上書きされる。
LIVE_ON = bytes([0x80, 0x0D, 0x01])


def restart_server():
    subprocess.run(["taskkill", "/F", "/IM", "mozc_server.exe"], capture_output=True)
    time.sleep(2)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--wait", type=int, default=1500, help="打ち終わってから Enter までの時間(ms)")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    profile = Path(asteria_paths.profile_dir())
    config = profile / "config1.db"
    # 打った文は学習に残るので、プロファイルを丸ごと控えて最後に戻す(設定もこれで戻る)。
    saved = args.out / "profile-backup"
    restart_server()
    if saved.exists():
        shutil.rmtree(saved, onexc=lambda f, path, _: (os.chmod(path, stat.S_IWRITE), f(path)))
    # 設定と学習のファイルだけ(通信用の .ipc などは除く)。
    saved.mkdir(parents=True)
    for f in profile.iterdir():
        if f.is_file() and f.suffix in (".db", ".txt", ".tsv", ".ini"):
            shutil.copy2(f, saved / f.name)
    backup = saved / "config1.db"
    compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
    source = Path(__file__).resolve().parents[2] / "eval" / "imebench" / "ImeBench.cs"
    exe = args.out / "ImeBench.exe"
    subprocess.run([str(compiler), "/nologo", "/target:winexe", "/platform:x64",
                    "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
                    "/out:" + str(exe), str(source)], check=True, capture_output=True)
    cases = args.out / "input.tsv"
    cases.write_text("first\tkakikukeko\n" + "".join(f"{i}\t{r}\n" for i, r, _ in CASES),
                     encoding="utf-8")
    warm = args.out / "warm.tsv"
    # 準備の入力は確かめる語と別にする(読みのまま確定すると、その語の読みを学習するため)。
    warm.write_text("warm\taiueo\n", encoding="utf-8")
    output = args.out / "output.tsv"
    bench = [str(exe), "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}", "{455A87E7-275F-4582-92CB-6600917A436D}"]
    try:
        config.write_bytes(backup.read_bytes() + LIVE_ON)
        restart_server()
        # 起動し直した変換サーバーが AI を読み込むまで待つ(読み込み中は読みのまま確定するため)。
        # 前の状態のファイルが残っていると判定を誤るので消してから、打って読み込みを促す。
        status = profile / "kotori_ai_status.txt"
        status.unlink(missing_ok=True)
        deadline = time.monotonic() + 120
        while True:
            subprocess.run([*bench, str(warm), str(args.out / "warm-out.tsv"), "500", "live"],
                           check=True, timeout=120)
            text = status.read_text(encoding="utf-8", errors="replace") if status.exists() else ""
            if "state=llm" in text or time.monotonic() > deadline:
                break
            time.sleep(3)
        subprocess.run([*bench, str(cases), str(output), str(args.wait), "live"], check=True, timeout=300)
    finally:
        restart_server()
        for f in saved.iterdir():
            target = profile / f.name
            # 変わっていないファイル(読み取り専用の鍵など)は触らない。
            if not f.is_file() or (target.exists() and target.read_bytes() == f.read_bytes()):
                continue
            if target.exists():
                os.chmod(target, stat.S_IREAD | stat.S_IWRITE)
            shutil.copy2(f, target)
    error = Path(str(output) + ".error.txt")
    if error.exists():
        raise RuntimeError(error.read_text(encoding="utf-8-sig"))
    rows = {}
    for line in output.read_text(encoding="utf-8-sig").splitlines():
        f = line.split("\t")
        if len(f) >= 3:
            rows[f[0]] = dict(committed=f[1], shown=f[2])
    result = []
    for i, romaji, expected in CASES:
        row = rows.get(i, {})
        result.append(dict(romaji=romaji, expected=expected, **row,
                           ok=row.get("committed") == expected))
    # 1 問目の前に変換サーバーを起動し直すので、warm(読み込み中)は数えない。
    report = dict(cases=result, passed=sum(r["ok"] for r in result), total=len(result),
                  profile_restored=all((profile / f.name).read_bytes() == f.read_bytes()
                                       for f in saved.iterdir() if f.is_file()))
    (args.out / "live.json").write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
