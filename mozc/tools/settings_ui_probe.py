"""本物の設定画面(WebView2)で、追加の AI の受け取り・品質の選択・削除を確かめる(カード 90、2026-11-04)。

設定画面を、WebView2 のデバッグ用の窓口つきで開き(`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`)、playwright でつないで
利用者と同じようにボタンを押す(確認の窓も押す)。受け取りは利用者のプロファイルの models に置かれ、本物のネットワークで
Hugging Face から受け取る。設定も書き換えるので、利用者が実 PC での確認を許可したときだけ使う。

  # 窓口つきで設定画面を開く(PowerShell)
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9333"
  Start-Process "C:\\Program Files (x86)\\Asteria\\mozc_tool.exe" --mode=config_dialog
  # 調べる・操作する(playwright==1.58.0 を入れた Python)
  python mozc/tools/settings_ui_probe.py models
  python mozc/tools/settings_ui_probe.py quality 2          # 0 Eco・1 Standard・2 High・3 Unreal
  python mozc/tools/settings_ui_probe.py download High
  python mozc/tools/settings_ui_probe.py delete High
"""
import argparse
import time

PORT = 9333


def connect(pw):
    browser = pw.chromium.connect_over_cdp(f"http://127.0.0.1:{PORT}")
    pages = [p for c in browser.contexts for p in c.pages if p.url and not p.url.startswith("devtools")]
    if not pages:
        raise RuntimeError("設定画面が見つからない(窓口つきで開いたか確かめる)")
    return browser, pages[0]


def row_text(locator):
    return locator.inner_text().replace("\n", " / ")


def model_row(page, tier):
    page.locator("#search").fill("ダウンロード")
    time.sleep(1)
    return page.locator(".model").filter(has=page.locator(".tag", has_text=tier))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["models", "quality", "download", "delete"])
    ap.add_argument("value", nargs="?")
    ap.add_argument("--timeout", type=float, default=3600)
    args = ap.parse_args()
    from playwright.sync_api import sync_playwright

    with sync_playwright() as pw:
        browser, page = connect(pw)
        if args.command == "models":
            page.locator("#search").fill("ダウンロード")
            time.sleep(1)
            for m in page.locator(".model").all():
                print(row_text(m))
        elif args.command == "quality":
            page.locator("#search").fill("品質")
            time.sleep(1)
            page.locator(f'button[role=radio][data-value="{args.value}"]').first.click()
            time.sleep(2)
            checked = [e.get_attribute("aria-checked") for e in page.locator("button[role=radio][data-value]").all()[:4]]
            print("選ばれている:", checked.index("true") if "true" in checked else None)
        elif args.command == "download":
            row = model_row(page, args.value)
            row.locator("button.btn.primary").click()
            page.locator("#confirm-ok").click()
            start = time.time()
            while "入っています" not in row.inner_text():
                if time.time() - start > args.timeout:
                    raise RuntimeError("時間内に終わらない: " + row_text(row))
                time.sleep(1)
            print("受け取り完了", round(time.time() - start), "秒:", row_text(row))
        else:
            row = model_row(page, args.value)
            row.locator("button.btn.danger").click()
            page.locator("#confirm-ok").click()
            time.sleep(3)
            print("削除後:", row_text(row))
            print("通知:", page.evaluate("() => [...document.querySelectorAll('.toast')].map(e => e.textContent)"))
        browser.close()


if __name__ == "__main__":
    main()
