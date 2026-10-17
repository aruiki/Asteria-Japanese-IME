"""実候補群の参照選好で小さな採点器を学習する開発用実験。製品へ自動適用しない。"""
import argparse
import hashlib
import json
import math
from pathlib import Path


def examples(rows):
    result = []
    excluded = {"missing_reference_score": 0, "no_competitor": 0}
    for row in rows:
        values = {}
        for candidate in row["candidates"]:
            feature = [float(candidate["zenz"]), float(candidate["llm"])]
            if any(math.isnan(x) or x == math.inf for x in feature):
                raise ValueError("採点特徴がNaNまたは正の無限大です")
            if not all(math.isfinite(x) for x in feature):
                continue
            if candidate["text"] in values and values[candidate["text"]] != feature:
                raise ValueError("同じ候補の採点特徴が一致しません")
            values[candidate["text"]] = feature
        references = set(row["references"])
        if not references.intersection(values):
            excluded["missing_reference_score"] += 1
            continue
        if not set(values).difference(references):
            excluded["no_competitor"] += 1
            continue
        result.append({"index": row["index"], "reading": row["reading"],
                       "references": references, "texts": list(values),
                       "x": list(values.values()), "final": row["final"],
                       "source_groups": set(row["source_groups"]), "split": row["split"]})
    if not result:
        raise ValueError("有限の参照スコアと競合候補を持つ実例がありません")
    return result, excluded


def logsumexp(values):
    maximum = max(values)
    return maximum + math.log(sum(math.exp(v - maximum) for v in values))


def loss_gradient(example, weights, scale):
    base = example["x"][0]
    x = [[(v[j] - base[j]) / scale[j] for j in range(2)] for v in example["x"]]
    scores = [sum(a * b for a, b in zip(v, weights)) for v in x]
    gold = [i for i, text in enumerate(example["texts"]) if text in example["references"]]
    total, positive = logsumexp(scores), logsumexp([scores[i] for i in gold])
    gradient = [sum(math.exp(scores[i] - total) * x[i][j] for i in range(len(x))) -
                sum(math.exp(scores[i] - positive) * x[i][j] for i in gold) for j in range(2)]
    return total - positive, gradient


def fit(train, regularization, steps=1200):
    differences = [[v[j] - e["x"][0][j] for j in range(2)] for e in train for v in e["x"]]
    scale = [max(1e-6, math.sqrt(sum(v[j] ** 2 for v in differences) / len(differences))) for j in range(2)]
    weights = [0.0, 0.0]
    for _ in range(steps):
        gradient = [0.0, 0.0]
        for example in train:
            _, g = loss_gradient(example, weights, scale)
            gradient = [a + b for a, b in zip(gradient, g)]
        weights = [max(0, w - 0.1 * (g / len(train) + regularization * w))
                   for w, g in zip(weights, gradient)]
    return [w / s for w, s in zip(weights, scale)]


def evaluate(data, weights):
    before, after, improved, regressed = 0, 0, 0, 0
    observed_final, downstream_changes = 0, 0
    loss = 0.0
    for example in data:
        scores = [sum(a * b for a, b in zip(value, weights)) for value in example["x"]]
        top = max(range(len(scores)), key=scores.__getitem__)
        # 最終表示と内部候補の文字列は後段の表記処理で変わる。
        # 採点を再計算しただけでその差を改善と数えない。
        legacy_scores = [v[0] + 0.7 * v[1] for v in example["x"]]
        legacy_top = max(range(len(legacy_scores)), key=legacy_scores.__getitem__)
        old = example["texts"][legacy_top] in example["references"]
        new = example["texts"][top] in example["references"]
        observed_final += example["final"] in example["references"]
        downstream_changes += example["final"] != example["texts"][legacy_top]
        before += old
        after += new
        improved += new and not old
        regressed += old and not new
        loss += loss_gradient(example, weights, [1, 1])[0]
    return {"eligible_cases": len(data), "before": before, "after": after,
            "improved": improved, "regressed": regressed, "nll": loss / len(data),
            "observed_final_correct": observed_final,
            "pool_vs_final_surface_changes": downstream_changes,
            "scope": "offline_pool_ranking_not_final_conversion"}


def load(path):
    raw = Path(path).read_bytes()
    manifest = json.loads((Path(path).parent / "manifest.json").read_text(encoding="utf-8"))
    if hashlib.sha256(raw).hexdigest() != manifest["observations_sha256"]:
        raise ValueError("実候補群のSHAが不一致です")
    return [json.loads(line) for line in raw.splitlines()], hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--train", required=True)
    parser.add_argument("--dev", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error("既存の実験結果は上書きしません")
    raw_train, train_hash = load(args.train)
    raw_dev, dev_hash = load(args.dev)
    train, excluded_train = examples(raw_train)
    dev, excluded_dev = examples(raw_dev)
    if any(r["split"] != "train" for r in raw_train) or any(r["split"] != "dev" for r in raw_dev):
        raise ValueError("train/devの取り違えがあります")
    # 除外した例も含め、元の取得区間と読みの漏洩を拒否する。
    for field in ("reading", "source_groups"):
        a = {v for r in raw_train for v in (r[field] if field == "source_groups" else [r[field]])}
        b = {v for r in raw_dev for v in (r[field] if field == "source_groups" else [r[field]])}
        if a & b:
            raise ValueError(f"train/devの{field}が重複しています")
    fits = []
    for regularization in (0.001, 0.01, 0.1, 1.0):
        weights = fit(train, regularization)
        fits.append({"regularization": regularization, "weights": weights,
                     "train": evaluate(train, weights), "dev": evaluate(dev, weights)})
    baseline = {"kind": "unchanged", "weights": [1, 0.7],
                "train": evaluate(train, [1, 0.7]), "dev": evaluate(dev, [1, 0.7])}
    best = min([baseline] + fits, key=lambda value: value["dev"]["nll"])
    report = {"train_sha256": train_hash, "dev_sha256": dev_hash,
              "train_observed": len(raw_train), "dev_observed": len(raw_dev),
              "excluded_train": excluded_train, "excluded_dev": excluded_dev,
              "baseline_dev": evaluate(dev, [1, 0.7]), "fits": fits, "selected": best,
              "decision": "retain_baseline" if best is baseline else "offline_candidate_only",
              "supervision": "corpus_reference_preference_not_universal_correctness",
              "product_adopted": False,
              "tool_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
