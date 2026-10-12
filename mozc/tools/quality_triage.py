"""同じ評価データの修正前後を照合し、デバッグ用の失敗例を抽出する。

例: python mozc/tools/quality_triage.py --before target/conversion-quality-debug/ascii-runtime --after target/conversion-quality-debug/unicode-after --baseline target/conversion-quality-debug/unicode-after --out target/conversion-quality-debug/triage.json
抽出した例は調整用。同じ例で改善しても独立した最終精度とは呼ばない。
"""
import argparse
import hashlib
import json
from pathlib import Path

SETS = {'daily': 'eval/sets/kotori-daily.json', 'ajimee': 'eval/data/ajimee-bench.json',
        'idioms': 'eval/sets/kotori-idioms.json', 'nuance': 'eval/sets/kotori-nuance.json',
        'notation': 'eval/sets/kotori-notation.json'}


def indexed(rows):
    result = {}
    for row in rows:
        key = str(row['index'])
        if key in result:
            raise ValueError('duplicate case index: ' + key)
        result[key] = row
    return result


def compare(before, after, baseline):
    b, a, m = map(indexed, (before, after, baseline))
    if b.keys() != a.keys() or b.keys() != m.keys():
        raise ValueError('case indices differ')
    failures, improved, regressed = [], [], []
    for key, row in a.items():
        for other in (b[key], m[key]):
            if (row['input'], row['expected']) != (other['input'], other['expected']):
                raise ValueError('reading or expected outputs differ: ' + key)
        # 古い/手編集したok値を信じず、保存した候補を期待表記と比較する。
        ok = row['top1'] in row['expected']
        old_ok = b[key]['top1'] in b[key]['expected']
        if ok and not old_ok:
            improved.append(row['index'])
        if old_ok and not ok:
            regressed.append(row['index'])
        if not ok:
            failures.append({**row, 'ok': False, 'before': b[key]['top1'],
                             'mozc': m[key]['top1'],
                             'mozc_was_correct': m[key]['top1'] in m[key]['expected']})
    return {'items': len(a), 'correct': len(a) - len(failures),
            'improved': improved, 'regressed': regressed, 'failures': failures}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for arg in ('before', 'after', 'baseline', 'out'):
        ap.add_argument('--' + arg, type=Path, required=True)
    ap.add_argument('--require-ai', action='store_true', help='前後のStandardに実採点ログがなければ失敗する')
    args = ap.parse_args()
    results = {}
    for name, dataset in SETS.items():
        paths = [args.before / f'Standard_{name}.json', args.after / f'Standard_{name}.json',
                 args.baseline / f'Mozc_{name}.json']
        manifests = [json.loads(p.with_name(p.name + '.manifest.json').read_text(encoding='utf-8')) for p in paths]
        identity = [(m['data']['sha256'], m['data']['items'], m['context']) for m in manifests]
        if len(set(identity)) != 1:
            raise ValueError('dataset/context differs: ' + name)
        if args.require_ai:
            for path in paths[:2]:
                log = path.with_suffix('.stderr')
                if not log.exists() or '[time] chars=' not in log.read_text(encoding='utf-8', errors='replace'):
                    raise ValueError('AI scoring was not observed: ' + str(path))
        result = compare(*(json.loads(p.read_text(encoding='utf-8')) for p in paths))
        if result['items'] != manifests[0]['data']['items']:
            raise ValueError('manifest item count differs: ' + name)
        if hashlib.sha256(Path(dataset).read_bytes()).hexdigest() != manifests[0]['data']['sha256']:
            raise ValueError('source dataset changed: ' + name)
        originals = indexed(json.loads(Path(dataset).read_text(encoding='utf-8')))
        for failure in result['failures']:
            failure['context_text'] = originals[str(failure['index'])].get('context_text', '')
            failure['source_dataset'] = dataset
        results[name] = result
        print(f"{name}: {result['correct']}/{result['items']} 改善{len(result['improved'])} 悪化{len(result['regressed'])}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(results, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print('失敗例:', sum(len(r['failures']) for r in results.values()), '出力:', args.out)


if __name__ == '__main__':
    main()