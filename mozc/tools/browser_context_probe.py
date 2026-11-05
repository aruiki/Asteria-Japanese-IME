"""Edge(Chromium)の入力欄で、前の文が IME に届いて変換に効くかを確かめる(カード 89、2026-11-04)。

Claude のデスクトップアプリなど Electron のアプリは Edge と同じ Chromium の TSF を使う。同じ読みを、前の文が
ある欄とない欄で打ち、Space → Enter で確定した文字を比べる。導入済みの Asteria を使い、実際の SendInput で打つ
(DevTools で文字を入れない)。利用者の学習に残さないよう、local_install_check.py と同じく学習を控えて戻す。

使い方(playwright==1.58.0 を入れた Python で):
  python mozc/tools/browser_context_probe.py --driver BrowserDuplicateDriver.exe --install <導入先> --out <フォルダ>
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
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import asteria_paths  # noqa: E402

# (前の文, 読み, 前の文があるときに期待する表記)
CASES = [
    ("変換の精度が低すぎて困っています。", "せいどのはなしなんかしていない", "精度の話なんかしていない"),
    ("家の仕事は分担しています。今日は", "かじのひですね", "家事の日ですね"),
    ("カラオケで歌うとき、", "ちょっとこうおんなんですよ", "ちょっと高音なんですよ"),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--driver", type=Path, required=True)
    ap.add_argument("--install", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    if os.environ.get("KOTORI_LOCAL_ACCEPTANCE") != "owner-approved":
        raise RuntimeError("利用者が実 PC での確認を許可したときだけ動かす(KOTORI_LOCAL_ACCEPTANCE=owner-approved)")
    from playwright.sync_api import sync_playwright

    args.out.mkdir(parents=True, exist_ok=True)
    marker = "AsteriaDuplicateProbe-context-" + uuid.uuid4().hex  # 部品は窓の目印の接頭辞を確かめる

    def drive(mode, delay=0):
        result = subprocess.run([str(args.driver.resolve()), marker, mode, str(delay), str(args.install.resolve())],
                                capture_output=True, text=True, encoding="utf-8", timeout=30)
        if result.returncode:
            raise RuntimeError(f"Edge driver({mode}) exit {result.returncode}: {result.stdout} {result.stderr}")

    # 打った文が学習に残らないよう、学習のファイルを控え、最後に変換サーバーを止めて戻す(installed_probe.py と同じ)。
    profile = Path(asteria_paths.profile_dir())
    saved = args.out / "profile-backup"
    if saved.exists():
        shutil.rmtree(saved, onexc=lambda f, path, _: (os.chmod(path, stat.S_IWRITE), f(path)))
    saved.mkdir()
    for f in profile.iterdir():
        if f.is_file() and f.suffix in (".db", ".txt", ".tsv", ".ini"):
            shutil.copy2(f, saved / f.name)
    try:
        rows = run(args, drive, marker, sync_playwright)
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
    json.dump(rows, open(args.out / "context.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    reached = sum(1 for r in rows if r["context"] and r["got"] == r["expected"])
    print(f"前の文ありで期待どおり {reached}/{len(CASES)}")


def run(args, drive, marker, sync_playwright):
    rows = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(channel="msedge", headless=False)
        page = browser.new_page(viewport=dict(width=1000, height=500))
        page.set_content("""<!doctype html><meta charset=utf-8>
<style>textarea{display:block;width:900px;height:80px;font:22px sans-serif;margin:16px}</style>
<textarea id=box></textarea>
<script>window.ends=0;document.getElementById('box').addEventListener('compositionend',()=>ends++);</script>""")
        page.evaluate("title=>document.title=title", marker)
        page.bring_to_front()
        page.locator("#box").focus()
        drive("activate")
        for context, reading, expected in CASES:
            for with_context in (True, False):
                before = context if with_context else ""
                page.evaluate("""t=>{const e=document.getElementById('box');e.value=t;window.ends=0;e.focus();
                                 e.setSelectionRange(t.length,t.length)}""", before)
                drive("escape")
                drive("keys:" + to_romaji(reading))
                time.sleep(0.3)
                drive("space_enter", 1500)
                page.wait_for_function("()=>ends>=1", timeout=15000)
                time.sleep(0.5)
                value = page.evaluate("()=>document.getElementById('box').value")
                got = value[len(before):]
                rows.append(dict(context=before, reading=reading, got=got, expected=expected))
                print(("前の文あり" if with_context else "前の文なし"), reading, "→", got, flush=True)
        browser.close()
    return rows


if __name__ == "__main__":
    main()
