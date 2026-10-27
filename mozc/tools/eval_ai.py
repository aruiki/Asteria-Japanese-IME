"""評価の道具(session_eval・learning_flow・session_soak)で共通の、AI のモデルの場所と動作の確認(v3.2.0)。

2026-10-27 に、評価に使っていたモデルのフォルダ(short-word-package-check-v7)が作業の途中で消え、その後の評価が
AI なしで動いた(結果は Mozc だけのもの)。同じことが起きても気づけるように:
  - モデルの場所は、残っている候補のうち、必要なモデルがそろった最初のものにする(なければ止める)。
  - サーバーを起動したら、プロファイルの kotori_ai_status.txt が state=llm(GPU で LLM まで動いている)になるまで
    待つ。ならなければ止める(AI なしの結果を数えない)。CPU の評価など、別の状態を許すときは allow で指定する。
"""
from pathlib import Path
import time

P = Path(__file__).resolve().parents[2]
REQUIRED = ("zenz-v2.5-small-q8_0.gguf", "tinyswallow-1.5b-q5_k_m.gguf")
CANDIDATES = (
    P / "target/accuracy-guide/eval-models",
    P / "target/accuracy-guide/v310-ci-msi-6459/extract/PFiles/Asteria",
    P / "target/accuracy-guide/short-word-package-check-v7/payload/ext/PFiles/Kotori",
)


def default_models():
    """必要なモデルがそろった最初の場所。どこにもなければ空文字(呼んだ側で止める)。"""
    for c in CANDIDATES:
        if all((c / f).is_file() for f in REQUIRED):
            return str(c)
    return ""


def check_models(path):
    missing = [f for f in REQUIRED if not (Path(path) / f).is_file()]
    if not path or missing:
        raise SystemExit(f"AI のモデルがない: {path or '(候補なし)'} に {', '.join(missing or REQUIRED)} がない。"
                         " --models で指定する")


def ai_state(profile):
    f = Path(profile) / "kotori_ai_status.txt"
    try:
        for line in f.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("state="):
                return line.split("=", 1)[1].strip()
    except OSError:
        pass
    return ""


def wait_ai(profile, poke, allow=("llm",), timeout=90):
    """AI が動くまで待つ。poke() は読み込みを促す入力を送る関数。動かなければ止める。"""
    deadline = time.monotonic() + timeout
    state = ""
    while time.monotonic() < deadline:
        state = ai_state(profile)
        if state in allow:
            return state
        poke()
        time.sleep(1)
    raise SystemExit(f"AI が動いていない(kotori_ai_status の state={state or '(なし)'})。"
                     " AI なしの結果は数えないので止める")
