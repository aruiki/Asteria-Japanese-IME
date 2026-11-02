"""入力中の候補の窓が、打ってから何 ms で出るか、何件出るか、1 番目が Space の結果と同じかを測る
(2026-11-03、docs/adr/0081)。窓に出た候補は shown に残す。
使い方: python mozc/tools/window_profile.py <session_handler_main> <models> <セット.json> <出力.json> [設定 名前=値 ...]"""
import json, os, queue, re, subprocess, sys, tempfile, threading, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji
exe, models, data, out = Path(sys.argv[1]).resolve(), sys.argv[2], sys.argv[3], sys.argv[4]
cfg = sys.argv[5:]
items = json.load(open(data, encoding="utf-8"))
env = dict(os.environ, KOTORI_MODEL_DIR=models, KOTORI_RUNTIME_DIR=models)
lines = queue.Queue(); rows = []
def cands(text):
    if "candidate_window {" not in text: return []
    body = text.split("candidate_window {", 1)[1]
    body = re.split(r"\n\w+ \{", body, maxsplit=1)[0]
    return re.findall(r'^\s+value: "([^"\n]*)"', body, re.M)
def preedit(text):
    if "preedit {" not in text: return ""
    body = re.split(r"\n\w+ \{", text.split("preedit {", 1)[1], maxsplit=1)[0]
    return "".join(re.findall(r'^\s+value: "([^"\n]*)"', body, re.M))
def delay(text):
    m = re.search(r"delay_millisec: (\d+)", text); return int(m.group(1)) if m else None
with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as prof:
    p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + prof + "/s"], cwd=str(exe) + ".runfiles/_main",
                         env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    threading.Thread(target=lambda: [lines.put(l.decode("utf-8", "replace").rstrip()) for l in p.stdout] + [lines.put(None)], daemon=True).start()
    def send(c):
        p.stdin.write((c + "\nSHOW_OUTPUT\n").encode()); p.stdin.flush(); o = []; tail = False
        while True:
            l = lines.get(timeout=60)
            if l is None: raise SystemExit("crash")
            o.append(l)
            if tail and l.strip() == "}": return "\n".join(o)
            tail |= l.startswith("removed_candidate_words_for_debug {")
    for c in cfg: k, v = c.split("=", 1); send("SET_CONFIG\t" + k + "\t" + v)
    send("SEND_KEY\tON"); send("SEND_KEYS\tai"); time.sleep(15); send("SEND_KEY\tESC")
    for it in items:
        romaji = it.get("romaji") or to_romaji(it["input"])
        if romaji is None: continue
        text = ""
        for ch in romaji:
            text = send("SEND_KEY\t" + ch); time.sleep(0.04)
        typed = time.monotonic(); first = None; shown = []
        # 製品の TIP と同じく、サーバーが返す間隔で取り直しを送る(最大 3 秒)。
        while delay(text) is not None and time.monotonic() - typed < 3:
            time.sleep(delay(text) / 1000); text = send("KOTORI_REFRESH")
            if first is None and cands(text):
                first = (time.monotonic() - typed) * 1000; shown = cands(text)
        if first is None and cands(text): first, shown = 0.0, cands(text)
        text = send("SEND_KEY\tSpace")
        while delay(text) is not None:
            time.sleep(delay(text) / 1000); text = send("KOTORI_REFRESH")
        space = preedit(text)
        rows.append(dict(index=it["index"], input=it["input"], window_ms=first, n=len(shown), top=shown[0] if shown else None,
                         shown=shown[:9], space=space, top_is_space=bool(shown) and shown[0] == space,
                         space_ok=space in it["expected_output"],
                         in_window_ok=any(s in it["expected_output"] for s in shown)))
        send("SEND_KEY\tESC"); send("SEND_KEY\tESC")
    p.stdin.close(); p.wait(60)
json.dump(rows, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
w = [r for r in rows if r["window_ms"] is not None]
ms = sorted(r["window_ms"] for r in w)
med = ms[len(ms)//2] if ms else None
print(f"問 {len(rows)} 窓が出た {len(w)} 出るまでの中央値 {med and round(med)} ms p90 {ms and round(ms[int(len(ms)*0.9)-1])} ms "
      f"件数の平均 {w and round(sum(r['n'] for r in w)/len(w),1)} 1番目=Spaceの結果 {sum(r['top_is_space'] for r in w)} "
      f"Spaceが正解 {sum(r['space_ok'] for r in rows)} 窓の中に正解 {sum(r['in_window_ok'] for r in w)} 窓にだけ正解(Spaceは不正解) {sum(r['in_window_ok'] and not r['space_ok'] for r in w)}")
