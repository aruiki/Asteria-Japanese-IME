"""変換器の観測候補を採点する。内部探索全体やTSF反映の証拠とは区別する。"""
import re
import math


SEGMENT = re.compile(r"^-{10} Segment (\d+)/(\d+) \[([^]]+)\] -{10}$")
CANDIDATE = re.compile(r"^  (\d+)/(\d+) (.*)$")
SCORE = re.compile(r"^\[kotori\] (.+) zenz=([^ ]+) llm=([^ ]+) (lattice|gen) score=([^ ]+)$")


def scoring_trace(stderr, expected):
    """一要求だけのログで候補総数と照合する。欠落時は未検証のままにする。"""
    candidates = []
    times = []
    for line in stderr.decode("utf-8", "strict").splitlines():
        if line.startswith("[time] "):
            fields = dict(re.findall(r"(\w+)=([^ ]+)", line))
            times.append(fields)
        elif match := SCORE.fullmatch(line):
            value, zenz, llm, origin, score = match.groups()
            candidates.append(dict(value=value, zenz=float(zenz), llm=float(llm),
                                   origin=origin, score=float(score)))
    if len(times) != 1 or not candidates or len(candidates) != int(times[0].get("cands", -1)):
        return {"status": "unverified", "reason": "missing_or_multiple_scoring_records"}
    matches = [c for c in candidates if c["value"] in expected]
    llm_on = times[0].get("llm_on") == "1"
    top = max(c["score"] for c in candidates)
    return {"status": "complete_logged_pool", "candidates": len(candidates),
            "expected_in_pool": bool(matches), "llm_on": llm_on,
            "expected_llm_scored": any(math.isfinite(c["llm"]) for c in matches) if llm_on else None,
            "expected_jointly_selectable": any(math.isfinite(c["score"]) for c in matches),
            "expected_has_max_logged_score": (any(c["score"] == top for c in matches)
                                                if math.isfinite(top) else None),
            "timing": times[0]}


def parse_segments(lines):
    segments = []
    current = None
    declared = None
    for line in lines:
        match = SEGMENT.fullmatch(line)
        if match:
            number, total, kind = match.groups()
            number, total = int(number), int(total)
            if kind == "HISTORY":
                raise ValueError("履歴文節を含む出力は独立した変換として採点できません")
            if number != len(segments) or (declared is not None and total != declared):
                raise ValueError("文節の順序または総数が不整合です")
            declared = total
            current = {"values": [], "total": None}
            segments.append(current)
        elif current is not None and (match := CANDIDATE.fullmatch(line)):
            rank, total, value = match.groups()
            rank, total = int(rank), int(total)
            if rank != len(current["values"]) or not value or total <= rank:
                raise ValueError("候補の順位、総数または本文が不正です")
            if current["total"] not in (None, total):
                raise ValueError("候補総数が不整合です")
            current["total"] = total
            current["values"].append(value)
    if not segments or declared != len(segments) or any(not s["values"] for s in segments):
        raise ValueError("変換出力の文節または第一候補が欠落しています")
    return segments


def reachable(segments, expected, per_segment_k=None):
    """候補直積を作らず、正解の接頭辞をたどる。全文の候補順位ではない。"""
    positions = {0}
    for segment in segments:
        values = segment["values"][:per_segment_k]
        positions = {pos + len(value) for pos in positions for value in values
                     if expected.startswith(value, pos)}
        if not positions:
            return False
    return len(expected) in positions


def score_case(item, lines, per_segment_k=5):
    segments = parse_segments(lines)
    expected = item["expected_output"]
    top = "".join(s["values"][0] for s in segments)
    ok = top in expected
    included = any(reachable(segments, value) for value in expected)
    truncated = any(len(s["values"]) < s["total"] for s in segments)
    stage = ("correct" if ok else "candidate_present_not_selected" if included else
             "unobserved_candidates" if truncated else "candidate_or_segmentation")
    return {"index": item["index"], "input": item["input"], "top1": top,
            "expected": expected, "ok": ok, "domain": item.get("domain", "unspecified"),
            "context_text": item.get("context_text", ""),
            "candidate_coverage": included, "candidates_truncated": truncated,
            "per_segment_k": per_segment_k,
            "reachable_at_per_segment_k": any(reachable(segments, value, per_segment_k)
                                               for value in expected),
            "diagnostic_stage": stage, "ai_delivery": "unverified",
            "segments": segments}


def validate_items(items):
    if not isinstance(items, list) or not items:
        raise ValueError("評価セットは空でない配列が必要です")
    seen = set()
    for item in items:
        index = item.get("index")
        if not isinstance(index, (str, int)) or isinstance(index, bool) or index in seen:
            raise ValueError("問題IDが不正または重複しています")
        seen.add(index)
        for key in ("input", "context_text"):
            value = item.get(key, "")
            if not isinstance(value, str) or any(c in value for c in "\r\n\t\0"):
                raise ValueError(f"{key} が不正です")
        if not item.get("input", "").strip():
            raise ValueError("読みが空です")
        expected = item.get("expected_output")
        if not isinstance(expected, list) or not expected or any(
                not isinstance(v, str) or not v for v in expected):
            raise ValueError("空でない正解表記集合が必要です")


def compare_rows(before, after):
    """並び順ではなくIDで対応づけ、正解集合や文脈を変えた比較を拒否する。"""
    def indexed(rows):
        result = {}
        for row in rows:
            key = row["index"]
            if key in result:
                raise ValueError("比較結果の問題IDが重複しています")
            if not isinstance(row["ok"], bool) or row["ok"] != (row["top1"] in row["expected"]):
                raise ValueError("正誤の記録が候補と一致しません")
            result[key] = row
        if not result:
            raise ValueError("比較結果が空です")
        return result
    a, b = indexed(before), indexed(after)
    if a.keys() != b.keys():
        raise ValueError("比較対象の問題ID集合が異なります")
    ledger = []
    for key, old in a.items():
        new = b[key]
        if any(old.get(field) != new.get(field) for field in
               ("input", "expected", "context_text", "domain")):
            raise ValueError(f"問題 {key} の読み・正解・文脈・分野が異なります")
        outcome = ("improved" if not old["ok"] and new["ok"] else
                   "regressed" if old["ok"] and not new["ok"] else
                   "unchanged_correct" if new["ok"] else "still_wrong")
        ledger.append({"index": key, "outcome": outcome, "before": old["top1"],
                       "after": new["top1"], "changed": old["top1"] != new["top1"],
                       "stage": new.get("diagnostic_stage", "unverified")})
    counts = {outcome: sum(r["outcome"] == outcome for r in ledger) for outcome in
              ("improved", "regressed", "unchanged_correct", "still_wrong")}
    return {"items": len(ledger), **counts,
            "net_improvement": counts["improved"] - counts["regressed"],
            "changed": sum(r["changed"] for r in ledger), "ledger": ledger}
