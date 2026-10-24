"""報告された変換例を隔離セッションで再実行する。実際のWindows TSFとは区別する。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import time
from first_display_compare import Session, romanize


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--cases", type=Path, required=True)
    ap.add_argument("--romanji-table", type=Path, required=True)
    ap.add_argument("--install", type=Path, default=Path((r"C:\Program Files (x86)\Asteria" if os.path.isdir(r"C:\Program Files (x86)\Asteria") else r"C:\Program Files (x86)\Kotori")))
    ap.add_argument("--warm", type=float, default=12)
    ap.add_argument("--immediate-space", action="store_true")
    ap.add_argument("--only", help="ケースIDをカンマで指定")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    exe = args.exe.resolve()
    identity = hashlib.sha256(exe.read_bytes()).hexdigest()
    cases = json.loads(args.cases.read_text(encoding="utf-8"))
    if args.only:
        wanted = set(args.only.split(","))
        cases = [case for case in cases if case["id"] in wanted]
        if {case["id"] for case in cases} != wanted:
            ap.error("未知のケースID")
    env = {k: v for k, v in os.environ.items() if not k.startswith("KOTORI_")}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install),
               KOTORI_LM_DEBUG="1", KOTORI_LM_TIME="1")
    rows, failures = [], []
    for case in cases:
        with tempfile.TemporaryDirectory(prefix="kotori-regression-") as tmp:
            session = Session(exe, env, Path(tmp),
                              args.out.with_name(args.out.stem + "." + case["id"] + ".stderr.txt"))
            failed = True
            try:
                session.send("SET_CONFIG\tkotori_lm_quality\tKOTORI_LM_STANDARD")
                session.send("SET_CONFIG\tkotori_lm_model\tKOTORI_LM_MODEL_LLM")
                session.send("SEND_KEY\tON\nSEND_KEYS\tai")
                time.sleep(args.warm)
                session.send("SEND_KEY\tESC\nRESET_CONTEXT")
                keys = romanize(case["reading"], args.romanji_table)
                for i, key in enumerate(keys):
                    command = "SEND_KEYS\t" + key
                    if i == 0 and case.get("preceding"):
                        command = "SEND_KEY_WITH_OPTION\t" + key + "\tcontext.preceding_text=" + json.dumps(case["preceding"], ensure_ascii=False)
                    last = session.send(command)
                    time.sleep(.01)
                if last["preedit_key"] != case["reading"]:
                    raise AssertionError("読みの不一致: " + last["preedit_key"])
                events = [last]
                if not args.immediate_space:
                    for _ in range(5):
                        callback = re.search(r"delay_millisec: (\d+)", events[-1]["output"])
                        if not callback:
                            break
                        time.sleep(int(callback.group(1)) / 1000)
                        events.append(session.send("KOTORI_REFRESH"))
                space1 = session.send("SEND_KEY\tSpace")
                space2 = session.send("SEND_KEY\tSpace")
                events.extend((space1, space2))
                errors = []
                first = next((event["first_candidate"] for event in events[:-2] if event["first_candidate"]), None)
                if not args.immediate_space and (first is None or re.fullmatch(case["expected"], first) is None):
                    errors.append("初回候補が不正: " + repr(first))
                for label, event in (("Space1", space1), ("Space2", space2)):
                    expected = case.get("expected_space2", case["expected"]) if label == "Space2" else case["expected"]
                    if re.fullmatch(expected, event["preedit_value"]) is None:
                        errors.append(label + "が不正: " + event["preedit_value"])
                if not args.immediate_space and first != space1["preedit_value"]:
                    errors.append("初回候補とSpace1の不一致")
                rows.append(dict(case, events=events, errors=errors))
                failures.extend(case["id"] + ": " + error for error in errors)
                print(json.dumps({"id": case["id"], "first": first,
                                  "space1": space1["preedit_value"],
                                  "space2": space2["preedit_value"], "errors": errors}, ensure_ascii=False), flush=True)
                failed = False
            finally:
                session.close(failed)
    if hashlib.sha256(exe.read_bytes()).hexdigest() != identity:
        raise RuntimeError("試験中に実行ファイルが変化")
    result = {"exe": str(exe), "exe_sha256": identity,
              "cases_sha256": hashlib.sha256(args.cases.read_bytes()).hexdigest(),
              "warm_seconds": args.warm, "immediate_space": args.immediate_space,
              "native_tsf": False, "rows": rows, "failures": failures}
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    if failures:
        raise SystemExit("変換の回帰失敗: " + "; ".join(failures))


if __name__ == "__main__":
    main()
