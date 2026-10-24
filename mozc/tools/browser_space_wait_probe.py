"""EdgeでSpace後の変換表示が届くまでの時間を観測する（2026-10-22、Space→Enterがひらがなで確定する調査）。

browser_duplicate_probe.pyと同じ一時プロファイルのEdgeとBrowserDuplicateDriverを使う。合否は付けず、
Space後の各待ち時間でのcompositionupdate（表示の文字列と時刻）とEnter後の確定を記録する。
利用者本人が実PCでの検証を許可した場合だけ実行する（KOTORI_LOCAL_ACCEPTANCE=owner-approved）。
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--install", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get("KOTORI_LOCAL_ACCEPTANCE") != "owner-approved":
        raise RuntimeError("利用者の許可がある実PCでだけ実行できます")
    from playwright.sync_api import sync_playwright

    args.out.mkdir(parents=True, exist_ok=True)
    marker = "AsteriaDuplicateProbe-" + uuid.uuid4().hex
    report = dict(trials=[])

    def drive(mode, delay=0):
        result = subprocess.run([str(args.driver.resolve()), marker, mode, str(delay), str(args.install.resolve())],
                                capture_output=True, text=True, encoding="utf-8", timeout=20)
        if result.returncode:
            raise RuntimeError("Native Edge driver: " + result.stderr)

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(channel="msedge", headless=False)
        page = browser.new_page(viewport=dict(width=1000, height=700))
        page.set_content("""<!doctype html><meta charset=utf-8>
<style>textarea{display:block;width:800px;height:100px;font:24px sans-serif;margin:20px}</style>
<textarea id=plain></textarea>
<script>
window.events=[];
const target=document.getElementById('plain');
for(const name of ['compositionstart','compositionupdate','compositionend'])
 target.addEventListener(name,e=>events.push({event:e.type,data:e.data,time:performance.now()}));
</script>""")
        page.evaluate("title=>document.title=title", marker)
        page.bring_to_front()
        page.locator("#plain").focus()
        try:
            drive("activate")
            for wait_ms in (2000, 2000, 600):
                page.locator("#plain").focus()
                drive("escape")
                page.evaluate("()=>{const e=document.getElementById('plain');e.value='';window.events=[];e.focus()}")
                drive("type")
                page.wait_for_function("()=>events.some(e=>e.event==='compositionupdate')")
                space_at = page.evaluate("()=>performance.now()")
                drive("space")
                time.sleep(wait_ms / 1000)
                before_enter = page.evaluate("()=>events.slice()")
                drive("enter")
                time.sleep(0.9)
                events = page.evaluate("()=>events.slice()")
                updates = [dict(ms=round(e["time"] - space_at, 1), data=e["data"])
                           for e in before_enter if e["event"] == "compositionupdate" and e["time"] >= space_at]
                report["trials"].append(dict(
                    wait_ms=wait_ms, updates_after_space=updates,
                    shown_before_enter=updates[-1]["data"] if updates else None,
                    committed=[e["data"] for e in events if e["event"] == "compositionend"],
                    value=page.evaluate("()=>document.getElementById('plain').value")))
        finally:
            (args.out / "space-wait.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
            browser.close()


if __name__ == "__main__":
    main()
