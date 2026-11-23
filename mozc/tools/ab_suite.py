#!/usr/bin/env python3
"""変換器の前後比較を 1 コマンドで行う(2026-11-23)。

変種(バイナリのフォルダと環境変数)× 固定のセット一覧を、GPU が空いていることを確かめながら 1 つずつ流し、
正解数の表・1 問ずつの入れ替わり・AI の時間を Markdown にまとめる(PR と verification にそのまま貼る)。

  python mozc/tools/ab_suite.py --out D:/dev/ab/phase2 \\
      --variant base D:/dev/bins/v521 \\
      --variant new  D:/dev/bins/p2 KOTORI_ZENZ_EXTRA_MODEL=... \\
      [--suite fast|wide] [--session] [--sets 名前 ...]

- 変種のフォルダは setup_mozc_tree.ps1 -Snapshot の形(converter\\converter_main.exe、session\\session_handler_main.exe)。
  converter_main.exe そのものを渡してもよい。環境変数 MODELS=<フォルダ> はモデルと llama.cpp の場所(既定はインストール先)。
- 既定は eval_baseline.py(converter_main、KOTORI_LM_PRELOAD=0、文脈あり)。--session は session_eval.py
  (製品と同じ session の経路。先回りの変換と入力中の取り直しを通る)。--timing immediate|gap150 で Space の押し方を変える。
- GPU: 同時に 1 つだけ流す(--lock のファイルを排他で開く)。各セットの前に nvidia-smi で使用率を見て、
  --gpu-busy %(既定 30)以上なら空くまで待ち、--gpu-wait 分を過ぎたら止める。AI が動かなかった問の多い結果は「無効?」と書く。
- 1 問ずつの比較は 1 つ目の変種を基準にする。
"""
import argparse
import contextlib
import importlib.util
import json
import os
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SETS = ROOT / "eval" / "sets"
DEFAULT_MODELS = "C:/Program Files (x86)/Asteria"

FAST = [
    "eval/data/ajimee-bench.json",
    "eval/sets/asteria-choice-dev-20261027.json",
    "eval/sets/asteria-passage-dev-20261027.json",
    "eval/sets/kotori-daily.json",
    "eval/sets/asteria-short-dev-20261022.json",
    "eval/sets/kotori-homophone.json",
    "eval/sets/asteria-longcontext-dev-20261123.json",
]
WIDE = FAST + [
    "eval/sets/kotori-nuance.json",
    "eval/sets/kotori-notation.json",
    "eval/sets/kotori-idioms.json",
    "eval/sets/kotori-typo.json",
    "eval/sets/asteria-typo-keys-20261026.json",
    "eval/sets/asteria-regression-golden.json",
    "filler:kotori-daily",
    "filler:asteria-short-dev-20261022",
    "filler:kotori-notation",
    "filler:kotori-homophone",
]
TIME_RE = re.compile(r"\[time\].*?load=(\d+)ms .*?total=(\d+)ms")


def log(msg):
    print(msg, file=sys.stderr, flush=True)


@contextlib.contextmanager
def gpu_lock(path):
    """GPU を使う評価を同時に 1 つにする。プロセスが終われば OS が鍵を外す。"""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    f = open(path, "a+")
    waited = False
    while True:
        try:
            if os.name == "nt":
                import msvcrt
                f.seek(0)
                msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            break
        except OSError:
            if not waited:
                log(f"ほかの評価が GPU を使っている({path})。終わるのを待つ")
                waited = True
            time.sleep(10)
    try:
        yield
    finally:
        f.close()


def gpu_utilization():
    try:
        out = subprocess.run(["nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"],
                             capture_output=True, text=True, timeout=10).stdout
        return max(int(x) for x in out.split())
    except (OSError, ValueError, subprocess.SubprocessError):
        return None


def wait_gpu_idle(busy, wait_min):
    """5 秒の平均使用率が busy % 未満になるまで待つ。nvidia-smi がなければ確かめない。"""
    deadline = time.monotonic() + wait_min * 60
    while True:
        samples = [gpu_utilization() for _ in range(5) if not time.sleep(1)]
        samples = [s for s in samples if s is not None]
        if not samples:
            return None
        avg = sum(samples) / len(samples)
        if avg < busy:
            return avg
        if time.monotonic() > deadline:
            raise SystemExit(f"GPU が {wait_min} 分たっても空かない(使用率 {avg:.0f}%)。ほかのアプリを止めてやり直す")
        log(f"GPU の使用率 {avg:.0f}% — 空くまで待つ")
        time.sleep(25)


