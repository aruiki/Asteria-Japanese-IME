# v1.1.3の公開用MSI検証（2026-10-12）

ユーザーの「最新版リリース」に従い、PR160を全CI成功後にmainへ統合。ADR0044の条件を満たした未加工CI成果物を公開に使用する。実装・workflow・版番号はCI前に固定し、その後はこの検証記録と公開ノートなどの文書だけを追加する。

## ソースと成果物の固定

- PR head: f006fe56cd59db384bde47e6c69b0ed27cafe7ca
- CI checkout: 4e841b20e87c868128c44dfd3d029ae5ca112064
- main統合: f519326c37bdb3ec6d056a4813fed04abb1f16ad
- 上記3コミットのtreeは一致。公開用commitはv1.1.3タグで固定し、追加差分を文書だけに限定する。
- 一般CI run38235273342、全7ジョブsuccess。main統合後のrun38246133783もsuccess。
- Windows MSI run38235273294（run number100）、build/10対象C++テスト/通知テスト/導入/TSF入力/削除/成果物uploadがsuccess。
- ProductVersion: **3.34.6431.100**。前の公開版3.34.6338.100より先頭3項目が大きい。
- SHA-256: **b7dbaee7e99dcea16544b169cadb7ea1e0c5b66f1225cbc01a044ad4293688d5**
- サイズ: **1,569,517,568 bytes**。1.8GiBの上限以内。
- CI artifactのAsteria64.msiと導入検証原票のMSIハッシュが一致。MSIを編集・再ビルドしない。旧リンク用Kotori64.msiも同じバイト列を公開する。

## 実インストーラーの検証

GitHub-hostedの使い捨てWindowsランナーで、当該MSIを導入（exit0）、当該TIPのTSF入力で「日本語」「東京」の確定を確認して削除した。result.jsonのinstall/input/uninstallはすべてtrue、errorsは空。
削除はexit3010、再起動要求あり。TIPファイル削除を確認した状態で、再起動後の確認を実施したことにはしない。開発者の普段のIME・プロファイルは変更していない。

これは2つの短い入力を確認する導入smokeであり、修正対象のAI ON/OFF効果、複数アプリ、低性能ノートPC、24時間利用の受入れではない。

## 公開前の内容検証

check_release.py --localの全項目成功。同梱モデル3つ、サーバー/設定/描画/TIP、英語辞書、帰属表示・ライセンスを確認。ggml-rpc.dllなし。mozc_serverのファイル版とProductVersionが一致。
配布TIP32/64の月のアイコン8フレーム、設定ツールの従来アイコン8フレームを元ICOのバイト列と照合して一致。実行せずPE資源として読み取った。
3つのモデルSHAは、精度評価に使ったものと完全一致。

## 精度と追加チェック

Standard AJIMEE183/200=91.5%、調整386問の補正保護前後の出力は不変。採用後の最終heldoutは289/300=96.3%、以前と分野ごとの値も一致。heldoutから規則を調整しない。
入力補正の2例が3条件で改善、CPU1,000読み+90操作、姓696読みOFF/ON、初回ロードGPU/CPU各40操作などはlarge-input-campaign-20261012.mdに記録。
ローカルfmt/clippy/全Rust147テスト成功。初回CMake終了0xc0000409と、その後の長いMSBuild出力パスFTK1011は記録に保持。短いASCIIパスで同一ソース・通常native構成を検査し、テスト無効化なし。

## 公開時の確認と残る範囲

旧v1.1.2の公開本文を保持して今回の説明を追加する。旧本文との差はWindows改行形式のみだった。
公開前にdraft assetsのMSIサイズ/digestとタグcommitを照合し、公開後にcheck_release.pyでMSIとSHAを取得し直して前版との版比較・同梱物を確認する。
コード署名なし。複数アプリ・低性能機・長時間利用、Windows通知センターからの後日操作、修正対象の実アプリAI ON/OFF受入れは未実施。既存の長文遅延と意味選択の残課題も引き継ぐ。製品の全完成条件がPASSという判定ではない。

原票はtarget/release-v1.1.3（verified-candidate.json、ci-installer-smoke、pre-public-check、binary-assets.json、ci-model-parity.json、heldout、各ログ）。公開URLはhttps://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.3。
