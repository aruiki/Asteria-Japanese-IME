# Asteria のタスクランナー(docs/SPEC.md 17.1)

default:
    @just --list

# PR 前に通す全ゲート(17.4-4)。ゴールデン・縮小評価・ベンチは M1 以降で追加する
ci: fmt-check clippy test

# ci に加えて MSRV(1.80)と Windows 向け clippy(Linux のとき)を確かめる。Rust を変えたら PR の前に通す
check: ci
    cargo +1.80 check --workspace --all-targets --locked
    if [ "{{os()}}" != "windows" ]; then ASTERIA_LM_NO_NATIVE=1 cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings; fi

# (Windows)Rust 版 TSF TIP (asteria-tip-win) を x64・x86 でビルドし、単体テストを走らせる
tip:
    RUSTFLAGS="-C target-feature=+crt-static" cargo build -p asteria-tip-win --release --locked --target x86_64-pc-windows-msvc
    RUSTFLAGS="-C target-feature=+crt-static" cargo build -p asteria-tip-win --release --locked --target i686-pc-windows-msvc
    cargo test -p asteria-tip-win --locked --target x86_64-pc-windows-msvc
    cargo test -p asteria-tip-win --locked --target i686-pc-windows-msvc

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets --locked -- -D warnings

test:
    cargo test --workspace --locked

# 辞書ソース(Mozc dictionary_oss)を取得し、SHA-256 を検証する(5.3)
fetch-dict:
    bash data/dict-src/fetch.sh

# システム辞書を作る(5.3、REQ-5-5)。出力は target/asteria/system.dict
dict: fetch-dict
    cargo run --release --locked -p asteria-dictc -- data/dict-src/mozc target/asteria/system.dict

# 配布物に同梱する THIRD_PARTY_NOTICES を作る(16.1、REQ-16-1)。cargo-about が要る
notices: fetch-dict
    bash frontends/windows/notices.sh target/asteria/THIRD_PARTY_NOTICES.txt

# 読みを入れて変換結果と候補を見る(M1)。先に just dict で辞書を作る
repl:
    cargo run --release --locked -p asteria-ime-eval -- repl target/asteria/system.dict

# 評価セットを取得して一括評価する(14.1、REQ-14-1)。結果は eval/results/ に JSON と Markdown で出る
eval: dict
    bash eval/fetch.sh
    cargo run --release --locked -p asteria-ime-eval -- run target/asteria/system.dict eval/results ajimee-bench=eval/data/ajimee-bench.json

# 上位候補を zenz-v2.5-small でリランクして評価する(6.2)。先に just zenz でモデルを用意する
eval-lm: dict
    bash eval/fetch.sh
    cargo run --release --locked -p asteria-ime-eval -- run --lm target/asteria/zenz-v2.5-small-f16.gguf target/asteria/system.dict eval/results-lm ajimee-bench=eval/data/ajimee-bench.json

# REQ-6-1 の条件(読み 20 文字、左文脈 64 文字、K=16)でリランクの遅延を測る(13.2)。先に just zenz でモデルを用意する
bench-lm: dict
    bash eval/fetch.sh
    cargo run --release --locked -p asteria-ime-eval -- bench-lm target/asteria/system.dict target/asteria/zenz-v2.5-small-f16.gguf eval/data/ajimee-bench.json

# zenz-v2.5-small を取得して GGUF に変換する(6.1、6.4、docs/adr/0007)。出力は target/asteria/zenz-v2.5-small-f16.gguf。
# 先に third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt を pip で入れておく
zenz python="python3":
    bash training/zenz/fetch.sh
    mkdir -p target/asteria
    {{python}} training/zenz/convert.py training/zenz/models/zenz-v2.5-small target/asteria/zenz-v2.5-small-f16.gguf --outtype f16
