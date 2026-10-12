"""固定したMozc原典と業務文テンプレートから大量入力ケースを再生成する。

データは出力先だけに置く。自然文には意味の正解を捏造しない。
文章数と同じ文章の操作条件数は別に数える。heldoutは読み込まない。
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import random
import re


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def hiragana(s):
    return ''.join(chr(ord(c)-0x60) if 'ァ' <= c <= 'ヶ' else c for c in s)


def roman_table(path):
    inverse = {}
    for line in Path(path).read_text(encoding='utf-8').splitlines():
        p = line.split('\t')
        if len(p)<2 or (len(p)>2 and p[2]) or not p[1]:
            continue
        key, val = p[:2]
        if not key.isascii() or any(c.isspace() for c in key):
            continue
        if val not in inverse or (len(key), key)<(len(inverse[val]),inverse[val]):
            inverse[val] = key
    inverse.update({'ん': "n'", 'っ':'xtu', 'ー':'-', '？':'?', '！':'!', '（':'(', '）':')'})
    return inverse


def romanize(reading, inverse):
    surfaces = sorted(inverse,key=lambda x:(-len(x),x))
    keys=[]; at=0
    while at<len(reading):
        surface=next((s for s in surfaces if reading.startswith(s,at)),None)
        if surface:
            keys.append(inverse[surface]); at+=len(surface)
        elif reading[at].isascii() and reading[at].isdigit():
            keys.append(reading[at]); at+=1
        else:
            raise ValueError('unsupported character: '+repr(reading[at:at+8]))
    return ''.join(keys)


# 意味のある実務上の対象/属性。単なるランダムかなではない。
BUSINESS = [
 ('ふぁいる','ファイル','なまえ','名前'),('ふぉるだー','フォルダー','ばしょ','場所'),
 ('しりょう','資料','ないよう','内容'),('せいきゅうしょ','請求書','きんがく','金額'),
 ('みつもりしょ','見積書','きげん','期限'),('けいやくしょ','契約書','じょうけん','条件'),
 ('ぎじろく','議事録','ないよう','内容'),('もうしこみしょ','申込書','なまえ','名前'),
 ('よやく','予約','にちじ','日時'),('かいぎ','会議','にってい','日程'),
 ('めーる','メール','あてさき','宛先'),('れんらくさき','連絡先','でんわばんごう','電話番号'),
 ('おきゃくさま','お客様','じゅうしょ','住所'),('しゃいん','社員','しめい','氏名'),
 ('あかうんと','アカウント','せってい','設定'),('ぱすわーど','パスワード','こうしんび','更新日'),
 ('さーばー','サーバー','せってい','設定'),('ねっとわーく','ネットワーク','じょうたい','状態'),
 ('あぷり','アプリ','ばーじょん','バージョン'),('ぶらうざー','ブラウザー','せってい','設定'),
 ('でーたべーす','データベース','なまえ','名前'),('ばっくあっぷ','バックアップ','ほぞんさき','保存先'),
 ('しゃしん','写真','ほぞんさき','保存先'),('どうが','動画','さいず','サイズ'),
 ('ぷろじぇくと','プロジェクト','しんちょく','進捗'),('さぎょう','作業','てじゅん','手順'),
 ('しょうひん','商品','ざいこ','在庫'),('ちゅうもん','注文','すうりょう','数量'),
 ('はっそう','発送','よていび','予定日'),('のうひん','納品','きげん','期限'),
 ('そうきん','送金','きんがく','金額'),('しはらい','支払い','きげん','期限'),
 ('けいひ','経費','ないよう','内容'),('こうつうひ','交通費','きんがく','金額'),
 ('しゅっちょう','出張','にってい','日程'),('ほうこくしょ','報告書','ないよう','内容'),
 ('しすてむ','システム','せってい','設定'),('えらー','エラー','ないよう','内容'),
 ('いんすとーらー','インストーラー','ばーじょん','バージョン'),('にほんごにゅうりょく','日本語入力','せってい','設定')]
PREFIX=[('まず','まず'),('つぎに','次に'),('さいごに','最後に'),('ねんのため','念のため'),
        ('さぎょうまえに','作業前に'),('さぎょうごに','作業後に'),('かいぎのまえに','会議の前に'),('もういちど','もう一度')]
ACTION=[('かくにんしてください','確認してください'),('かくにんしました','確認しました'),
        ('へんこうしてください','変更してください'),('へんこうしました','変更しました'),
        ('とうろくしてください','登録してください'),('とうろくしました','登録しました'),
        ('こうしんしてください','更新してください'),('こうしんしました','更新しました')]


def generate(mozc, repo, book_limit=8000, name_limit=3000, seed=20261012):
    table=mozc/'data/preedit/romanji-hiragana.tsv'; inverse=roman_table(table)
    cases=[]; excluded=collections.Counter(); seen=set(); sources={}
    def add(reading, family, expected=None, predicates=None, preceding='', extra=None):
        reading=hiragana(reading)
        if not reading or len(reading)>160:
            excluded['empty_or_over_160']+=1; return
        try: keys=romanize(reading,inverse)
        except ValueError:
            excluded['unsupported_for_romaji']+=1; return
        if reading in seen:
            excluded['duplicate_reading']+=1; return
        seen.add(reading)
        row={'id':hashlib.sha256((family+'\0'+reading).encode()).hexdigest()[:20],
             'family':family,'reading':reading,'romaji':keys,'expected':expected or [],
             'predicates':predicates or [],'preceding':preceding}
        if extra: row.update(extra)
        cases.append(row)
    # 既存の調整セットのみ。最終評価用heldoutには触らない。
    for name in ['daily','idioms','nuance','notation','typo','homophone']:
        p=repo/f'eval/sets/kotori-{name}.json'; sources[str(p)]=digest(p)
        for r in json.loads(p.read_text(encoding='utf-8')):
            add(r['input'],'curated-'+name,r['expected_output'],preceding=r.get('context_text',''))
    for nr,nv,pr,pv in BUSINESS:
        for xr,xv in PREFIX:
            for ar,av in ACTION:
                add(xr+nr+'の'+pr+'を'+ar,'business', [xv+nv+'の'+pv+'を'+av])
    # 上流の不具合例。全文1位のMatch/Not Matchは意味判定、候補順位は診断用。
    for name in ['oss.tsv','regression.tsv']:
        p=mozc/'data/test/quality_regression_test'/name; sources[str(p)]=digest(p)
        grouped={}
        for line in p.read_text(encoding='utf-8').splitlines():
            if not line or line.startswith('#'): continue
            parts=line.split('\t')
            if len(parts)<4: continue
            label,reading,value,command=parts[:4]
            grouped.setdefault(reading,[]).append({'label':label,'value':value,'command':command})
        for reading,predicates in grouped.items(): add(reading,'upstream-regression',predicates=predicates)
    p=mozc/'data/test/quality_regression_test/myoji.tsv'; sources[str(p)]=digest(p)
    names=collections.defaultdict(list)
    for line in p.read_text(encoding='utf-8').splitlines():
        if not line or line.startswith('#'): continue
        parts=line.split('\t')
        if len(parts)>=4: names[parts[1]].append(parts[2])
    rng=random.Random(seed); keys=sorted(names); rng.shuffle(keys)
    for reading in keys[:name_limit]: add(reading,'surname',extra={'surname_targets':names[reading]})
    p=mozc/'data/test/stress_test/sentences.txt'; sources[str(p)]=digest(p)
    readings=sorted({line for line in p.read_text(encoding='utf-8').splitlines() if line and not line.startswith('#')})
    rng.shuffle(readings)
    for reading in readings[:book_limit]: add(reading,'natural-reading')
    # 原典とテンプレート由来を区別し、単なる操作回数を文章数に混ぜない。
    return cases,{'seed':seed,'source_sha256':sources,'romanji_table_sha256':digest(table),
                 'unique_inputs':len(cases),'families':dict(collections.Counter(r['family'] for r in cases)),
                 'excluded':dict(excluded),'meaning_oracle_cases':sum(bool(r['expected']) or any(p['command'] in ['Conversion Match','Conversion Not Match'] for p in r['predicates']) for r in cases),
                 'business_template_families':len(BUSINESS),'native_tsf':False}


def cpu_subset(cases, seed=20261013):
    """既存のCPU検査と同じ層化抽出。元の配列/ケースは変更しない。"""
    groups = collections.defaultdict(list)
    for case in cases:
        groups[case['family']].append(case)
    rng = random.Random(seed)
    result = []
    limits = {'business': 200, 'upstream-regression': 100,
              'surname': 150, 'natural-reading': 287}
    for family, group in groups.items():
        rng.shuffle(group)
        result.extend(group[:limits.get(family, len(group))])
    return result


def main():
    ap=argparse.ArgumentParser(description=__doc__); ap.add_argument('--mozc',type=Path,required=True)
    ap.add_argument('--repo',type=Path,default=Path.cwd()); ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--cpu-out',type=Path,help='同じ原票からCPU層化サブセットも保存する')
    ap.add_argument('--book-limit',type=int,default=8000); ap.add_argument('--name-limit',type=int,default=3000)
    args=ap.parse_args(); cases,manifest=generate(args.mozc,args.repo,args.book_limit,args.name_limit)
    args.out.parent.mkdir(parents=True,exist_ok=True)
    args.out.write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in cases),encoding='utf-8')
    manifest['cases_sha256']=digest(args.out)
    args.out.with_suffix('.manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
    if args.cpu_out:
        args.cpu_out.parent.mkdir(parents=True,exist_ok=True)
        subset = cpu_subset(cases)
        args.cpu_out.write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in subset),encoding='utf-8')
        args.cpu_out.with_suffix('.manifest.json').write_text(json.dumps({
            'source_cases_sha256':manifest['cases_sha256'], 'seed':20261013,
            'cases_sha256':digest(args.cpu_out), 'unique_inputs':len(subset),
            'families':dict(collections.Counter(r['family'] for r in subset))},
            ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(manifest,ensure_ascii=False,indent=2))


if __name__=='__main__': main()