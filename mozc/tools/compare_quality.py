#!/usr/bin/env python3
"""同一問題・同一正解集合の前後比較と誤変換台帳をJSONで保存する。"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from conversion_quality import compare_rows


def load_result(path):
    path = Path(path)
    raw = path.read_bytes()
    metadata = json.loads(Path(str(path) + ".manifest.json").read_text(encoding="utf-8"))
    if metadata.get("result_sha256") != hashlib.sha256(raw).hexdigest():
        raise ValueError(f"結果のSHA-256が未記録または不一致: {path}")
    if not metadata.get("data", {}).get("sha256"):
        raise ValueError("評価データのSHA-256が必要です")
    rows = json.loads(raw)
    if len(rows) != metadata["data"]["items"]:
        raise ValueError("結果とmanifestの問題数が異なります")
    return rows, metadata


def summarize(rows):
    n = len(rows)
    covered = sum(r["candidate_coverage"] for r in rows)
    return {"items": n, "top1_correct": sum(r["ok"] for r in rows),
            "observed_candidate_coverage": covered,
            "reachable_at_per_segment_k": sum(r["reachable_at_per_segment_k"] for r in rows),
            "truncated_items": sum(r["candidates_truncated"] for r in rows),
            "stages": dict(Counter(r["diagnostic_stage"] for r in rows)),
            "scoring_failures": dict(Counter(
                "unverified" if r.get("scoring_trace", {}).get("status") != "complete_logged_pool" else
                "expected_missing_from_pool" if not r["scoring_trace"]["expected_in_pool"] else
                "expected_excluded_from_joint_score" if not r["scoring_trace"]["expected_jointly_selectable"] else
                "expected_near_logged_max_final_mismatch" if r["scoring_trace"].get("expected_has_max_logged_score") else
                "expected_scored_below_winner"
                for r in rows if not r["ok"])),
            "domains": {domain: {"items": sum(r["domain"] == domain for r in rows),
                                  "correct": sum(r["domain"] == domain and r["ok"] for r in rows)}
                        for domain in sorted({r["domain"] for r in rows})}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    try:
        before, a = load_result(args.before)
        after, b = load_result(args.after)
        if a["data"]["sha256"] != b["data"]["sha256"] or a["context"] != b["context"]:
            raise ValueError("データまたは文脈の条件が異なります")
        if a.get("dictionary") != b.get("dictionary") or a.get("case_isolated") != b.get("case_isolated"):
            raise ValueError("辞書または要求分離の条件が異なります")
        report = compare_rows(before, after)
        report.update(before=summarize(before), after=summarize(after),
                      before_manifest=a, after_manifest=b,
                      evidence_scope="offline_converter_not_installed_tsf",
                      caveats=["候補包含は出力された文節候補の組合せ。内部のAI探索集合とは異なる",
                               "K=5は各文節の上位5候補。全文の上位5件ではない",
                               "AI反映率・修正操作数・重大誤変換の意味判定・TSFは未検証"])
        Path(args.out).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"改善 {report['improved']} / 改悪 {report['regressed']} / "
              f"変更 {report['changed']} / 全 {report['items']}")
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"比較失敗: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
