# 73: 公開名と配布導線をAsteriaへ統一

利用者の「すべて移行してください」に従う。GitHubリポジトリをAsteria-Japanese-IMEへ、説明/Topics/リリース表示名/最新配布名/開発と公開スクリプト/README導線を統一する。

既存のGitタグと履歴/リリースID/既存添付ファイルは保持。最新版に同一SHAのAsteria64.msiと専用sha256を追加し、旧Kotori64.msiは旧リンクの互換性用に残す。将来のworkflowでも両名を出し、検証スクリプトはAsteria名を優先し古い版のみKotori名へフォールバック。

内部の保存先/環境変数/GUID/UpgradeCodeはADR0056の互換性方針を維持。旧版の測定記録や受入れログは改変しない。README/公開本文は構成を保ち、公開名とURLを必要箇所だけ変更。

確認: 移行前後のrepo ID/Stars/Forks、旧URL転送、全GitタグSHA、リリースID/公開日/区分、既存添付ファイルID/ハッシュ/ダウンロード数保持、Asteria添付のSHA、公開工具の両名検証、一般CI。

実施結果: 公開メタデータ/タグ/添付/転送照合OK。公開MSI再取得とcheck_release全項目OK。Python14テスト、通知8テスト、3パッチ再生成/適用OK。記録docs/verification/asteria-public-migration-20261012.md。一般CIはPRで確認。
