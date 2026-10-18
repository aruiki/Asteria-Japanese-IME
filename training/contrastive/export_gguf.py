"""既存zenz変換器を再利用し、学習原票のSHAを持つGGUFを作る。"""
import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--model',type=Path,required=True)
    ap.add_argument('--llama-root',type=Path,required=True)
    ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    if args.out.exists():raise FileExistsError('Do not overwrite an existing model export')
    receipt=json.loads((args.model/'merge-receipt.json').read_text(encoding='utf-8'))
    if hashlib.sha256((args.model/'model.safetensors').read_bytes()).hexdigest()!=receipt['merged_weights_sha256']:
        raise ValueError('Merged weights changed')
    sys.path.insert(0,str(args.llama_root/'gguf-py'));sys.path.insert(0,str(args.llama_root))
    if receipt.get('task','zenz')=='semantic':
        import convert_hf_to_gguf
        from conversion import base
        original=base.TextModel.prepare_metadata
        def prepare(self,vocab_only):
            original(self,vocab_only)
            self.gguf_writer.add_string('asteria.semantic_prompt_version','1')
            self.gguf_writer.add_string('asteria.training_receipt_sha256',receipt['training_receipt_sha256'])
        base.TextModel.prepare_metadata=prepare
        sys.argv=['convert_hf_to_gguf.py',str(args.model),'--outfile',str(args.out),'--outtype','f16']
        convert_hf_to_gguf.main()
    else:
        source=Path(__file__).resolve().parents[1]/'zenz/convert.py'
        spec=importlib.util.spec_from_file_location('asteria_base_zenz_export',source)
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        module.REVISIONS[args.model.name]='contrastive-'+receipt['training_receipt_sha256']
        sys.argv=[str(source),str(args.model),str(args.out),'--outtype','f16']
        module.main()
    result={'gguf_sha256':hashlib.sha256(args.out.read_bytes()).hexdigest(),'source_merge_receipt_sha256':hashlib.sha256((args.model/'merge-receipt.json').read_bytes()).hexdigest(),'training_receipt_sha256':receipt['training_receipt_sha256'],'quantization':'F16','adopted_for_product':False}
    args.out.with_suffix('.export.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result),flush=True)

if __name__=='__main__':main()
