"""READMEの説明図・比較図を公開済みデータから描く。python docs/images/gen_readme_images.py
必要: PillowとWindowsのYu Gothic / Noto Serif JP。画像生成の月は別に保存。
実画面(settings*.png)と過去のhero.pngはこのスクリプトでは変更しない。
"""
import difflib
from asteria_style import Figure, CREAM, TEXT, SUB, GRAY, GRAY2
from gen_ime_comparison import published_rows, main as ime_comparison


def features():
    f = Figure(475, "考えることに、集中できる入力を。", "いつもの日本語入力に、文脈を読むAIを。")
    items = [
        ("01", "文脈を読む", ["同じ読みを持つ言葉も、", "文章のつながりから選びます。"]),
        ("02", "続きを予測する", ["入力を止めると、裏で次の候補を", "考えます。Tabで選べます。"]),
        ("03", "PCの中で完結", ["辞書もAIモデルもPC内に。", "入力した文章を外へ送りません。"]),
        ("04", "いつもの操作で", ["Mozcの操作、学習、", "ユーザー辞書を使えます。"]),
        ("05", "環境に合わせる", ["専用GPUがないPCでは、", "CPU向けの設定に切り替えます。"]),
        ("06", "使わない間は休む", ["10分間入力がなければ、", "AIモデルのVRAMを解放します。"]),
    ]
    for i, (number, head, lines) in enumerate(items):
        x, y = 36+(i%3)*275, 120+(i//3)*155
        f.panel(x, y, 258, 137)
        f.text(x+18, y+14, number, 12, CREAM, serif=True)
        f.text(x+18, y+43, head, 19, serif=True)
        for j, line in enumerate(lines):
            f.text(x+18, y+82+j*22, line, 13, SUB, max_width=222)
    f.save("features.png")


def examples():
    rows = [
        ("かれはこうえんでこうえんをした", "", "彼は公園で公園をした", "彼は公園で公演をした"),
        ("おんせいにんしきのせいどがあがった", "", "音声認識の制度が上がった", "音声認識の精度が上がった"),
        ("あしたのかいぎはじゅうじからです", "", "明日の会議は従事からです", "明日の会議は１０時からです"),
        ("このやくはむずかしい", "英語の文を日本語にしています。", "この薬は難しい", "この訳は難しい"),
        ("ここではきものをぬいでください", "玄関で靴を脱いで。", "ここでは着物を脱いでください", "ここで履物を脱いでください"),
    ]
    f = Figure(710, "読みは同じ。言葉は、文脈で変わる。", "公開済みの変換例 ／ Standard ／ 2026-10-01")
    for i, (yomi, context, mozc, kotori) in enumerate(rows):
        y = 120+i*104
        if i:
            f.rule(36, y-10, 844)
        f.text(36, y, yomi, 13, SUB)
        if context:
            f.text(420, y, "前の文："+context, 11, SUB, max_width=424)
        f.text(36, y+30, "Mozc", 12, SUB)
        f.text(128, y+26, mozc, 17, SUB)
        f.text(36, y+62, "Kotori", 12, CREAM, bold=True)
        x = 128
        for op, _, _, j1, j2 in difflib.SequenceMatcher(None, mozc, kotori).get_opcodes():
            value = kotori[j1:j2]
            if value:
                x += f.text(x, y+58, value, 17, TEXT if op == "equal" else CREAM, bold=op != "equal")
    f.footer("KotoriはAsteriaの旧名称です。例は測定時の出力を保持しています。",
             "文脈を使う例では、上記の前の文を渡しています。")
    f.save("examples.png")


def accuracy():
    rows = [
        ("Kotori Unreal", 93.0, CREAM), ("Kotori High", 92.5, CREAM),
        ("Kotori Standard", 91.5, CREAM), ("Kotori Low（GPU）", 88.0, CREAM),
        ("Kotori Low（CPU）", 83.0, CREAM), ("azooKey + Zenzai（参考）", 85.0, GRAY),
        ("Mozc", 51.0, GRAY2),
    ]
    f = Figure(690, "変換器の評価 — 品質設定ごとの正解率", "AJIMEE-Bench 200問 ／ 前の文あり ／ 実入力比較とは条件が異なります")
    for i, (name, value, color) in enumerate(rows):
        y = 126+i*47
        f.text(36, y+5, name, 14, color)
        f.bar(292, y+5, 440, 24, value, color=color)
        f.text(292+440*value/100+12, y+1, f"{value:.1f}%", 17, color)
    f.axis(292, 465, 440)
    f.text(36, 515, "日常の文 81問 ／ Standard 97.5%・Mozc 80.2%", 16, serif=True)
    f.footer("測定：2026-10-01 ／ RTX 3060（CPU行を除く）／ Kotoriは現Asteriaの旧名称",
             "azooKey参考値：zenz-v3.2-small・推論5回。別製品との実入力比較はime-comparison.pngを参照。",
             "公開済みの値を保持。現在版の再測定値ではありません。")
    f.save("accuracy.png")


def compare():
    data = published_rows()
    groups = [
        ("AJIMEE-Bench", "198問", "ajimee_198_percent"),
        ("日常の文", "81問", "daily_81_percent"),
        ("同音語の使い分け", "40問", "homophones_40_percent"),
        ("人名・地名・新語", "40問", "names_40_percent"),
    ]
    colors = [CREAM, GRAY, GRAY2]
    f = Figure(700, "同じ入力で、四つの比較。", "実際のIMEへキー入力 ／ Space 1回の第1候補 ／ 前の文なし")
    for i, (name, color) in enumerate(zip(["Kotori（現 Asteria）", "Microsoft IME", "Google 日本語入力"], colors)):
        f.text(36+i*275, 113, name, 13, color)
    for i, (name, count, key) in enumerate(groups):
        y = 151+i*113
        f.text(36, y+8, name, 17, serif=True)
        f.text(36, y+39, count, 12, SUB)
        for j, (r, color) in enumerate(zip(data, colors)):
            value = float(r[key])
            f.bar(300, y+j*28, 435, 19, value, color=color)
            f.text(300+435*value/100+10, y+j*28-3, f"{value:.1f}%", 13, color)
        if i < len(groups)-1:
            f.rule(36, y+99, 844)
    f.axis(300, 605, 435)
    f.footer("2026-10-01 ／ RTX 3060 ／ Kotori beta.8・Unreal ／ 全角・半角の差は正規化",
             "Microsoft IME：Windows 11 ／ Google 日本語入力：3.34.6260 ／ 現在版の再測定ではありません。")
    f.save("compare.png")


def lightness():
    f = Figure(415, "使うときは働き、使わないときは休む。", "公開済みの負荷・待ち時間の改善記録 ／ RTX 3060・Ryzen 7 5700X")
    panels = [
        ("入力中のAI稼働率", "GPUを使っている時間の割合", [("beta.4", 68.6), ("改善後", 42.6)], "%", 100),
        ("待機中のVRAM", "Standard・入力をやめて10分後", [("解放前", 1.66), ("解放後", 0.0)], "GB", 2),
        ("CPUでの変換", "長い文・打ってすぐSpaceの中央値", [("改善前", 380), ("改善後", 162)], "ms", 400),
    ]
    for i, (head, sub, vals, unit, vmax) in enumerate(panels):
        x, y = 36+i*275, 121
        f.panel(x, y, 258, 190)
        f.text(x+16, y+14, head, 18, serif=True)
        f.text(x+16, y+46, sub, 11, SUB, max_width=226)
        for j, (label, value) in enumerate(vals):
            yy = y+79+j*48
            color = GRAY if j == 0 else CREAM
            f.text(x+16, yy, label, 11, SUB)
            f.bar(x+76, yy+5, 97, 16, value, vmax, color)
            formatted = f"{value:.2f}" if unit == "GB" else f"{value:.1f}" if unit == "%" else f"{value:.0f}"
            f.text(x+184, yy, formatted+(" "+unit if unit != "%" else unit), 13, color)
        if i == 1:
            f.text(x+16, y+168, "0はモデル解放後の増分", 10, SUB)
    f.footer("2026-09-30〜10-01の記録 ／ ADR0024・0033・0035 ／ 現在版の再測定ではありません。",
             "CPUの測定はデスクトップ環境。機器や文の長さによって待ち時間は変わります。")
    f.save("lightness.png")


def how():
    f = Figure(355, "言葉を選ぶ、その仕組み。", "候補づくりと文脈の評価を、PCの中で行います。")
    steps = [
        ("01", "候補を集める", ["Mozcの辞書と変換用AIが", "複数の候補を作ります。"]),
        ("02", "文脈で評価する", ["前の文とのつながりから", "自然さを採点します。"]),
        ("03", "読みを確かめる", ["生成された候補の読みを", "Mozcの辞書で確認します。"]),
        ("04", "候補を表示する", ["評価に基づいて、", "変換結果を表示します。"]),
    ]
    for i, (number, head, lines) in enumerate(steps):
        x, y = 36+i*207, 122
        f.panel(x, y, 187, 148)
        f.text(x+16, y+12, number, 12, CREAM, serif=True)
        f.text(x+16, y+42, head, 17, serif=True)
        for j, line in enumerate(lines):
            f.text(x+16, y+80+j*23, line, 12, SUB, max_width=155)
        if i < 3:
            f.text(x+191, y+64, "›", 20, SUB)
    f.footer("AIの処理が間に合わないときは、Mozcの結果で入力を続けます。")
    f.save("how.png")


if __name__ == "__main__":
    for draw in (features, examples, accuracy, compare, lightness, how, ime_comparison):
        draw()
    print("ok: 7 Asteria figures; original measurement values preserved")
