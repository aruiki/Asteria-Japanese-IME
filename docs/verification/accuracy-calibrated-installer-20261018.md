# 較正済みソースのMSI候補（2026-10-18）

ソース407a449f56e3291e1c6d7ac3e38a882d74ef3427から新しいMSIを生成し、実MSIの管理展開・モデル全4件のSHA・版・ビルド元サーバーとの完全一致を確認した。これはビルド候補で、インストール・実TSF・アンインストール・公開は未実施。以前の較正前MSIをこの候補へ読み替えない。

## 固定した成果物

| 項目 | 結果 |
| --- | --- |
| ソース | 407a449f56e3291e1c6d7ac3e38a882d74ef3427 |
| 枝 | codex/release-quality-calibration-20261018 |
| MSI | target/accuracy-guide/release-candidate-1.1.4-rc.1-calibrated/Asteria64.msi |
| バイト数 | 1,662,353,408（約1.55GiB） |
| SHA-256 | e09bffa5479ea44b95afd4d2a14a89ba900be95ada0c8ba512dc15a74ae530fb |
| ProductVersion | 3.34.6339.100 |
| 署名 | NotSigned（Get-AuthenticodeSignatureで確認） |
| 同梱サーバーSHA | bbaa6a7a32737c97003bf5a8c34337860e8db2a28129daad2d58c22a0c64c6b9 |

較正前の内部候補と同じProductVersionだが、MSIのバイトは異なる。両方とも未公開・未導入であり、旧内部候補の上書き更新を確認したとはしない。公開用CIでは従来どおりBUILDを増やし、実際のProductVersionで更新条件を確認する。

## 実行した検査

`ag16/src`で`bazelisk build package --config release_build --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm`成功、exit0、966.349秒。build後の正規化したgit diffは空、HEADは固定値と一致。Bazel shutdownも同じワークスペースから成功した。ビルド中に測定・ソース変更・別Bazelは実行していない。

元のBazel出力とコピー先のMSIのSHAが一致。元の較正前MSIを保持して別フォルダへ保存した。candidate.jsonとMSI.sha256、build.logも同フォルダへ保存。

`check_release.py --local <新MSI> --require-generator --work target/accuracy-guide/confidence-package-check-v2`はexit0。実展開した元3モデルと追加生成モデル全4件のSHA、追加モデルのmetadata、NOTICE/ライセンス、必須バイナリが検証用ソースの固定情報と一致した。ggml-rpc.dllなし、mozc_server.exeのFileVersionとMSI ProductVersionが一致。

展開したmozc_server.exeのSHAはBazelが生成したサーバーと完全一致。実際のビルド元は構成遷移先`x64_windows-opt-ST-908940cc2e23/bin/server/mozc_server.exe.exe`で、通常の`bazel-bin/server/mozc_server.exe`ではなかった。candidate.jsonへ実パスとSHAを記録し、推測した別バイナリと比較していない。

v1の管理展開はWindows Installerの別処理中でexit1618。既存のインストーラーを停止せず、別トランザクションの正常終了イベントを確認してから新規フォルダのv2で成功した。v1ログを保持。候補MSIの導入失敗や成功を示すものではない。

原票`target/accuracy-guide/confidence-package-build-v1.log`、`confidence-package-check-v{1,2}.log`、候補フォルダのcandidate.json、展開先`confidence-package-check-v2/ext/PFiles/Kotori/`。

## 品質と未完了条件

この候補のソースは[較正の精度検証](accuracy-confidence-calibration-20261018.md)でAJIMEE188/200、開発308/326を確認したもの。[CPU即時Space](accuracy-cpu-startup-20261018.md)は隔離セッション30件の入力経路成功、表記28件一致。実MSIに含むmozc_server.exeを導入してこれらを再現した証拠ではない。

初回AI準備前の誤変換、初回キーの同期初期化の疑い、実アプリのTSF/表示/確定、保護フィールド、低性能/低メモリ/障害時、導入/更新/削除、新しい独立評価、署名・独立verifier・公開は残る。コード/モデルのGitHub送信は明示許可待ちで、今回もpush/PR/CI/公開はしていない。モデル資産の取得URLは未公開のままなので、そのままmainへ統合しない。
