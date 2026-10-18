"""同じ読みの正例/辞書候補の対比でzenzを学習する。モデルは実験出力のみ。

NLLで元の変換能力を維持しつつ、対比損失で不自然な同音語を低く採点する。
区間devはモデル選択用。IMEの精度・実入力の検証を置き換えない。
"""
import argparse
import hashlib
import json
import random
import shutil
import time
from pathlib import Path

import torch
import torch.nn.functional as F
from peft import LoraConfig, get_peft_model
from transformers import AutoConfig, AutoModelForCausalLM, AutoTokenizer


def prompt(reading, context):
    reading = ''.join(chr(ord(c)+0x60) if 'ぁ' <= c <= 'ゖ' else c for c in reading)
    normalize = lambda s: s.replace('\n', '').replace('\r', '').replace(' ', '\u3000')
    context = normalize(context)[-40:]
    return '\uee00'+normalize(reading)+ ('\uee02'+context if context else '')+'\uee01'


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--model', type=Path, required=True)
    ap.add_argument('--pairs', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--steps', type=int, default=600)
    ap.add_argument('--task', choices=['zenz','semantic'], default='zenz')
    ap.add_argument('--batch', type=int, default=16)
    ap.add_argument('--lr', type=float, default=0.0002)
    ap.add_argument('--margin', type=float, default=2.0)
    ap.add_argument('--rank-weight', type=float, default=0.25)
    ap.add_argument('--joint-llm-weight', type=float, default=0.0,
                    help='Frozen LLM offset for observed native candidate pairs only')
    ap.add_argument('--seed', type=int, default=20261016)
    ap.add_argument('--eval-every', type=int, default=100)
    ap.add_argument('--dev-limit', type=int, default=512)
    args = ap.parse_args()
    if not torch.cuda.is_available():
        raise RuntimeError('This experiment requires CUDA; no silent CPU training')
    if args.out.exists() and any(args.out.iterdir()):
        raise FileExistsError('Keep prior training outputs unchanged; choose a new output directory')
    args.out.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).read_bytes()
    source_sha = hashlib.sha256(source).hexdigest()
    (args.out/'training-source.py').write_bytes(source)
    random.seed(args.seed); torch.manual_seed(args.seed); torch.cuda.manual_seed_all(args.seed)
    torch.set_num_threads(4)
    torch.backends.cuda.matmul.allow_tf32 = True
    dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
    data_manifest = json.loads(args.pairs.with_suffix('.manifest.json').read_text(encoding='utf-8'))
    if data_manifest['pairs_sha256'] != hashlib.sha256(args.pairs.read_bytes()).hexdigest():
        raise ValueError('Dataset manifest SHA differs')
    excluded_names = {Path(x['file']).name for x in data_manifest['excluded_evaluations']}
    if not {'ajimee-bench.json', 'kotori-heldout.json'} <= excluded_names:
        raise ValueError('AJIMEE and independent evaluation must be excluded before training')
    with args.pairs.open(encoding='utf-8') as pair_file:
        rows = [json.loads(line) for line in pair_file]
    train = [r for r in rows if r['split']=='train']; dev = [r for r in rows if r['split']=='dev']
    if not train or not dev:
        raise ValueError('Both independent corpus groups are required')
    if {r['reading'] for r in train} & {r['reading'] for r in dev}:
        raise ValueError('Train/dev reading leakage')
    def category(row):
        kind=row.get('kind','original')
        return 'contrast' if 'contrast' in kind else 'agreement' if 'agreement' in kind else 'original'
    rng = random.Random(args.seed)
    groups={kind:[r for r in dev if category(r)==kind] for kind in ('original','agreement','contrast')}
    groups={k:v for k,v in groups.items() if v}
    dev=[]
    for values in groups.values():
        rng.shuffle(values);dev.extend(values[:max(1,args.dev_limit//len(groups))])
    dev_kinds=[category(r) for r in dev]
    train_kinds=[category(r) for r in train]
    train_offsets=[args.joint_llm_weight*r.get('frozen_llm_score_difference',0.) for r in train]
    dev_offsets=[args.joint_llm_weight*r.get('frozen_llm_score_difference',0.) for r in dev]

    tokenizer = AutoTokenizer.from_pretrained(args.model, local_files_only=True)
    config = AutoConfig.from_pretrained(args.model, local_files_only=True)
    # 元configのIDは語彙とずれるため、既存GGUF変換と同じくトークン名で確定する。
    semantic = args.task == 'semantic'
    if semantic:
        eos=tokenizer.eos_token_id;pad=tokenizer.pad_token_id or eos
        yes=tokenizer.convert_tokens_to_ids('yes');no=tokenizer.convert_tokens_to_ids('no')
        if tokenizer.encode('yes',add_special_tokens=False)!=[yes] or tokenizer.encode('no',add_special_tokens=False)!=[no]:
            raise ValueError('The reranker needs single yes/no tokens')
        targets=['q_proj','k_proj','v_proj','o_proj','gate_proj','up_proj','down_proj']
    else:
        eos=tokenizer.convert_tokens_to_ids('</s>');pad=tokenizer.convert_tokens_to_ids('[PAD]')
        config.eos_token_id=eos;config.bos_token_id=tokenizer.convert_tokens_to_ids('<s>');config.pad_token_id=pad
        config.attn_pdrop=config.embd_pdrop=config.resid_pdrop=0.0
        targets=['c_attn','c_proj','c_fc']
    model=AutoModelForCausalLM.from_pretrained(args.model,config=config,local_files_only=True,attn_implementation='sdpa',dtype=dtype if semantic else torch.float32).cuda()
    model=get_peft_model(model,LoraConfig(r=16,lora_alpha=32,lora_dropout=0.0,target_modules=targets,fan_in_fan_out=not semantic,task_type='CAUSAL_LM'))
    model.config.use_cache = False
    model.print_trainable_parameters()
    # 一度だけトークン化し、prefixと出力部分を明示的に分離する。
    def encode(row):
        values=[]
        if semantic:
            before='<|im_start|>system\nJudge whether the Document meets the requirements based on the Query and the Instruct provided. Note that the answer can only be "yes" or "no".<|im_end|>\n<|im_start|>user\n'
            after='<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n'
            instruction='Given a Japanese hiragana reading and preceding context, assess whether the document is a correct kana-kanji conversion. It must preserve the reading while using grammatically correct and semantically natural Japanese. Resolve homophones using the meaning of the entire sentence and the preceding context.'
            for name in ('positive','negative'):
                body='<Instruct>: '+instruction+'\n<Query>: Reading: '+row['reading']+'\nPreceding context: '+row['context']+'\n<Document>: '+row[name]
                body=body.replace('<|','＜|')
                tokens=tokenizer.encode(before+body+after,add_special_tokens=False)
                if not tokens or len(tokens)>512:raise ValueError('Refuse silent semantic-input truncation')
                values.append((tokens,len(tokens)))
        else:
            prefix=tokenizer.encode(prompt(row['reading'],row['context']),add_special_tokens=False)
            for name in ('positive','negative'):
                target=tokenizer.encode(row[name].replace(' ','\u3000'),add_special_tokens=False)+[eos]
                if not prefix or not target or len(prefix)+len(target)>256:
                    raise ValueError('Unexpected token span; refuse silent truncation')
                values.append((prefix+target,len(prefix)))
        return values
    train = [encode(r) for r in train]; dev = [encode(r) for r in dev]
    def scores(batch):
        values = [x[0] for x in batch]+[x[1] for x in batch]
        width = max(len(tokens) for tokens,_ in values)
        ids = torch.full((len(values),width), pad, dtype=torch.long, device='cuda')
        attention = torch.zeros_like(ids); mask = torch.zeros_like(ids,dtype=torch.bool)
        for i,(tokens,start) in enumerate(values):
            if semantic:
                ids[i,-len(tokens):]=torch.tensor(tokens,device='cuda');attention[i,-len(tokens):]=1
            else:
                ids[i,:len(tokens)]=torch.tensor(tokens,device='cuda')
                attention[i,:len(tokens)]=1;mask[i,start:len(tokens)]=True
        with torch.autocast('cuda',dtype=dtype):
            if semantic:
                positions=(attention.cumsum(-1)-1).masked_fill(attention==0,0)
                logits=model(input_ids=ids,attention_mask=attention,position_ids=positions,logits_to_keep=1).logits[:,-1,:].float()
                totals=logits[:,yes]-logits[:,no]
                return totals[:len(batch)],totals[len(batch):],torch.ones(len(batch),device='cuda')
            logits=model(input_ids=ids,attention_mask=attention).logits[:,:-1,:]
        logp = F.log_softmax(logits.float(),dim=-1).gather(-1,ids[:,1:,None]).squeeze(-1)
        mask = mask[:,1:]
        totals = (logp*mask).sum(-1); lengths = mask.sum(-1)
        return totals[:len(batch)], totals[len(batch):], lengths[:len(batch)]
    def evaluate():
        model.eval(); hit, count, nll, gap = 0, 0, 0., 0.
        counts={k:[0,0] for k in set(dev_kinds)}
        with torch.no_grad():
            for at in range(0,len(dev),args.batch):
                batch=dev[at:at+args.batch]; pos,neg,length=scores(batch)
                offset=torch.tensor(dev_offsets[at:at+len(batch)],device='cuda')
                joint_gap=pos-neg+offset
                correct=(joint_gap>0).tolist()
                for i,ok in enumerate(correct):
                    counts[dev_kinds[at+i]][0]+=int(ok);counts[dev_kinds[at+i]][1]+=1
                hit += sum(correct); count += len(batch)
                loss=(F.softplus(-pos)+F.softplus(neg))/2 if semantic else -pos/length
                nll+=loss.sum().item();gap+=joint_gap.sum().item()
        model.train()
        return {'pair_correct':hit,'pairs':count,'accuracy':hit/count,'balanced_accuracy':sum(v[0]/v[1] for v in counts.values())/len(counts),'by_kind':counts,'nll':nll/count,'score_gap':gap/count}
    log=[]; baseline=evaluate(); print('baseline',json.dumps(baseline),flush=True)
    best = baseline['balanced_accuracy'];best_nll=baseline['nll'];best_step=0
    # baseline adapterも保存し、悪化した学習を採用しない。
    best_dir=args.out/'best-adapter'; model.save_pretrained(best_dir)
    optimizer=torch.optim.AdamW((p for p in model.parameters() if p.requires_grad),lr=args.lr,weight_decay=0.01)
    scaler=torch.amp.GradScaler('cuda',enabled=dtype==torch.float16)
    started=time.monotonic()
    training_groups={k:[i for i,v in enumerate(train_kinds) if v==k] for k in set(train_kinds)}
    model.train()
    for step in range(1,args.steps+1):
        if 'contrast' in training_groups:
            half=args.batch//2;quarter=(args.batch-half)//2
            ids=rng.choices(training_groups['contrast'],k=half)
            ids+=rng.choices(training_groups.get('agreement',training_groups['original']),k=quarter)
            ids+=rng.choices(training_groups['original'],k=args.batch-len(ids))
        else:
            ids=rng.choices(list(range(len(train))),k=args.batch)
        batch=[train[i] for i in ids]
        optimizer.zero_grad(set_to_none=True)
        pos,neg,length=scores(batch)
        nll=((F.softplus(-pos)+F.softplus(neg))/2).mean() if semantic else (-pos/length).mean()
        offset=torch.tensor([train_offsets[i] for i in ids],device='cuda')
        rank=F.softplus((neg-pos-offset+args.margin)/2.).mean()*2.
        loss=nll+args.rank_weight*rank
        if not torch.isfinite(loss):
            raise FloatingPointError('Non-finite contrastive training loss')
        warm=min(1.,step/50.); decay=max(0.1,1.-step/args.steps)
        for group in optimizer.param_groups:group['lr']=args.lr*warm*decay
        scaler.scale(loss).backward();scaler.unscale_(optimizer)
        torch.nn.utils.clip_grad_norm_((p for p in model.parameters() if p.requires_grad),1.)
        scaler.step(optimizer);scaler.update()
        if step%25==0:
            print('step',step,'loss',round(loss.item(),4),'nll',round(nll.item(),4),'rank',round(rank.item(),4),'seconds',round(time.monotonic()-started,1),flush=True)
        if step%args.eval_every==0 or step==args.steps:
            result={'step':step,**evaluate()};log.append(result);print('dev',json.dumps(result),flush=True)
            # Keep ordinary-text pair accuracy while improving native preferences.
            preserved = all(result['by_kind'][k][0] >= counts[0]
                            for k, counts in baseline['by_kind'].items())
            if preserved and (result['balanced_accuracy']>best or (result['balanced_accuracy']==best and result['nll']<best_nll)):
                best=result['balanced_accuracy'];best_nll=result['nll'];best_step=step;model.save_pretrained(best_dir)
            (args.out/'metrics.json').write_text(json.dumps({'baseline':baseline,'evaluations':log,'best_step':best_step},indent=2)+'\n',encoding='utf-8')
    receipt={'args':{k:str(v) if isinstance(v,Path) else v for k,v in vars(args).items()},'pairs_sha256':hashlib.sha256(args.pairs.read_bytes()).hexdigest(),'base_weights_sha256':hashlib.sha256((args.model/'model.safetensors').read_bytes()).hexdigest(),'baseline':baseline,'best_selection_score':best,'best_step':best_step,'torch':torch.__version__,'cuda':torch.version.cuda,'gpu':torch.cuda.get_device_name(0),'training_source_sha256':source_sha,'adopted_for_product':False}
    (args.out/'training-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print('complete',json.dumps(receipt),flush=True)

if __name__ == '__main__':
    main()
