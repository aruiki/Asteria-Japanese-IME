#!/usr/bin/env python3
"""genmai-310M(Hugging Face の safetensors)を Asteria 用の GGUF にする(docs/adr/0085)。

使い方:
    python training/genmai/convert.py <fetch.sh の出力ディレクトリ> <出力 .gguf> [--outtype q8_0]

training/zenz/convert.py と同じく llama.cpp の変換(third_party/llama.cpp、コミット固定)をそのまま使い、
次だけを補う。

1. genmai は tokenizer.json しか持たないので、vocab.json と merges.txt をそこから書き出す。
   語彙は zenz-v2.5 と同じ 1 文字ずつの 6000 語。取り違えを防ぐため、tokenizer.json が固定した
   genmai-310M のものと一致するときだけ変換する。
2. 前処理(pre-tokenizer)は上流に登録がないので、zenz と同じく GPT-2 の正規表現として書く。
3. 特殊トークンの ID は tokenizer_config.json の名前から引き直す(終端は </s>)。
4. メタデータ `kotori.*`(版・プロンプトの形・ライセンス)を書き込む。genmai は左文脈を読みの前に置く
   形(zenzlc: <s> U+EE02 左文脈 U+EE00 読み U+EE01)で学習している。
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LLAMA = ROOT / "third_party" / "llama.cpp"
sys.path.insert(0, str(LLAMA / "gguf-py"))
sys.path.insert(0, str(LLAMA))

import convert_hf_to_gguf  # noqa: E402
import gguf  # noqa: E402
from conversion import base  # noqa: E402

# genmai-310M のリビジョン 143e574c の tokenizer.json(training/genmai/genmai-310M.sha256)。
TOKENIZER_SHA256 = "5f47a842d17ec6211bf243fc2f32b6ee12c9cbc00e68f9bbfdfd550b584c1838"
REVISION = "143e574c6a25f545770461cc42e3c48baa89bc75"
LICENSE = (
    "CC-BY-SA-4.0; genmai-310M (subdiox), fine-tuned from zenz-v2.5-medium (c) Keita Miwa, "
    "based on ku-nlp/gpt2-medium-japanese-char (CC-BY-SA-4.0)"
)


def main() -> None:
    model_dir = Path(sys.argv[1])
    tokenizer_path = model_dir / "tokenizer.json"
    if hashlib.sha256(tokenizer_path.read_bytes()).hexdigest() != TOKENIZER_SHA256:
        sys.exit("tokenizer.json が固定した genmai-310M のものと一致しない")
    tokenizer = json.loads(tokenizer_path.read_text(encoding="utf-8"))
    vocab = tokenizer["model"]["vocab"]
    merges = tokenizer["model"]["merges"]
    (model_dir / "vocab.json").write_text(json.dumps(vocab, ensure_ascii=False), encoding="utf-8")
    lines = ["#version: 0.2"] + [m if isinstance(m, str) else " ".join(m) for m in merges]
    (model_dir / "merges.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    config = json.loads((model_dir / "tokenizer_config.json").read_text(encoding="utf-8"))
    special = {typ: vocab[config[f"{typ}_token"]] for typ in ("bos", "eos", "unk", "pad")}

    original_load = gguf.SpecialVocab._load

    def load_special(self, path: Path) -> None:
        original_load(self, path)
        self.special_token_ids.update(special)

    gguf.SpecialVocab._load = load_special

    original_pre = base.TextModel.get_vocab_base_pre

    def get_vocab_base_pre(self, tokenizer) -> str:
        try:
            return original_pre(self, tokenizer)
        except NotImplementedError:
            return "gpt-2"

    original_prepare = base.TextModel.prepare_metadata

    def prepare_metadata(self, vocab_only: bool) -> None:
        original_prepare(self, vocab_only)
        w = self.gguf_writer
        w.add_string("kotori.model_version", f"genmai-310M@{REVISION}")
        w.add_string("kotori.prompt_format", "zenzlc")
        w.add_string("kotori.license", LICENSE)

    base.TextModel.get_vocab_base_pre = get_vocab_base_pre
    base.TextModel.prepare_metadata = prepare_metadata

    sys.argv = ["convert_hf_to_gguf.py", str(model_dir), "--outfile", sys.argv[2], *sys.argv[3:]]
    convert_hf_to_gguf.main()


if __name__ == "__main__":
    main()
