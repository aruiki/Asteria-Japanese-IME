"""使い捨てWindows CIのEdgeで、実TSFの確定イベントとDOMの文字列を照合する。"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--install", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if (sys.platform != "win32" or os.environ.get("GITHUB_ACTIONS") != "true" or
            os.environ.get("KOTORI_DISPOSABLE_RUNNER") != "github-hosted"):
        raise RuntimeError("使い捨てWindows CI以外では実行できません")
    from playwright.sync_api import sync_playwright

    args.out.mkdir(parents=True, exist_ok=True)
    marker = "AsteriaDuplicateProbe-" + uuid.uuid4().hex
    report = dict(complete=False, scope="Edge textarea/contenteditable; CPU GitHub runner",
                  trials=[], driver_calls=[], source_commit=os.environ.get("GITHUB_SHA"))

    def drive(mode, delay=0):
        result = subprocess.run([str(args.driver.resolve()), marker, mode, str(delay), str(args.install.resolve())],
                                capture_output=True, text=True, encoding="utf-8", timeout=20)
        if result.returncode:
            raise RuntimeError("Native Edge driver: " + result.stderr)
        row = json.loads(result.stdout)
        report["driver_calls"].append(row)

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(channel="msedge", headless=False)
        page = browser.new_page(viewport=dict(width=1000, height=700))
        page.set_content("""<!doctype html><meta charset=utf-8>
<style>textarea,[contenteditable]{display:block;width:800px;height:100px;font:24px sans-serif;margin:20px}</style>
<textarea id=plain></textarea><div id=editable contenteditable></div><input id=other>
<script>
window.events=[];
for(const id of ['plain','editable']) {
 const target=document.getElementById(id);
 for(const name of ['compositionstart','compositionupdate','compositionend','beforeinput','input'])
  target.addEventListener(name,e=>events.push({id,event:e.type,data:e.data,inputType:e.inputType,
    composing:e.isComposing,value:target.value??target.textContent,time:performance.now()}));
}
window.snapshot=id=>({value:document.getElementById(id).value??document.getElementById(id).textContent,
 events:events.filter(e=>e.id===id)});
</script>""")
        page.evaluate("title=>document.title=title", marker)
        page.bring_to_front()
        page.locator("#plain").focus()
        try:
            drive("activate")
            for host in ("plain", "editable"):
                for mode, delay in (("enter", 0), ("space_enter", 0), ("space_enter", 250),
                                    ("down_enter", 100), ("click", 0), ("focus", 0), ("intentional_repeat", 0)):
                    row = dict(host=host, mode=mode, delay_ms=delay, passed=False)
                    report["trials"].append(row)
                    try:
                        page.locator("#" + host).focus()
                        drive("escape")
                        page.evaluate("id=>{let e=document.getElementById(id);if(e.tagName==='TEXTAREA') e.value='';"
                                      "else e.textContent='';window.events=[];e.focus()}", host)
                        drive("type")
                        page.wait_for_function("id=>events.some(e=>e.id===id&&e.event==='compositionupdate')", arg=host)
                        row["before"] = page.evaluate("snapshot", host)
                        if mode in ("click", "down_enter"):
                            drive("space")
                            page.wait_for_function("id=>{let a=events.filter(e=>e.id===id&&e.event==='compositionupdate');"
                                                   "return a.length&&a[a.length-1].data&&a[a.length-1].data!=='ごーるにせっていして'}", arg=host)
                            time.sleep(0.5)
                            drive(mode, delay)
                            if mode == "click":
                                time.sleep(0.18)
                                row["after_click"] = page.evaluate("snapshot", host)
                                if any(event["event"] == "compositionend" for event in row["after_click"]["events"]):
                                    raise RuntimeError("Candidate selection unexpectedly committed composition")
                                drive("enter")
                        elif mode == "focus":
                            page.locator("#other").focus()
                        elif mode == "intentional_repeat":
                            drive("f6_enter")
                            page.wait_for_function("id=>events.filter(e=>e.id===id&&e.event==='compositionend').length===1", arg=host)
                            drive("type")
                            drive("f6_enter")
                        else:
                            drive(mode, delay)
                        count = 2 if mode == "intentional_repeat" else 1
                        page.wait_for_function("({id,n})=>events.filter(e=>e.id===id&&e.event==='compositionend').length>=n",
                                               arg=dict(id=host, n=count), timeout=15000)
                        time.sleep(0.9)
                        snapshot = page.evaluate("snapshot", host)
                        commits = [event["data"] for event in snapshot["events"] if event["event"] == "compositionend"]
                        if len(commits) != count or any(not value for value in commits) or snapshot["value"] != "".join(commits):
                            raise RuntimeError("DOM text or composition-end count differs from intentional commits")
                        if count == 1:
                            value = commits[0]
                            if len(value) % 2 == 0 and value[:len(value)//2] == value[len(value)//2:]:
                                raise RuntimeError("Whole phrase duplicated inside one commit")
                        elif commits[0] != commits[1]:
                            raise RuntimeError("Two intentional equal readings did not remain equal")
                        row["passed"] = True
                    finally:
                        row["after"] = page.evaluate("snapshot", host)
                        page.screenshot(path=str(args.out / (host + "-" + mode + "-" + str(delay) + ".png")))
            expected_tip = str((args.install / "mozc_tip64.dll").resolve()).casefold()
            if not any(expected_tip == path.casefold() for call in report["driver_calls"] for path in call["loaded_tips"]):
                raise RuntimeError("Edge process did not load the installed Asteria TIP")
            if len(report["trials"]) != 14:
                raise RuntimeError("Incomplete browser duplicate suite")
            report["complete"] = True
        except Exception as error:
            report["error"] = str(error)
            raise
        finally:
            (args.out / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
            browser.close()


if __name__ == "__main__":
    main()
