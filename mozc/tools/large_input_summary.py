"""大量実行の原票を保ち、入力数/操作数/疑義/待ち時間を分けて集計する。"""
import argparse
import collections
import json
import math
from pathlib import Path
from large_input_campaign import sha


def percentile(values, p):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(p*len(ordered))-1)] if ordered else None


def summarize(cases, rows):
    by_id = {case['id']: case for case in cases}
    if len(by_id) != len(cases): raise ValueError('duplicate corpus ID')
    if len({case['reading'] for case in cases}) != len(cases): raise ValueError('duplicate corpus reading')
    seen = set(); modes = collections.Counter(); defects = collections.Counter(); reviews = collections.Counter()
    families = collections.Counter(); latencies = collections.defaultdict(list); findings = []
    primary_ids = set(); gold_ids = set(); gold_flags = set()
    for row in rows:
        sid = row['scenario_id']
        if sid in seen: raise ValueError('duplicate scenario: ' + sid)
        seen.add(sid)
        case = by_id[row['case_id']]
        if row['reading'] != case['reading'] or row['family'] != case['family'] or row['expected'] != case['expected']:
            raise ValueError('row differs from pinned corpus: ' + sid)
        if sid != case['id'] + ':' + row['mode']: raise ValueError('scenario ID differs from mode')
        modes[row['mode']] += 1; defects.update(row['defects']); reviews.update(row['reviews'])
        if row['mode'] == 'immediate':
            primary_ids.add(case['id']); families[case['family']] += 1
            if case['expected'] or any(p['command'] in ['Conversion Match','Conversion Not Match'] for p in case['predicates']):
                gold_ids.add(case['id'])
                if any(flag in row['reviews'] for flag in ['expected_surface_mismatch','upstream_match_missing','upstream_forbidden_surface']):
                    gold_flags.add(case['id'])
        for event_name, event in row['events'].items():
            if 'ms' in event: latencies[row['mode'] + ':' + event_name].append(event['ms'])
        if row['defects'] or row['reviews']:
            # 原期待値/判定を変更しない。表記揺れや複数文節の候補は別途人が再現/分類する。
            findings.append(dict(row, predicates=case['predicates'], surname_targets=case.get('surname_targets',[])))
    return {'unique_inputs_requested': len(cases), 'unique_primary_inputs_executed': len(primary_ids),
            'primary_complete': primary_ids == set(by_id), 'scenario_counts': dict(modes), 'scenarios_total': len(rows),
            'primary_families': dict(families), 'primary_meaning_oracle_cases': len(gold_ids),
            'primary_meaning_oracle_flagged': len(gold_flags), 'defect_flags': dict(defects), 'review_flags': dict(reviews),
            'latency_ms': {key: {'n':len(v),'p50':percentile(v,.5),'p95':percentile(v,.95),
                                'p99':percentile(v,.99),'max':max(v)} for key,v in latencies.items()},
            'native_tsf': False, 'strict_mismatch_is_not_automatically_a_semantic_bug': True}, findings


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--cases', type=Path, required=True); ap.add_argument('--run', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    cases = [json.loads(l) for l in args.cases.read_text(encoding='utf-8').splitlines() if l]
    results = args.run/'results.jsonl'
    rows = [json.loads(l) for l in results.read_text(encoding='utf-8').splitlines() if l]
    report, findings = summarize(cases, rows)
    manifest = json.loads((args.run/'manifest.json').read_text(encoding='utf-8'))
    if manifest['identity']['cases_sha256'] != sha(args.cases): raise ValueError('case SHA differs from run')
    report.update(cases_sha256=sha(args.cases),results_sha256=sha(results),run_manifest_sha256=sha(args.run/'manifest.json'))
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out/'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    (args.out/'findings.jsonl').write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in findings),encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False,indent=2))


if __name__ == '__main__': main()
