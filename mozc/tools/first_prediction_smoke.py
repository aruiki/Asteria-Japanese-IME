"""実辞書/通常設定で、入力中の続き予測とTabの一致を記録する。
実行中のWindows IMEには接続しない。モデルの生成条件を短縮しない。
開発用予測セットを使用し、heldoutをしきい値調整に使わない。
"""
import argparse
import json
import os
from pathlib import Path
import re
import tempfile
import time
from first_display_compare import Session, romanize, sha256


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", type=Path, default=Path("eval/sets/kotori-predict.json"))
    ap.add_argument("--romanji-table", type=Path, required=True)
    ap.add_argument("--install", type=Path, default=Path((r"C:\Program Files (x86)\Asteria" if os.path.isdir(r"C:\Program Files (x86)\Asteria") else r"C:\Program Files (x86)\Kotori")))
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--settle", type=float, default=2.4)
    ap.add_argument("--warm", type=float, default=12)
    args = ap.parse_args()
    args.exe = args.exe.resolve()
    items = json.loads(args.data.read_text(encoding="utf-8"))
    assert items
    env = {k: v for k, v in os.environ.items() if not k.startswith("KOTORI_")}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install), KOTORI_LM_DEBUG="1")
    rows = []
    initial_hash = sha256(args.exe)
    with tempfile.TemporaryDirectory(prefix="kotori-first-prediction-") as tmp:
        profile = Path(tmp)
        mapping = profile / "context.tsv"
        mapping.write_text("".join(x["typed"] + "\t" + x["context"] + "\n" for x in items if x["context"]), encoding="utf-8")
        env["KOTORI_LM_CONTEXT_MAP"] = str(mapping)
        session = Session(args.exe, env, profile, args.out.with_suffix(".stderr.txt"))
        failed = True
        try:
            session.send("SEND_KEY\tON\nSEND_KEYS\tai")
            time.sleep(args.warm)
            for index, item in enumerate(items):
                session.send("SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT")
                key_ms = []
                for char in romanize(item["typed"], args.romanji_table):
                    event = session.send("SEND_KEYS\t" + char)
                    key_ms.append(event["response_ms"])
                    time.sleep(.01)
                if event["preedit_key"] != item["typed"]:
                    raise AssertionError("読みが一致しません: " + repr(event["preedit_key"]))
                events = [event]
                deadline = time.monotonic() + args.settle
                while time.monotonic() < deadline:
                    callback = re.search(r"delay_millisec: (\d+)", events[-1]["output"])
                    if not callback:
                        break
                    time.sleep(min(int(callback.group(1))/1000, max(0, deadline-time.monotonic())))
                    events.append(session.send("KOTORI_REFRESH"))
                visible = [x for x in events if x["first_candidate"]]
                first = visible[0]["first_candidate"] if visible else None
                final = visible[-1]["first_candidate"] if visible else None
                tab = session.send("SEND_KEY\tTab")
                row = dict(item, first=first, settled=final, tab=tab["first_candidate"],
                           key_max_ms=max(key_ms), events=events, tab_event=tab,
                           continuation=bool(final and len(final)>len(item["conv"]) and final.startswith(item["conv"])),
                           reference_prefix=bool(final and item["truth"].startswith(final)))
                rows.append(row)
                print(f"{index+1}/{len(items)} {item['typed']}: {final!r} / Tab={tab['first_candidate']!r}", flush=True)
            status = profile / "session" / "kotori_ai_status.txt"
            model_status = status.read_text(encoding="utf-8") if status.exists() else None
            failed = False
        finally:
            session.close(failed)
    if sha256(args.exe) != initial_hash:
        raise RuntimeError("測定中に実行ファイルが変更されました")
    result = dict(scope="CLI session; not installed Windows TSF", exe=str(args.exe), exe_sha256=initial_hash,
                  data_sha256=sha256(args.data), model_status=model_status, items=len(rows),
                  continuations=sum(r["continuation"] for r in rows),
                  initial_tab_mismatches=sum(r["first"] is not None and r["first"] != r["tab"] for r in rows),
                  settled_tab_mismatches=sum(r["settled"] is not None and r["settled"] != r["tab"] for r in rows), rows=rows)
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: result[k] for k in ["items", "continuations", "initial_tab_mismatches", "settled_tab_mismatches"]}), flush=True)
    # 続きの計算中は評価済み本文を先に表示できる。初回との差も数値で残す。
    # 手が止まった後の最終表示とTabが異なる場合は失敗。
    if result["settled_tab_mismatches"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
