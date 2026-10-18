"""実変換候補の学習準備。公開コーパスの参照表記を万能な正解と呼ばない。"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import random
import unicodedata

from conversion_quality import SCORE, scoring_trace, validate_items
from compare_quality import load_result


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def reading_key(value):
    value = unicodedata.normalize("NFKC", value)
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c
                   for c in value if not c.isspace())


def grams(value):
    return {value[i:i + 3] for i in range(max(0, len(value) - 2))}


def evaluation_readings(value):
    """変換と入力中予測の読みだけを除外に使い、期待出力を学習へ渡さない。"""
    if isinstance(value, list):
        for item in value:
            yield from evaluation_readings(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            if key in ("input", "reading", "yomi", "kana", "typed") and isinstance(item, str):
                yield reading_key(item)
            elif isinstance(item, (dict, list)):
                yield from evaluation_readings(item)


def similar(a, b, threshold=0.8):
    if a == b:
        return True
    x, y = grams(a), grams(b)
    return bool(x and y) and len(x & y) / len(x | y) >= threshold


def prepare(rows, excluded, train_limit, dev_limit, seed=20261017, max_reading=64):
    if min(train_limit, dev_limit) <= 0:
        raise ValueError("train/dev件数は正の数が必要です")
    groups = {"train": {}, "dev": {}}
    counts = Counter()
    excluded_grams = [grams(value) for value in excluded]
    exclusion_cache = {}
    for row in rows:
        if row.get("kind", "original") != "original":
            counts["non_original"] += 1
            continue
        split = row["split"]
        if split not in groups or not row.get("source_group"):
            raise ValueError("コーパスの分割または取得区間が不明です")
        reading, context, reference = row["reading"], row.get("context", ""), row["positive"]
        key = reading_key(reading)
        if key not in exclusion_cache:
            value_grams = grams(key)
            exclusion_cache[key] = key in excluded or any(
                value_grams and other and
                min(len(value_grams), len(other)) / max(len(value_grams), len(other)) >= 0.8 and
                len(value_grams & other) / len(value_grams | other) >= 0.8
                for other in excluded_grams)
        if exclusion_cache[key]:
            counts["evaluation_reading"] += 1
            continue
        if not 6 <= len(key) <= max_reading:
            counts["reading_length"] += 1
            continue
        identity = (key, context)
        entry = groups[split].setdefault(identity, {
            "input": reading, "context_text": context, "expected_output": [],
            "source_groups": set(), "label_scope": "corpus_reference_preference"})
        entry["source_groups"].add(row["source_group"])
        if reference not in entry["expected_output"]:
            entry["expected_output"].append(reference)
    train_keys = {k[0] for k in groups["train"]}
    dev_keys = {k[0] for k in groups["dev"]}
    if train_keys & dev_keys:
        raise ValueError("正規化読みがtrain/dev間で重複しています")
    source_groups = {split: {g for r in values.values() for g in r["source_groups"]}
                     for split, values in groups.items()}
    if source_groups["train"] & source_groups["dev"]:
        raise ValueError("同一コーパス取得区間がtrain/devへ混在しています")
    rng = random.Random(seed)
    selected = {}
    for split, limit in (("train", train_limit), ("dev", dev_limit)):
        values = sorted(groups[split].items())
        rng.shuffle(values)
        chosen = []
        for (key, _), row in values:
            # 少数の反復文や、trainとほぼ同じdevによる見かけの改善を避ける。
            blockers = [reading_key(r["input"]) for r in chosen]
            if split == "dev":
                blockers += [reading_key(r["input"]) for r in selected["train"]]
            if any(similar(key, previous) for previous in blockers):
                counts[split + "_near_duplicate"] += 1
                continue
            row = dict(row, index=f"{split}-{len(chosen):05d}", split=split,
                       domain="public-corpus-original", source_groups=sorted(row["source_groups"]))
            chosen.append(row)
            if len(chosen) == limit:
                break
        if len(chosen) != limit:
            raise ValueError(f"独立した{split}が不足: {len(chosen)}/{limit}")
        validate_items(chosen)
        selected[split] = chosen
    return selected, dict(counts)


def preferences(cases, results, trace_dir):
    """正解/誤りの自動断定を避け、実候補と参照表記の選好を記録する。"""
    validate_items(cases)
    if len(results) != len(cases):
        raise ValueError("変換結果の件数が異なります")
    observations, counts = [], Counter()
    for number, (case, row) in enumerate(zip(cases, results)):
        if (row["index"], row["input"], row["expected"], row["context_text"]) != (
                case["index"], case["input"], case["expected_output"], case["context_text"]):
            raise ValueError("結果とコーパス参照の対応が異なります")
        if not isinstance(row["ok"], bool) or row["ok"] != (row["top1"] in case["expected_output"]):
            raise ValueError("記録された正誤が参照表記と一致しません")
        trace_path = Path(trace_dir) / f"{number}.stderr"
        if digest(trace_path) != row.get("trace", {}).get("stderr_sha256"):
            raise ValueError("要求単位の採点原票SHAが不一致です")
        raw = trace_path.read_bytes()
        trace = scoring_trace(raw, case["expected_output"])
        if trace["status"] != "complete_logged_pool":
            counts["unverified_pool"] += 1
            continue
        # スコアは文字列表現を保持し、未採点の-infを有限スコアに変えない。
        candidates = []
        for line in raw.decode("utf-8").splitlines():
            if match := SCORE.fullmatch(line):
                text, zenz, llm, origin, score = match.groups()
                candidates.append(dict(text=text, zenz=zenz, llm=llm, origin=origin,
                                       score=score, matches_reference=text in case["expected_output"]))
        observations.append({"index": case["index"], "split": case["split"],
                             "reading": case["input"], "context": case["context_text"],
                             "references": case["expected_output"], "candidates": candidates,
                             "final": row["top1"], "source_groups": case["source_groups"],
                             "trace_sha256": digest(trace_path),
                             "supervision": "corpus_reference_preference",
                             "alternatives_semantically_reviewed": False})
        counts["complete_pool"] += 1
        counts["reference_in_pool"] += trace["expected_in_pool"]
        counts["final_matches_reference"] += row["ok"]
    return observations, dict(counts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prep = commands.add_parser("prepare")
    prep.add_argument("--pairs", type=Path, required=True)
    prep.add_argument("--evaluation", type=Path, action="append", required=True)
    prep.add_argument("--train", type=int, default=64)
    prep.add_argument("--dev", type=int, default=32)
    prep.add_argument("--out", type=Path, required=True)
    collect = commands.add_parser("collect")
    collect.add_argument("--cases", type=Path, required=True)
    collect.add_argument("--results", type=Path, required=True)
    collect.add_argument("--traces", type=Path, required=True)
    collect.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error("既存の原票は上書きしません。新しい出力先を指定してください")
    if args.command == "prepare":
        raw = args.pairs.read_bytes()
        source = json.loads(args.pairs.with_suffix(".manifest.json").read_text(encoding="utf-8"))
        if hashlib.sha256(raw).hexdigest() != source["pairs_sha256"]:
            raise ValueError("コーパス対のSHAが不一致です")
        evaluations = {str(p): digest(p) for p in args.evaluation}
        required = {record["sha256"] for record in source["excluded_evaluations"]}
        if not required <= set(evaluations.values()):
            raise ValueError("元のコーパス作成時に除外した評価セットが不足しています")
        excluded = set()
        for path in args.evaluation:
            readings = set(evaluation_readings(json.loads(path.read_text(encoding="utf-8"))))
            if not readings:
                raise ValueError(f"評価セットの読みフィールドが不明です: {path}")
            excluded.update(readings)
        selected, counts = prepare([json.loads(line) for line in raw.splitlines()],
                                   excluded, args.train, args.dev)
        args.out.mkdir(parents=True)
        for split, cases in selected.items():
            (args.out / f"{split}.json").write_text(json.dumps(cases, ensure_ascii=False, indent=2), encoding="utf-8")
        metadata = {"source_sha256": hashlib.sha256(raw).hexdigest(), "evaluations": evaluations,
                    "seed": 20261017, "near_duplicate_jaccard": 0.8, "filter_counts": counts,
                    "cases": {split: digest(args.out / f"{split}.json") for split in selected},
                    "label_scope": "corpus_reference_preference_not_universal_correctness"}
    else:
        rows, metadata = load_result(args.results)
        if metadata["data"]["sha256"] != digest(args.cases) or not (
                metadata.get("case_isolated") or metadata.get("request_traced")):
            raise ValueError("固定ケースと要求に対応づけた実行原票が必要です")
        observations, counts = preferences(json.loads(args.cases.read_text(encoding="utf-8")), rows, args.traces)
        args.out.mkdir(parents=True)
        (args.out / "observations.jsonl").write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in observations), encoding="utf-8")
        metadata = {"results_sha256": digest(args.results), "cases_sha256": digest(args.cases),
                    "observations_sha256": digest(args.out / "observations.jsonl"), "counts": counts,
                    "supervision": "corpus_reference_preference", "product_adopted": False}
    metadata["tool_sha256"] = digest(__file__)
    (args.out / "manifest.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(metadata, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
