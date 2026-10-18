"""別取得区間の較正候補だけで、初期統計順位の係数を固定する。"""
import argparse
import json
from pathlib import Path

from model import CharacterModel, MAX_COST_DELTA, select_configuration
from prepare import digest


def score_rows(model, observations):
    scored = []
    for row in observations:
        current = row['top1']
        base_cost = min(c['cost'] for c in row['candidates'])
        values = {}
        for c in row['candidates']:
            delta = c['cost'] - base_cost
            if delta <= MAX_COST_DELTA:
                values[c['text']] = min(values.get(c['text'], delta), delta)
        # 表示中の答えを0差分の対照にする。連語/履歴で上がった答えを
        # ラティスの元順位だけで戻す実験にはしない。
        values[current] = 0
        result = [dict(text=value, cost_delta=delta,
                       stat_cost=model.cost(value, len(row['reading']))) for value, delta in values.items()]
        scored.append(dict(row, scored=result))
    return scored


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--model', type=Path, required=True)
    ap.add_argument('--calibration', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    manifest = json.loads((args.model / 'manifest.json').read_text(encoding='utf-8'))
    observed_manifest = json.loads((args.calibration / 'manifest.json').read_text(encoding='utf-8'))
    model_path = args.model / 'character-model.json.gz'
    observations_path = args.calibration / 'observations.jsonl'
    if (digest(model_path) != manifest['model_sha256'] or
        digest(observations_path) != observed_manifest['observations_sha256'] or
        len(observed_manifest['identities']) < 2 or
        manifest['calibration_sha256'] not in observed_manifest['identities'].values()):
        raise ValueError('Calibration/model identity mismatch')
    model = CharacterModel.load(model_path)
    observations = [json.loads(line) for line in observations_path.read_text(encoding='utf-8').splitlines()]
    if len(observations) != 256:
        raise ValueError('Calibration count differs from frozen plan')
    scored = score_rows(model, observations)
    chosen, configs = select_configuration(scored)
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / 'scored.jsonl').write_text(''.join(json.dumps(row, ensure_ascii=False)+'\n' for row in scored), encoding='utf-8', newline='\n')
    report = dict(model_sha256=digest(model_path), calibration_sha256=manifest['calibration_sha256'],
                  observations_sha256=digest(observations_path), calibration_cases=len(scored),
                  chosen=chosen, configs=configs, corpus_only_calibration=True,
                  evaluation_results_used_for_selection=False, product_adopted=False,
                  configuration_script_sha256=digest(__file__))
    (args.out / 'configuration.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report), flush=True)


if __name__ == '__main__': main()
