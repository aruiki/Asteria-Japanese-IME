"""打ってすぐ Space を押したときの応答時間を測る(docs/adr/0024)。ノート PC で「10 文字打って Space で
固まる」問題の再現と確認に使う。起動直後(モデルの読み込み中)から、日常の文の読みを 1 文字ずつ
間隔の秒おきに入力中の候補として送り、打ち終わったら --pause 秒後に変換する。

使い方: python mozc/tools/space_latency.py [--interval 秒] [--n 文の数] [環境変数=値 ...]
  例(GPU を使わない。ノート PC の再現): python mozc/tools/space_latency.py KOTORI_LM_DEVICE=cpu
  KOTORI_CONVERTER_MAIN、KOTORI_INSTALL_DIR は typing_test.py と同じ。
"""
import argparse
import json
import os
import subprocess
import threading
import time


def kata_to_hira(s: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--interval", type=float, default=0.15)
    ap.add_argument("--n", type=int, default=15)
    ap.add_argument("--pause", type=float, default=0.0, help="打ち終わってから Space までの秒")
    ap.add_argument("--data", default="eval/sets/kotori-daily.json")
    ap.add_argument("--out", help="中央値・p95・項目ごとの時間をJSONで保存する")
    ap.add_argument("--warm", type=int, default=0, help="最初のN文を慣らしとして集計から外す(モデル読込後の測定)")
    ap.add_argument("env", nargs="*")
    args = ap.parse_args()
    exe = os.environ.get("KOTORI_CONVERTER_MAIN",
                         r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
    inst = os.environ.get("KOTORI_INSTALL_DIR", (r"C:\Program Files (x86)\Asteria" if os.path.isdir(r"C:\Program Files (x86)\Asteria") else r"C:\Program Files (x86)\Kotori"))
    env = dict(os.environ, KOTORI_RUNTIME_DIR=inst, KOTORI_MODEL_DIR=inst)
    env.update(dict(a.split("=", 1) for a in args.env))
    warmup = json.load(open("eval/sets/kotori-daily.json", encoding="utf-8"))[: args.warm]
    items = json.load(open(args.data, encoding="utf-8"))[: args.n]
    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, cwd=exe + ".runfiles/_main", env=env)
    out = []
    threading.Thread(target=lambda: [out.append(x) for x in p.stdout], daemon=True).start()

    def send(cmd: str):
        start = len(out)
        t0 = time.time()
        p.stdin.write(f"{cmd}\nreset\nkotori_sep\n".encode("utf-8"))
        p.stdin.flush()
        while time.time() - t0 < 60:
            if any(x.startswith(b"ExecCommand() return false") for x in out[start:]):
                break
            time.sleep(0.001)
        return (time.time() - t0) * 1000, out[start:]

    t_start = time.time()
    space, keys, ok = [], [], 0
    for index, it in enumerate(warmup + items):
        r = kata_to_hira(it["input"])
        for n in range(1, len(r) + 1):
            key_ms = send(f"suggest {r[:n]}")[0]
            if index >= len(warmup):
                keys.append(key_ms)
            time.sleep(args.interval)
        time.sleep(args.pause)
        ms, lines = send(f"start {r}")
        if index < len(warmup):
            print(f"{time.time() - t_start:5.1f}s  慣らし {ms:5.0f} ms  {r[:14]}")
            continue
        space.append(ms)
        top = ""
        for line in lines:
            s = line.decode("utf-8", "replace")
            if s.startswith("  0/"):
                top += s.split(" ", 3)[-1].strip()
        ok += top in it["expected_output"]
        print(f"{time.time() - t_start:5.1f}s  Space {ms:5.0f} ms  {r[:14]:<14} {top[:24]}")
    p.stdin.close()
    rows = list(zip([kata_to_hira(it["input"]) for it in items], space))
    space.sort()
    keys.sort()
    # p95は「遅い側から5%」の位置(最も近い順位)。GOALの速度条件と同じ定義。
    p95 = space[max(0, -(-len(space) * 95 // 100) - 1)]
    print(f"Space: 中央値 {space[len(space) // 2]:.0f} ms、p95 {p95:.0f} ms、最大 {space[-1]:.0f} ms / "
          f"入力中の候補: 最大 {keys[-1]:.0f} ms / 正解 {ok}/{len(items)}")
    if args.out:
        json.dump(dict(data=args.data, interval=args.interval, pause=args.pause, env=args.env,
                       converter=exe, median_ms=space[len(space) // 2], p95_ms=p95, max_ms=space[-1],
                       key_max_ms=keys[-1], correct=ok, n=len(items),
                       items=[dict(reading=r, space_ms=round(ms, 1)) for r, ms in rows]),
                  open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
