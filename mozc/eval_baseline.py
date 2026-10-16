#!/usr/bin/env python3
"""Mozc(//converter:converter_main)で AJIMEE-Bench などの Acc@1 を測る(docs/adr/0012 段階 3)。

使い方: python3 mozc/eval_baseline.py <converter_main> [--data 評価セット.json] [--out 結果.json] [--context]
  <converter_main> は bazel-bin/converter/converter_main(Windows は .exe)。データは runfiles の中にあるので、
  runfiles の _main を作業ディレクトリにして起動する。
通常は1回の起動で全問を解く。同じ読みで前文が異なる問題と --case-traces は別起動へ分ける。
第1候補は各文節の第1候補をつないだ文。観測候補と正解包含も結果へ保存する。
LM リランク(docs/adr/0013、0016)は環境変数 KOTORI_ZENZ_MODEL などで設定する(rewriter/lm_rewriter.h)。
--context は問題の前の文(context_text)を AI に渡す(アプリから直前の文を受け取った場合に当たる)。
前の文は「読み<TAB>前の文」のファイルを KOTORI_LM_CONTEXT_MAP で渡す。

converter_main が異常終了した、出力が問題の数に足りない、時間切れのときは失敗(終了コード 1)にする。
--min-acc を付けると、Acc@1 がそれ未満のときも失敗(終了コード 2)にする(回帰の検出用)。
--out を付けると、結果の隣に実行条件(<out>.manifest.json: データ・変換器・モデルの SHA-256、
KOTORI_ の環境変数、コミット、GPU)も残す(docs/IMPROVEMENT_PROPOSALS.md の案 11)。
"""
import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from tools.conversion_quality import score_case, scoring_trace, validate_items


