# v1.1.4-rc.1 公開記録（2026-10-19）

[v1.1.4-rc.1を公開](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.1)。2026-10-19 01:03:04 UTC、prerelease=true、Latest=false。利用者の最新方針はRC公開後も改善を続け、個別の誤変換は開発側で管理すること。公開本文は変更点と導入方法中心とする。

- tag/source: `71300bff41d14b81a9c2f5d12830bfeeb7670fe0`
- build head: `6272fa113eaa63826b2c48bd8f10f03056ae5235`
- build checkout: `0ab7fdaabd2cf5880fe15261b5fa33679eb2f7ec`
- 上記treeは一致。PR168の全8チェック成功、[Windows run39158232305](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39158232305)のMSIを編集せず再利用した（ADR0044）。
- ProductVersion: `3.34.6435.100`、1,662,763,008 bytes。
- SHA-256: `0f12eb1db8726e7a3c49186626098a100c64c36ffd9cab97c9dbe526ffcdf57a`
- MSI、外部SHA、`distribution-info.json`を公開。GitHub側digestも実取得したCI MSIと一致。AuthenticodeはNotSigned、詳細は配布情報へ記録した。

## 自動受入れと内容

同じMSIの導入・EDIT/RichEditの入力/Space/確定、候補Down/PageDown/PageUp、取消、Backspace、F6〜F10、欄の移動、パスワード欄の見える挙動が全成功。同一MSI再導入exit0で、同じファイルSHAと再入力も成功。削除exit3010、再起動要求あり。[追加受入れ原票](candidate-automatic-acceptance-20261019.md)。

管理展開で4モデル/34,661連語/出典・帰属/サーバー版を確認。公開6431より先頭3版が大きい。TIP32/64の製品アイコン群102、設定ツールのアイコン各8フレームがソースICOと完全一致。最初の全120資源対8フレーム比較は対象指定誤りとして保持し、ソースrcのIDに合わせて製品アイコン群だけを比較した。

最終報告用の固定300問はStandard290/300、AIなし247/300（49改善/6改悪）。以前の固定source289/300比は2改善/1改悪。モデル・係数の選択に使わないオフラインconverterの結果であり、実アプリの精度比較ではない。[携帯可能な固定結果](heldout-6432-20261019.json)。AJIMEE188/200とは別のデータ。

公開後は`target/accuracy-guide/verify_public_rc114_v1.py`で公開MSI・SHA・配布情報を再取得し、前公開6431と`check_release.py --local --previous-local --require-generator --require-collocations`で照合する。結果は`target/accuracy-guide/public-rc114-check-v1/`。製品完成/独立verifierの判定とはしない。
