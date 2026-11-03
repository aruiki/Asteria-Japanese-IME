"""セッションの経路での乱打試験(2026-11-03、docs/adr/0081)。
落ちないことと、入力中の窓が要件どおりに出ることを確かめる: 予測の元を両方切ったら窓を出さない、候補数は
suggestions_size 以下、読みが kotori_suggest_min_chars 文字より短いときは出さない(kotori_suggest_completions_only=false
は Mozc と同じ出し方なので窓の検査をしない)。
使い方: python mozc/tools/session_fuzz.py <session_handler_main> <models> <回数> <乱数の種> [設定 名前=値 ...]"""
import os, queue, random, re, subprocess, sys, tempfile, threading, time
from pathlib import Path
exe, models, n, seed = Path(sys.argv[1]).resolve(), sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
configs = sys.argv[5:]
opts = dict(c.split("=", 1) for c in configs)
check = opts.get("kotori_suggest_completions_only", "true") != "false"
sources_off = opts.get("use_history_suggest") == "false" and opts.get("use_dictionary_suggest") == "false"
max_count = int(opts.get("suggestions_size", 3))
min_chars = int(opts.get("kotori_suggest_min_chars", 3))


def violation(text):
    """入力中の窓(category: SUGGESTION)が要件に反していれば理由を返す。"""
    if "candidate_window {" not in text or "category: SUGGESTION" not in text:
        return None
    window = text.split("candidate_window {", 1)[1]
    window = re.split(r"\n\w+ \{", window, maxsplit=1)[0]
    count = len(re.findall(r"^  candidate \{", window, re.M))
    preedit = ""
    if "preedit {" in text:
        body = re.split(r"\n\w+ \{", text.split("preedit {", 1)[1], maxsplit=1)[0]
        preedit = "".join(re.findall(r'^\s+value: "([^"\n]*)"', body, re.M))
    if sources_off:
        return "予測の元を切っているのに窓が出た"
    if count > max_count:
        return f"候補数 {count} > {max_count}"
    # ライブ変換では未確定の文字が変換した表記(初日)なので、読みの文字数とは比べられない。
    if opts.get("kotori_live_conversion") != "true" and len(preedit) < min_chars:
        return f"読み {preedit} が {min_chars} 文字より短いのに窓が出た"
    return None
rnd = random.Random(seed)
syllables = ["a","i","u","e","o","ka","ki","ku","ke","ko","sa","shi","su","se","so","ta","chi","tsu","te","to","na","ni","nu","ne","no",
             "ha","hi","fu","he","ho","ma","mi","mu","me","mo","ya","yu","yo","ra","ri","ru","re","ro","wa","wo","nn","ga","gi","gu",
             "ge","go","za","ji","zu","ze","zo","da","de","do","ba","bi","bu","be","bo","pa","pi","kyo","sho","cha","-","tte","sse",
             "github","linux","x"]
specials = ["backspace", "space", "enter", "escape", "tab", "down", "up", "left", "right", "shift enter", "ctrl enter",
            "f7", "f10", "hankaku", "shift space"]
env = dict(os.environ, KOTORI_MODEL_DIR=models, KOTORI_RUNTIME_DIR=models)
lines = queue.Queue()
violations, counts = [], {"keys": 0, "refresh": 0, "window_after_type": 0}
with tempfile.TemporaryDirectory(prefix="fuzz-", ignore_cleanup_errors=True) as profile:
    p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + str(Path(profile) / "s")],
                         cwd=str(exe) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    threading.Thread(target=lambda: [lines.put(l.decode("utf-8", "replace").rstrip()) for l in p.stdout] + [lines.put(None)], daemon=True).start()
    def send(c):
        p.stdin.write((c + "\nSHOW_OUTPUT\n").encode()); p.stdin.flush()
        out = []; tail = False; deadline = time.monotonic() + 30
        while True:
            l = lines.get(timeout=max(.01, deadline - time.monotonic()))
            if l is None:
                raise SystemExit(f"CRASH exit={p.wait(30):#x} step={counts} last={c!r}")
            out.append(l)
            if tail and l.strip() == "}": return "\n".join(out)
            tail |= l.startswith("removed_candidate_words_for_debug {")
    for c in configs:
        k, v = c.split("=", 1); send("SET_CONFIG\t" + k + "\t" + v)
    send("SEND_KEY\tON"); time.sleep(12)
    history = []
    for i in range(n):
        r = rnd.random()
        if r < 0.62:
            syl = rnd.choice(syllables)
            caps = rnd.random() < 0.05
            for ch in syl[:-1]:
                send("SEND_KEY\t" + ("caps " + ch.upper() if caps and ch.isalpha() else ch)); counts["keys"] += 1
            ch = syl[-1]
            key = "caps " + ch.upper() if caps and ch.isalpha() else ch
            kind = "type"
        elif r < 0.80:
            key, kind = None, "refresh"
        else:
            key, kind = rnd.choice(specials), "special"
        text = send("KOTORI_REFRESH") if kind == "refresh" else send("SEND_KEY\t" + key)
        history.append(key or "R"); history = history[-30:]
        counts["refresh" if kind == "refresh" else "keys"] += 1
        # 打鍵・取り直しの直後に入力中の候補の窓が出ていないこと(変換中の窓は Space などで出る)。
        if kind in ("type", "refresh") and "category: SUGGESTION" in text:
            counts["window_after_type"] += 1
            why = violation(text) if check else None
            if why: violations.append(why + ": " + " ".join(history))
        if rnd.random() < 0.10:
            # 手を止めて、AI の結果が出そろった後の取り直しも試す。
            time.sleep(rnd.choice([0.15, 0.6, 1.2]))
            text = send("KOTORI_REFRESH"); counts["refresh"] += 1; history.append("R")
            if "category: SUGGESTION" in text:
                counts["window_after_type"] += 1
                why = violation(text) if check else None
                if why: violations.append(why + ": " + " ".join(history[-30:]))
    p.stdin.close(); code = p.wait(60)
    for v in violations[:3]: print("VIOLATION:", v)
    print(f"{'OK' if not violations else 'NG'} steps={n} seed={seed} exit={code:#x} violations={len(violations)} "
          f"configs={configs} counts={counts}")
