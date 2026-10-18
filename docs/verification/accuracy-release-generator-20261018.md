# 候補選択モデルの採否と追加モデル同梱（2026-10-18）

新リリース開発のため、既存の候補選択実験を原票・ソースと照合した。候補選択は製品へ採用せず、188/200の追加生成構成をMSIへ接続する。

## 候補選択の実測

同じAJIMEE200問・正解集合・文脈、元の生成モデルとTinySwallow採点を保持し、Qwen3-4Bを僅差の既存候補2件の選択に使用した。A/Bを逆順でも照合する。

| 構成 | 正解 | 改善/改悪 | 出力変更 | モデル処理中央値/p95 |
| --- | --- | --- | --- | --- |
| 選択OFF | 188/200 | 0/0 | 0 | 172/549ms |
| 選択ON | 187/200 | 1/2 | 4 | 199/733ms |
| 漢字置換・保護語・辞書確認付き | 188/200 | 0/0 | 0 | 184/617ms |

保護前は8回の内部選択変更があり、最終出力は4問で変わった。保護後は32回の選択照合を行ったが採用0回で、全200問の最終出力はOFFと一致した。処理時間はconverter内のモデル処理で、実TSFの表示遅延ではない。保護後のC++66試験成功はchoice-style-test.xml。保護条件を緩めて公開しない。

- off: source `028d7985fc195b55d8dbc5538fa2cad80ad3eed5`、converter SHA `c65249a8f5b22aafce4e8e2ff38a0ec8ae1458ff9728f76f2ab207dcc62899a7`、原票 `target/accuracy-guide/ajimee-choice-off-v1/`。
- on: source `028d7985fc195b55d8dbc5538fa2cad80ad3eed5`、converter SHA `c65249a8f5b22aafce4e8e2ff38a0ec8ae1458ff9728f76f2ab207dcc62899a7`、原票 `target/accuracy-guide/ajimee-choice-on-v1/`。
- style: source `a9297890c55841d9c822dfe72b1db9cf681d9cb3`、converter SHA `e3f72e0fc05784654251816352ce10ab3706e36d7e03a5bab7b82f2f41eae551`、原票 `target/accuracy-guide/ajimee-choice-style-v1/`。

不採用コードはMozc枝 `codex/choice-verifier-20261018` に保存。製品ソースは `fb6de1384` から専用枝 `codex/release-generator-bundle-20261018` を作り、選択モデルを加えず同梱処理だけ変更した。

## 追加生成モデルの配布経路

- 追加モデルをBazelの必須入力・WiX Component/Featureへ追加。WiX実行前に名前とSHAを照合する。
- `fetch_generator.py` でビルド時に固定SHAを照合する。既存の不一致ファイルは保持して失敗する。失敗した取得はインストール可能な名前へ移さない。
- `native_generator.json` をMSIへ同梱。モデル、学習原票、データ識別のSHA対応を確認した。
- 派生モデルの帰属・元モデル・変更点・CC BY-SA 4.0をNOTICEと使用許諾へ反映した。
- `check_release.py --require-generator` でMSI展開後の実バイトと、検証ソース側の固定情報を照合する。同梱manifestを信用して通過させない。
- CIへモデル取得、MSI内容検査、関連Python試験を接続。Standardの評価設定が旧生成幅/採点数を上書きしていた点も修正した。

モデル配布URLは公開準備用で、モデル資産は未公開。過去の自動承認審査でGitHubへのコード送信が明示許可なしとして拒否されており、push/PR/モデル公開は再試行していない。ローカル成果物と検証を揃えた後に許可を求める。

Python: 関連学習/取得9件とリリース検査7件成功。実ビルドスクリプトでも選択モデルの受入れ・別モデルの拒否を確認した。新MSI・品質比較・再構成の結果は以下へ追記する。実TSF・導入/削除・未使用評価・CPU/初回/低メモリ/障害時・署名・独立verifierを含む完成条件は未達。


## ローカルMSIと固定ソース

- Mozc source: `3b29ddd6f3fd34c58a419a8a91c404b8f884cdea`。製品精度のソースはfb6de1384から継続し、追加生成モデルの同梱と版だけ変更した。
- `bazelisk build package //converter:converter_main --config release_build --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm` 成功、1311.296秒。初回はLLVMの絶対参照不一致で失敗し、元ログも保持した。
- C++: LM62件・診断5件・セッション190件、計257件成功。原票 `generator-package-test-receipts/summary.json` と各XML。試験終了後、実際のMozcワークスペースでBazel shutdown成功。最初の停止要求は製品リポジトリから呼んでbatch扱いだったため、正しいワークスペースから実行し直した。
- ローカル候補 `target/accuracy-guide/release-candidate-1.1.4-rc.1/Asteria64.msi`、1,662,377,984 bytes、SHA256 `9ddf96a55d7488234e8e7e9a68d12b223363de4ee8069b680a16a1ae6a964701`、未署名。
- 実MSI ProductVersion / mozc_server.exe は `3.34.6339.100` で一致。現在の導入済みは `3.34.6337.100`。先頭3フィールドで新しいことを確認した。実際の上書き導入は未実施。
- `check_release.py --local <候補> --require-generator --work target/accuracy-guide/generator-package-check-v1` 成功。展開先の元3モデルと追加モデル全4件のSHAも固定値に一致。通信部品ggml-rpc.dllなし。原票 `generator-package-check.log`、`generator-full-model-check.log`。展開は導入済みIMEを変更しない。
- `make_patches.sh`で製品パッチ再生成、素の基点a069a88dへ0001〜0003適用成功。ビルド元132ファイル不一致0、適用後の実ソースwhitespace検査成功。`generator-source-reconstruction.json`。外側のpatchファイル差分では空の文脈行（単一スペース）をtrailing whitespaceとして検出したため、適用後ソースの検査と区別している。パッチを手修正せず、生成元との一致を維持した。

このMSIをインストール・実TSF入力・アンインストールした証拠はまだない。MSI内モデルを使う新converterのAJIMEE比較は次に実施する。


後続の較正は[順位付け較正](accuracy-confidence-calibration-20261018.md)へ記録。新ソース407a449f5で開発308/326、既出300問の回帰290/300。ここに記載した旧MSIには較正を含めていないので、新リリースとして配布しない。
