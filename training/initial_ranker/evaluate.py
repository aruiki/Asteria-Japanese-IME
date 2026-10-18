"""較正済み係数を固定し、製品の既存開発セットで改善/改悪を数える。"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from calibrate import score_rows
from model import CharacterModel, ranked_choice
from prepare import digest, ROOT


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('model', 'configuration', 'exe', 'baseline', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--source-commit', required=True)
    args = ap.parse_args()
    configuration_path = args.configuration / 'configuration.json'
    config = json.loads(configuration_path.read_text(encoding='utf-8'))
    model_path = args.model / 'character-model.json.gz'
    if digest(model_path) != config['model_sha256'] or not config['corpus_only_calibration']:
        raise ValueError('Model/calibration identity mismatch')
    chosen = config['chosen']
    if chosen['improvements'] <= 0 or chosen['regressions']:
        raise ValueError('Calibration did not justify product comparison')
    args.out.mkdir(parents=True, exist_ok=False)
    script_paths = [Path(__file__), Path(__file__).with_name('model.py'), Path(__file__).with_name('collect.py'), Path(__file__).with_name('calibrate.py')]
    frozen = {str(p.resolve()): digest(p) for p in [args.exe, model_path, configuration_path, *script_paths]}
    model = CharacterModel.load(model_path)
    records = []
    for name in ['ajimee', 'idioms', 'nuance', 'daily']:
        data = args.baseline / ('data-' + name + '.json')
        before = args.baseline / 'mozc-no-context' / (name + '.json')
        frozen[str(data.resolve())] = digest(data)
        frozen[str(before.resolve())] = digest(before)
        out = args.out / name
        command = [sys.executable, '-X', 'utf8', str(Path(__file__).with_name('collect.py')),
                   '--exe', str(args.exe), '--source-commit', args.source_commit, '--data', str(data), '--out', str(out)]
        proc = subprocess.run(command, capture_output=True, timeout=150)
        (args.out / (name+'.log')).write_bytes(proc.stdout + proc.stderr)
        if proc.returncode: raise RuntimeError(name + ' native collection failed')
        observations = [json.loads(line) for line in (out/'observations.jsonl').read_text(encoding='utf-8').splitlines()]
        old = json.loads(before.read_text(encoding='utf-8'))
        if len(observations) != len(old): raise ValueError('Comparison count mismatch')
        scored = score_rows(model, observations)
        result = []
        for row, reference in zip(scored, old):
            if row['index'] != str(reference['index']) or row['top1'] != reference['top1'] or row['references'] != reference['expected']:
                raise ValueError('Trace-only source changed baseline or case identity: '+row['index'])
            choice = ranked_choice(row, chosen['weight'], chosen['margin'])
            result.append(dict(index=row['index'], input=row['reading'], before=row['top1'], after=choice,
                               references=row['references'], old_ok=row['top1'] in row['references'],
                               new_ok=choice in row['references'], lattice_hit=any(c['text'] in row['references'] for c in row['candidates'])))
        (args.out / (name + '-results.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
        record = dict(set=name, cases=len(result), old_hit=sum(r['old_ok'] for r in result), new_hit=sum(r['new_ok'] for r in result),
                      improved=sum(not r['old_ok'] and r['new_ok'] for r in result), regressed=sum(r['old_ok'] and not r['new_ok'] for r in result),
                      changed=sum(r['before'] != r['after'] for r in result), lattice_reference_hit=sum(r['lattice_hit'] for r in result))
        records.append(record)
        (args.out/'partial.json').write_text(json.dumps(records, indent=2), encoding='utf-8')
        print(json.dumps(record), flush=True)
    if frozen != {p: digest(p) for p in frozen}:
        raise RuntimeError('Experiment artifacts changed')
    report = dict(configuration=chosen, identities=frozen, source_commit=args.source_commit,
                  product_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                  results=records, offline_only=True, native_tsf=False, product_adopted=False)
    (args.out/'summary.json').write_text(json.dumps(report, indent=2), encoding='utf-8')


if __name__ == '__main__': main()