def filler_set(name, out_dir):
    """既存セットの前に、話題に中立な長い文を足したもの(長い前の文の副作用を見る)。"""
    spec = importlib.util.spec_from_file_location("make_longcontext", SETS / "make_longcontext.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    items = json.loads((SETS / f"{name}.json").read_text(encoding="utf-8"))
    for i, it in enumerate(items):
        it["context_text"] = mod.FILLERS[(i + 2) % 6] + mod.FILLERS[(i + 3) % 6] + (it.get("context_text") or "")
    path = out_dir / "sets" / f"{name}+filler.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(items, ensure_ascii=False), encoding="utf-8")
    return path


def resolve_exe(folder, session):
    p = Path(folder)
    if p.is_file():
        return p
    sub = p / ("session/session_handler_main.exe" if session else "converter/converter_main.exe")
    if not sub.is_file():
        raise SystemExit(f"見つからない: {sub}")
    return sub


def run_one(args, variant, set_path, label, out_dir):
    name, folder, env_items = variant
    env = dict(os.environ, PYTHONIOENCODING="utf-8", KOTORI_LM_TIME="1")
    if not args.session:
        # converter_main は要求の中でモデルを読む(評価専用)。session の経路は製品と同じく起動時に裏で読む
        # (付けると AI の状態が立たず、session_eval.py が止まる)。
        env["KOTORI_LM_PRELOAD"] = "0"
    else:
        env.pop("KOTORI_LM_PRELOAD", None)
    extra = dict(e.split("=", 1) for e in env_items)
    models = extra.pop("MODELS", DEFAULT_MODELS)
    env.update(KOTORI_MODEL_DIR=models, KOTORI_RUNTIME_DIR=models)
    env.update(extra)
    exe = resolve_exe(folder, args.session)
    result = out_dir / f"{name}--{label}.json"
    stderr = out_dir / f"{name}--{label}.stderr.txt"
    if args.session:
        cmd = [sys.executable, "-X", "utf8", str(ROOT / "mozc/tools/session_eval.py"), str(exe),
               "--data", str(set_path), "--out", str(result), "--context", "--models", models]
        if args.timing == "immediate":
            cmd.append("--immediate-space")
        elif args.timing == "gap150":
            cmd += ["--space-gap", "150"]
        cmd += [f"{k}={v}" for k, v in extra.items()]
    else:
        cmd = [sys.executable, "-X", "utf8", str(ROOT / "mozc/eval_baseline.py"), str(exe),
               "--data", str(set_path), "--context", "--out", str(result), "--stderr", str(stderr)]
        # --profile は付けない(使い捨てのプロファイル。前の回の学習を持ち越さない)。
    started = time.monotonic()
    p = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
    (out_dir / f"{name}--{label}.log").write_text(p.stdout + p.stderr, encoding="utf-8")
    if p.returncode != 0 or not result.is_file():
        log(f"  {name} {label}: 失敗(終了コード {p.returncode}、{out_dir / (name + '--' + label + '.log')})")
        return None
    rows = json.loads(result.read_text(encoding="utf-8"))
    times, ai_lines = [], 0
    if args.session:
        times = [r["space_to_candidate_ms"] for r in rows if r.get("space_to_candidate_ms") is not None]
        ai_lines = len(rows)
    elif stderr.is_file():
        for line in stderr.read_text(encoding="utf-8", errors="replace").splitlines():
            m = TIME_RE.search(line)
            if m:
                ai_lines += 1
                if int(m.group(1)) < 50:  # モデルの読み込みを含む回は除く
                    times.append(int(m.group(2)) - int(m.group(1)))
    ok = sum(1 for r in rows if r.get("ok"))
    log(f"  {name} {label}: {ok}/{len(rows)}({time.monotonic() - started:.0f} 秒)")
    return {"rows": rows, "ok": ok, "n": len(rows), "times": times,
            "ai_coverage": ai_lines / max(1, len(rows))}


def pct(values, q):
    if not values:
        return None
    s = sorted(values)
    return s[min(len(s) - 1, int(len(s) * q))]


def report(args, variants, results, labels):
    names = [v[0] for v in variants]
    lines = [f"# 前後比較({'session の経路' + ('・' + args.timing if args.timing else '') if args.session else 'converter_main'})", ""]
    for v in variants:
        lines.append(f"- {v[0]}: `{v[1]}` {' '.join(v[2])}".rstrip())
    lines += ["", "| セット | " + " | ".join(names) + " |", "| --- |" + " --- |" * len(names)]
    total = {n: [0, 0] for n in names}
    for label in labels:
        cells = []
        for n in names:
            r = results.get((n, label))
            if r is None:
                cells.append("失敗")
                continue
            total[n][0] += r["ok"]
            total[n][1] += r["n"]
            mark = "" if r["ai_coverage"] >= 0.95 else f" 無効?(AI {r['ai_coverage']:.0%})"
            cells.append(f"{r['ok']}/{r['n']}{mark}")
        lines.append(f"| {label} | " + " | ".join(cells) + " |")
    lines.append("| 計 | " + " | ".join(f"{a}/{b}" for a, b in total.values()) + " |")
    lines += ["", "AI の時間(ms、" + ("Space→候補" if args.session else "読み込みを除く計算") + ")", "",
              "| 変種 | 中央値 | p95 | 回数 |", "| --- | --- | --- | --- |"]
    for n in names:
        t = [x for label in labels if results.get((n, label)) for x in results[(n, label)]["times"]]
        if t:
            lines.append(f"| {n} | {statistics.median(t):.0f} | {pct(t, 0.95):.0f} | {len(t)} |")
    base = names[0]
    for n in names[1:]:
        flips = []
        for label in labels:
            a, b = results.get((base, label)), results.get((n, label))
            if not a or not b:
                continue
            for x, y in zip(a["rows"], b["rows"]):
                if bool(x.get("ok")) != bool(y.get("ok")):
                    tag = "直った" if y.get("ok") else "外れた"
                    exp = (x.get("expected") or x.get("expected_output") or [""])[0]
                    flips.append(f"- {tag} [{label}] {exp} | {base}: {x.get('top1')} → {n}: {y.get('top1')}")
        fixed = sum(1 for f in flips if f.startswith("- 直った"))
        lines += ["", f"## {base} → {n}: 直った {fixed}、外れた {len(flips) - fixed}", ""]
        lines += flips[: args.max_flips] or ["(入れ替わりなし)"]
        if len(flips) > args.max_flips:
            lines.append(f"- ほか {len(flips) - args.max_flips} 件")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--variant", nargs="+", action="append", required=True, metavar=("名前 フォルダ", "KEY=VAL"),
                    help="名前 フォルダ [環境変数=値 ...]。MODELS=<フォルダ> でモデルの場所")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--suite", choices=["fast", "wide"], default="fast")
    ap.add_argument("--sets", nargs="*", help="セットを直接指定(パス、または filler:<名前>)")
    ap.add_argument("--session", action="store_true", help="session_eval.py(製品と同じ経路)で測る")
    ap.add_argument("--timing", choices=["immediate", "gap150"], help="--session の Space の押し方")
    ap.add_argument("--lock", default=str(Path.home() / ".asteria-gpu-eval.lock"))
    # 3D CAD やブラウザを開いているだけで 10〜20% になる。正解数が大きく崩れたのは 49% のとき(AJIMEE 189→165)。
    # 崩れたかどうかは表の「無効?」(AI が動いた問の割合)で見る。
    ap.add_argument("--gpu-busy", type=float, default=30)
    ap.add_argument("--gpu-wait", type=float, default=20)
    ap.add_argument("--max-flips", type=int, default=40)
    args = ap.parse_args()
    variants = []
    for v in args.variant:
        if len(v) < 2:
            raise SystemExit("--variant には 名前 と フォルダ が要る")
        variants.append((v[0], v[1], v[2:]))
    args.out.mkdir(parents=True, exist_ok=True)
    specs = args.sets if args.sets else (FAST if args.suite == "fast" else WIDE)
    sets = []
    for s in specs:
        if s.startswith("filler:"):
            name = s.split(":", 1)[1]
            sets.append((f"{name}+filler", filler_set(name, args.out)))
        else:
            p = (ROOT / s) if not Path(s).is_absolute() else Path(s)
            if not p.is_file():
                raise SystemExit(f"セットがない: {p}(AJIMEE は eval/fetch.sh で取得)")
            sets.append((p.stem, p))
    results = {}
    with gpu_lock(args.lock):
        for label, path in sets:
            # 変種を同じセットの中で続けて流し、時間による環境の揺れを両方に同じように受けさせる。
            for v in variants:
                util = wait_gpu_idle(args.gpu_busy, args.gpu_wait)
                if util is not None and util > 5:
                    log(f"  (GPU の使用率 {util:.0f}%)")
                results[(v[0], label)] = run_one(args, v, path, label, args.out)
    md = report(args, variants, results, [label for label, _ in sets])
    (args.out / "summary.md").write_text(md, encoding="utf-8")
    print(md)


if __name__ == "__main__":
    main()
