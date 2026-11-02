r"""query_of_death.log(IME のサーバーが落ちたときの直前の操作)を session_handler_main に流し直し、
今の版で落ちないかを確かめる(2026-11-02)。ログは %USERPROFILE%\AppData\LocalLow\Asteria にある。
使い方: python mozc/tools/qod_replay.py <session_handler_main> <models> <query_of_death.log> [設定 名前=値 ...]"""
import os, queue, re, subprocess, sys, tempfile, threading, time
from pathlib import Path
exe, models, log = Path(sys.argv[1]).resolve(), sys.argv[2], sys.argv[3]
configs = sys.argv[4:]
t = open(log, encoding="utf-8", errors="replace").read()
steps = []
for b in t.split("goo.gle/debugproto")[1:]:
    ty = re.search(r"^type: (\w+)", b.strip(), re.M)
    if not ty: continue
    if ty.group(1) == "SEND_KEY":
        kc = re.search(r"key_code: (\d+)", b); sk = re.search(r"special_key: (\w+)", b)
        mods = [m.lower() for m in re.findall(r"modifier_keys: (\w+)", b)]
        key = chr(int(kc.group(1))) if kc else sk.group(1).lower()
        steps.append(("key", " ".join(mods + [key])))
    elif "KOTORI_REFRESH_SUGGESTION" in b:
        steps.append(("refresh", None))
env = dict(os.environ, KOTORI_MODEL_DIR=models, KOTORI_RUNTIME_DIR=models)
lines = queue.Queue()
with tempfile.TemporaryDirectory(prefix="qod-", ignore_cleanup_errors=True) as profile:
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
                raise SystemExit(f"CRASH exit={p.wait(30):#x} after {c!r}")
            out.append(l)
            if tail and l.strip() == "}": return "\n".join(out)
            tail |= l.startswith("removed_candidate_words_for_debug {")
    for c in configs:
        n, v = c.split("=", 1); send("SET_CONFIG\t" + n + "\t" + v)
    send("SEND_KEY\tON"); time.sleep(15)
    for kind, arg in steps:
        if kind == "key":
            send("SEND_KEY\t" + arg); time.sleep(0.12)
        else:
            send("KOTORI_REFRESH"); time.sleep(0.03)
    time.sleep(1); send("KOTORI_REFRESH")
    p.stdin.close(); code = p.wait(60)
    print(f"OK steps={len(steps)} exit={code:#x} configs={configs}")
