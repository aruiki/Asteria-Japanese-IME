"""検証で選んだLoRAだけを元モデルへ統合し、数値的一致と由来を保存する。"""
import argparse
import hashlib
import json
import shutil
from pathlib import Path


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--model',type=Path,required=True)
    ap.add_argument('--training',type=Path,required=True)
    ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    import torch
    from peft import PeftModel
    from transformers import AutoModelForCausalLM,AutoTokenizer
    torch.set_num_threads(4)
    receipt_path=args.training/'training-receipt.json'
    receipt=json.loads(receipt_path.read_text(encoding='utf-8'))
    if receipt['best_step']==0:
        raise ValueError('Training did not select an improved checkpoint')
    if hashlib.sha256((args.model/'model.safetensors').read_bytes()).hexdigest()!=receipt['base_weights_sha256']:
        raise ValueError('Base weights changed')
    if args.out.exists():
        raise FileExistsError('Keep prior exports unchanged; choose a new output directory')
    tok=AutoTokenizer.from_pretrained(args.model,local_files_only=True)
    base=AutoModelForCausalLM.from_pretrained(args.model,local_files_only=True,dtype=torch.float32)
    adapter=PeftModel.from_pretrained(base,args.training/'best-adapter',local_files_only=True).eval()
    control=tok('\uee00キョウノヨテイヲカクニンスル\uee01',add_special_tokens=False,return_tensors='pt')
    with torch.no_grad():before=adapter(**control).logits
    merged=adapter.merge_and_unload(safe_merge=True).eval()
    with torch.no_grad():after=merged(**control).logits
    maximum=(before-after).abs().max().item()
    if not torch.allclose(before,after,atol=0.001,rtol=0.0001):
        raise ValueError('Merged model differs from the selected adapter: '+str(maximum))
    receipt_sha=hashlib.sha256(receipt_path.read_bytes()).hexdigest()
    task=receipt['args'].get('task','zenz')
    if task=='zenz':
        merged.config.bos_token_id=tok.convert_tokens_to_ids('<s>')
        merged.config.eos_token_id=tok.convert_tokens_to_ids('</s>')
        merged.config.pad_token_id=tok.convert_tokens_to_ids('[PAD]')
    merged.config._name_or_path='asteria-semantic-reranker' if task=='semantic' else 'asteria-contrastive-zenz'
    merged.config.asteria_training_manifest_sha256=receipt_sha
    merged.save_pretrained(args.out,safe_serialization=True)
    # トークンIDだけでなく、既存変換器が検証する元ファイルのSHAも維持する。
    for name in ('vocab.json','merges.txt','tokenizer_config.json','special_tokens_map.json','tokenizer.json'):
        if (args.model/name).is_file():shutil.copyfile(args.model/name,args.out/name)
    output={'task':task,'training_receipt_sha256':receipt_sha,'base_weights_sha256':receipt['base_weights_sha256'],'adapter_sha256':hashlib.sha256((args.training/'best-adapter/adapter_model.safetensors').read_bytes()).hexdigest(),'merged_weights_sha256':hashlib.sha256((args.out/'model.safetensors').read_bytes()).hexdigest(),'merge_max_absolute_logit_error':maximum,'best_step':receipt['best_step'],'adopted_for_product':False}
    (args.out/'merge-receipt.json').write_text(json.dumps(output,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(output),flush=True)

if __name__=='__main__':main()