def kata_to_hira(s: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


SEG = re.compile(r"^-{10} Segment \d+/\d+ \[.*\] -{10}$")
CAND = re.compile(r"^\s+0/\d+ (.*)$")
SEP = "kotori_eval_separator"


def first_candidates(lines) -> str:
    parts, want = [], False
    for line in lines:
        if SEG.match(line):
            want = True
        elif want:
            m = CAND.match(line)
            if m:
                parts.append(m.group(1))
                want = False
    return "".join(parts)


def wilson(hit: int, n: int, z: float = 1.96) -> tuple:
    """Acc@1 の 95% 信頼区間(Wilson)。問題数が少ないと幅が広いので、差がこの幅より小さければ偶然と区別できない。"""
    if n == 0:
        return 0.0, 0.0
    p = hit / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    m = z * ((p * (1 - p) + z * z / (4 * n)) / n) ** 0.5
    return 100 * (c - m) / d, 100 * (c + m) / d


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def manifest(args, exe: Path, env: dict, acc: float, n: int) -> dict:
    """結果を再現するための実行条件。"""
    def run(cmd):
        try:
            return subprocess.run(cmd, capture_output=True, text=True, timeout=30,
                                  cwd=Path(__file__).resolve().parent.parent).stdout.strip()
        except (OSError, subprocess.SubprocessError):
            return ""
    models = {}
    model_dir = env.get("KOTORI_MODEL_DIR")
    paths = [env.get("KOTORI_ZENZ_MODEL"), env.get("KOTORI_LLM_MODEL")]
    if model_dir and Path(model_dir).is_dir():
        paths += [str(p) for p in sorted(Path(model_dir).glob("*.gguf"))]
    for p in paths:
        if p and Path(p).is_file() and p not in models:
            models[p] = sha256(Path(p))
    runtime_dir = env.get("KOTORI_RUNTIME_DIR")
    runtime = {str(p): sha256(p) for p in sorted(Path(runtime_dir).glob("*.dll"))} if runtime_dir else {}
    return {
        "data": {"path": args.data, "sha256": sha256(Path(args.data)), "items": n},
        "context": args.context,
        "evidence_scope": "offline_converter_not_installed_tsf",
        "converter_source_commit": args.converter_source_commit or "unverified",
        "evaluator_sha256": sha256(Path(__file__)),
        "quality_module_sha256": sha256(Path(__file__).parent / "tools/conversion_quality.py"),
        "profile": args.profile,
        "profile_policy": "explicit" if args.profile else "isolated_temporary",
        "tracked_working_changes": bool(run(["git", "status", "--porcelain", "--untracked-files=no"])),
        "acc": acc,
        "converter_main": {"path": str(exe), "sha256": sha256(exe)},
        "models": models,
        "runtime": runtime,
        "env": {k: v for k, v in sorted(env.items()) if k.startswith("KOTORI_")},
        "commit": run(["git", "rev-parse", "HEAD"]),
        "gpu": run(["nvidia-smi", "--query-gpu=name,driver_version", "--format=csv,noheader"]),
        "host": {"platform": platform.platform(), "cpu": platform.processor(),
                 "logical_cpus": os.cpu_count()},
        "dictionary": {"path": str(exe.parent / (exe.name + ".runfiles") / "_main/data_manager/oss/mozc.data"),
                       "sha256": sha256(exe.parent / (exe.name + ".runfiles") / "_main/data_manager/oss/mozc.data")},
        "time": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }


def context_batches(items, use_context):
    """読み→文脈マップの衝突を別プロセスへ分離する。順序は保存する。"""
    batches, batch, contexts = [], [], {}
    for i, item in enumerate(items):
        reading = kata_to_hira(item["input"])
        context = item.get("context_text", "") if use_context else ""
        if reading in contexts and contexts[reading] != context:
            batches.append(batch)
            batch, contexts = [], {}
        contexts[reading] = context
        batch.append(i)
    if batch:
        batches.append(batch)
    return batches


def run_conversions(args, exe, cwd, items, env):
    blocks, stderr = [], []
    deadline = time.monotonic() + args.timeout
    batches = context_batches(items, args.context)
    if getattr(args, "case_traces", ""):
        batches = [[i] for i in range(len(items))]
    with tempfile.TemporaryDirectory(prefix="asteria-eval-") as directory:
        for batch_number, batch in enumerate(batches):
            batch_env = dict(env)
            # --context なしで外部の文脈マップが混入するのを防ぐ。
            batch_env.pop("KOTORI_LM_CONTEXT_MAP", None)
            if args.context:
                path = Path(directory) / f"context-{batch_number}.tsv"
                contexts = {kata_to_hira(items[i]["input"]): items[i].get("context_text", "")
                            for i in batch}
                path.write_text("".join(f"{r}\t{c}\n" for r, c in contexts.items() if c),
                                encoding="utf-8")
                batch_env["KOTORI_LM_CONTEXT_MAP"] = str(path)
            command = [str(exe)]
            if args.profile:
                profile = Path(args.profile).resolve()
                if len(batches) > 1:
                    profile = profile / f"batch-{batch_number}"
            else:
                profile = Path(directory) / f"profile-{batch_number}"
            profile.mkdir(parents=True, exist_ok=True)
            command.append("--user_profile_dir=" + str(profile))
            script = "".join(f"start {kata_to_hira(items[i]['input'])}\nreset\n{SEP}\n"
                             for i in batch)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, args.timeout)
            proc = subprocess.run(command, input=script.encode("utf-8"), capture_output=True,
                                  cwd=cwd, env=batch_env, timeout=remaining)
            stderr.append(proc.stderr)
            if getattr(args, "case_traces", ""):
                trace_dir = Path(args.case_traces)
                trace_dir.mkdir(parents=True, exist_ok=True)
                (trace_dir / f"{batch[0]}.stdout").write_bytes(proc.stdout)
                (trace_dir / f"{batch[0]}.stderr").write_bytes(proc.stderr)
            if proc.returncode:
                raise ValueError(f"変換器の終了コード {proc.returncode}: " +
                                 proc.stderr.decode("utf-8", "replace")[-2000:])
            current, batch_blocks, in_segment = [], [], False
            # Windowsの起動ヘッダーはACPのパスを含む。候補本文だけをUTF-8で厳密に読む。
            for raw_line in proc.stdout.splitlines():
                if raw_line.startswith(b"ExecCommand() return false"):
                    batch_blocks.append(current)
                    current = []
                    in_segment = False
                elif raw_line.startswith(b"---------- Segment "):
                    in_segment = True
                    current.append(raw_line.decode("utf-8", "strict"))
                elif in_segment:
                    current.append(raw_line.decode("utf-8", "strict"))
            if len(batch_blocks) != len(batch):
                raise ValueError(f"出力件数 {len(batch_blocks)} != 問題数 {len(batch)}")
            blocks.extend(batch_blocks)
    return blocks, b"\n".join(stderr), len(batches)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("converter_main")
    ap.add_argument("--data", default="eval/data/ajimee-bench.json")
    ap.add_argument("--out", default="")
    ap.add_argument("--profile", default="", help="試験専用のconverter_mainプロファイル。普段のIMEから分離する")
    ap.add_argument("--context", action="store_true", help="問題の context_text を前の文として AI に渡す")
    ap.add_argument("--timeout", type=float, default=36000, help="全体の制限時間(秒)")
    ap.add_argument("--stderr", default="", help="変換器の stderr(KOTORI_LM_DEBUG などの出力)を保存するファイル")
    ap.add_argument("--min-acc", type=float, default=None, help="Acc@1(%%)がこれ未満なら失敗にする")
    ap.add_argument("--converter-source-commit", default="", help="変換器ビルド元の記録。未指定なら未検証")
    ap.add_argument("--case-traces", default="", help="同意済み診断入力専用。各問を別プロセスで実行して本文を含む原票を保存。性能測定には使わない")
    args = ap.parse_args()

    exe = Path(args.converter_main).resolve()
    cwd = exe.parent / (exe.name + ".runfiles") / "_main"
    data_bytes = Path(args.data).read_bytes()
    items = json.loads(data_bytes)
    env = dict(os.environ)
    env.pop("KOTORI_LM_CONTEXT_MAP", None)
    env.pop("KOTORI_LM_PRECEDING", None)
    try:
        validate_items(items)
        before_metadata = manifest(args, exe, env, 0, len(items)) if args.out else None
        if before_metadata and before_metadata["data"]["sha256"] != hashlib.sha256(data_bytes).hexdigest():
            raise ValueError("読み込んだ評価データが測定前に変更されました")
        t0 = time.time()
        blocks, stderr, batch_count = run_conversions(args, exe, cwd, items, env)
        elapsed = time.time() - t0
        metadata = manifest(args, exe, env, 0, len(items)) if args.out else None
        if metadata and any(metadata[key] != before_metadata[key] for key in
                            ("data", "converter_main", "dictionary", "models", "runtime",
                             "evaluator_sha256", "quality_module_sha256")):
            raise ValueError("測定中にデータ・変換器・辞書・モデル・評価器が変更されました")
    except subprocess.TimeoutExpired:
        print(f"失敗: {args.timeout:.0f} 秒で終わらなかった", file=sys.stderr)
        return 1
    except OSError as e:
        print(f"失敗: 変換器を起動できない: {e}", file=sys.stderr)
        return 1
    except (ValueError, UnicodeError) as e:
        print(f"失敗: {e}", file=sys.stderr)
        return 1
    if args.stderr:
        Path(args.stderr).write_bytes(stderr)
    # KOTORI_LM_TIME を付けたときは、AI の変換にかかった時間(文全体の選択)の分布も出す。
    times = sorted(float(m.group(1)) for m in
                   re.finditer(r"\[time\].* total=(\d+)ms", stderr.decode("utf-8", "replace")))
    rows, hit = [], 0
    for i, it in enumerate(items):
        try:
            row = score_case(it, blocks[i])
            if args.case_traces:
                trace = Path(args.case_traces) / f"{i}.stderr"
                row["scoring_trace"] = scoring_trace(trace.read_bytes(), it["expected_output"])
                row["trace"] = {"case_index": i, "stderr_sha256": sha256(trace),
                                "stdout_sha256": sha256(Path(args.case_traces) / f"{i}.stdout")}
        except ValueError as e:
            print(f"失敗: 問題 {it['index']}: {e}", file=sys.stderr)
            return 1
        hit += row["ok"]
        rows.append(row)
    lo, hi = wilson(hit, len(items))
    print(f"Acc@1 {hit}/{len(items)} = {100 * hit / len(items):.1f}%(95% 区間 {lo:.1f}〜{hi:.1f}%)  "
          f"({elapsed:.0f} 秒)")
    if times:
        print(f"AI の変換 中央値 {times[len(times) // 2]:.0f} ms、p95 {times[int(len(times) * 0.95)]:.0f} ms、"
              f"最大 {times[-1]:.0f} ms")
    # 分野(domain)のある評価セット(kotori-heldout)では、分野ごとの Acc@1 も出す。
    domains = {}
    for it, row in zip(items, rows):
        if "domain" in it:
            d = domains.setdefault(it["domain"], [0, 0])
            d[0] += row["ok"]
            d[1] += 1
    if domains:
        print("  " + "、".join(f"{k} {v[0]}/{v[1]}" for k, v in domains.items()))
    acc = 100 * hit / len(items)
    if args.out:
        json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
        metadata["acc"] = acc
        metadata.update(result_sha256=sha256(Path(args.out)), context_batches=batch_count)
        metadata["case_isolated"] = bool(args.case_traces)
        json.dump(metadata,
                  open(args.out + ".manifest.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    if args.min_acc is not None and acc < args.min_acc:
        print(f"失敗: Acc@1 {acc:.1f}% が基準 {args.min_acc:.1f}% を下回った", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
