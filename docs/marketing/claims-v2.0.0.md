# 紹介文の主張と出典

2026-10-22時点。利用者の指定に従い、紹介の中心は「変換精度の高い日本語IME」。製品の実装と、確認できた範囲を分けて紹介します。

| 紹介する内容 | 根拠・条件 |
| --- | --- |
| 変換精度を中心にした製品紹介 | [測定条件](../BENCHMARKS.md)と既存の実変換改善に基づく製品方針。全入力の精度保証や現行Google日本語入力より高精度という主張はしない。短文辞書は[最終比較済みの配布準備候補](../verification/short-input-lexicon-20261022.md)で、公開v2.0.0には含めていない |
| GPUで動くAIが候補を生成・採点 | [AI主体の候補表示](../adr/0067-ai-first-candidate-publication.md)、[RC4公開記録](../verification/release-v1.1.4-rc.4-20261021.md)。導入済みTSFのGPU AI ON/OFF寄与は未検証 |
| 最初のAI結果の前も読みの表示・編集を継続 | [非同期の検証](../verification/ai-first-publication-20261019.md)、[文脈引継ぎ](../verification/queued-context-20261019.md)。固定EXEの隔離Sessionによる詳細確認 |
| オフライン・変換文章を外部へ送信しない | [データとプライバシー](../USER_GUIDE.md#データとプライバシー)、AGENTSの実行時通信禁止。モデル/実行環境はMSI同梱。保護欄内部のAI/学習停止の実機確認は未完了 |
| GPUなしはCPU向けLow | [ハードウェア別の設定](../adr/0024-cost-and-cpu-low.md)。内蔵GPU不使用。低性能実機の現在版確認は未完了 |
| 英語表記31,068読み・36,402組 | [利用ガイド](../USER_GUIDE.md#使う)。入力モードの変更・追加インポート不要 |
| 188/200、94.0% | [測定条件](../BENCHMARKS.md)。Standard、RTX 3060、AI準備後、指定前文あり、オフラインの調整・回帰用200問。RC4対応sourceでも第一候補数を維持。実アプリ精度や他IMEの最新比較へ転用しない |
| 採点済みの別候補を保持 | [候補保持の検証](../verification/scored-alternatives-20261020.md)。候補包含の改善であり、第一候補精度の改善ではない |
| 導入・入力・削除確認 | [同じv2.0.0 MSIの記録](../verification/release-v2.0.0-20261021.md)。EDIT/RichEditとEdgeの限定条件。旧版更新・再起動・全アプリ受入れではない |
| 無料・オープンソース | [ライセンス](../licenses.md)。支援の有無による機能制限なし |
| 未署名とSmartScreenの案内 | 実MSIのNotSigned、[Microsoft一次資料](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)。警告の意味と出所確認を案内し、安全性や回避成功は保証しない |

v2.0.0の明示URLを使い、GitHubのLatestも正式版v2.0.0であることを確認します。配布MSIは未署名です。

正式版への切替を、全アプリ・GPU実機の導入TSF・低性能PC・内部privacy・再起動・独立完成判定の検証済みという意味へ広げません。
