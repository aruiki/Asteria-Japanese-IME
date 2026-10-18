"""README図版。最新の開発評価と、日付を固定した過去の実入力比較を描く。"""
import difflib
import json
from pathlib import Path
from asteria_style import Figure, CREAM, TEXT, SUB, GRAY, GRAY2
from gen_ime_comparison import main as ime_comparison

DATA = json.loads((Path(__file__).with_name("evaluation-2026-10-19.json")).read_text(encoding="utf-8"))


def examples():
    rows = DATA["ordinary_examples"]
    f = Figure(1220, "日常の言葉を、基礎から。", "追加連語による改善例 ／ AIなし・前の文なし")
    for i,row in enumerate(rows):
        yomi, mozc, kotori = row["input"], row["before"], row["after"]
        y=f.body_top+i*244
        f.panel(24,y-8,592,228)
        f.text(40,y,yomi,20,SUB)
        yy=y+40
        f.text(40,yy,"変更前",20,SUB,bold=True)
        f.text(40,yy+29,mozc,28,TEXT,bold=True)
        f.text(40,yy+73,"Asteria 開発版",20,CREAM,bold=True)
        x=40
        for op,_,_,j1,j2 in difflib.SequenceMatcher(None,mozc,kotori).get_opcodes():
            value=kotori[j1:j2]
            if value:
                x+=f.text(x,yy+102,value,28,TEXT if op=="equal" else CREAM,bold=True)
    f.footer("2026-10-19更新 ／ v1.1.4開発候補・オフライン評価", "日常81問：65→69正解。改善4問・改悪0問。", "実アプリの初回変換を測った結果ではありません。")
    f.save("examples.png")


def features():
    items = [
        ("01", "文脈を読む", ["同じ読みを持つ言葉も、", "文章のつながりから選びます。"]),
        ("02", "続きを予測する", ["入力を止めると、裏で次の候補を", "考えます。Tabで選べます。"]),
        ("03", "PCの中で完結", ["辞書もAIモデルもPC内に。", "入力した文章を外へ送りません。"]),
        ("04", "いつもの操作で", ["Mozcの操作、学習、", "ユーザー辞書を使えます。"]),
        ("05", "環境に合わせる", ["専用GPUがないPCでは、", "CPU向けの設定に切り替えます。"]),
        ("06", "使わない間は休む", ["10分間入力がなければ、", "AIモデルのVRAMを解放します。"]),
    ]
    f=Figure(1200,"考えることに、集中できる入力を。","いつもの日本語入力に、文脈を読むAIを。")
    for i,(number,head,lines) in enumerate(items):
        y=f.body_top+i*160
        f.panel(24,y-6,592,144)
        f.text(40,y+10,number,24,CREAM,bold=True)
        f.text(96,y+7,head,30,TEXT,bold=True)
        for j,line in enumerate(lines):
            f.text(40,y+58+j*34,line,24,TEXT)
    f.save("features.png")


def accuracy():
    f=Figure(1050,"Asteria 開発版の変換精度","AJIMEE-Bench 200問 ／ オフライン評価")
    top = f.body_top
    best = DATA["modes"][2]["scores"]["ajimee"]
    f.text(32,top,f"{100*best['correct']/best['total']:.1f}%",76,CREAM,bold=True)
    f.text(32,top+112,"188 / 200 正解",28,TEXT,bold=True)
    f.text(32,top+162,"Standard・AI準備後・前の文あり",24,SUB)
    f.rule(32,top+220,608)
    for i,(mode,color) in enumerate(zip(DATA["modes"],[GRAY2,GRAY,CREAM])):
        y=top+253+i*130
        score=mode["scores"]["ajimee"]
        value=100*score["correct"]/score["total"]
        f.text(32,y,mode["label"],24,TEXT,bold=True)
        f.text(470,y+35,f"{value:.1f}%",34,color,bold=True)
        f.text(32,y+42,f"{score['correct']} / {score['total']} 正解",22,SUB)
        f.bar(32,y+86,560,23,value,color=color)
    f.footer("2026-10-19更新 ／ 次期版 v1.1.4 開発候補", "AIなしはMozc＋追加連語。StandardはRTX 3060で測定。", "開発用データでの評価。初回や実アプリの精度とは別です。", "他IMEを含む最新の実入力比較は未実施です。")
    f.save("accuracy.png")


def compare():
    colors=[GRAY2,GRAY,CREAM]
    f=Figure(1760,"四つの開発評価で、条件を確認。","Asteria v1.1.4 開発候補 ／ オフライン評価")
    for i,group in enumerate(DATA["sets"]):
        key,name=group["key"],group["label"]
        count=DATA["modes"][0]["scores"][key]["total"]
        y=f.body_top+i*350
        f.text(32,y,f"{name} · {count}問",28,TEXT,bold=True)
        for j,(mode,color) in enumerate(zip(DATA["modes"],colors)):
            yy=y+53+j*90
            score=mode["scores"][key]
            value=100*score["correct"]/score["total"]
            f.text(32,yy,mode["label"],22,TEXT,bold=j==2)
            f.text(480,yy-5,f"{value:.1f}%",29,color,bold=True)
            f.bar(32,yy+37,560,23,value,color=color)
        if i<3: f.rule(32,y+324,608)
    f.footer("2026-10-19更新 ／ StandardはAI準備後・RTX 3060", "AIなしはMozc＋追加連語。前文ありは指定した文脈を使用。", "調整・回帰用の固定データ。独立した製品評価ではありません。", "他IMEとの実入力比較とは測定条件が異なります。")
    f.save("compare.png")


def lightness():
    timing = DATA["native_input"]
    panels = [
        ("キー応答の最大", "通常キーの応答時間", [(r["label"],r["key_max_ms"]) for r in timing], "ms", 4),
        ("Space応答の最大", "起動直後とAI準備後を含む", [(r["label"],r["space_max_ms"]) for r in timing], "ms", 600),
        ("入力シナリオの成功", "文字欠落・確定差分などを確認", [(r["label"],r["passed"]) for r in timing], "件", 30),
    ]
    f=Figure(1230,"入力を続けられるか、確かめる。","ネイティブ入力処理の検査 ／ CPU・RTX 3060")
    for i,(head,sub,vals,unit,vmax) in enumerate(panels):
        y=f.body_top+i*270
        f.panel(24,y-8,592,248)
        f.text(40,y+4,head,30,TEXT,bold=True)
        f.text(40,y+49,sub,20,SUB)
        for j,(label,value) in enumerate(vals):
            yy=y+91+j*67
            color=GRAY if j==0 else CREAM
            f.text(40,yy,label,22,TEXT,bold=True)
            formatted=f"{value:.2f}" if unit=="ms" else f"{value:.0f} / 30"
            f.text(400,yy-4,formatted+" "+unit,28,color,bold=True)
            f.bar(40,yy+37,552,17,value,vmax,color)
    f.footer("2026-10-19更新 ／ v1.1.4開発候補・固定バイナリ", "各30件：新規起動15件＋AI準備後15件。最大値を表示。", "Windows SessionHandlerの検査。実アプリの測定とは別です。", "低性能PCの実機検証や旧版との速度比較は未実施です。")
    f.save("lightness.png")


def how():
    steps = [
        ("01", "候補を集める", ["Mozcの辞書と変換用AIが", "複数の候補を作ります。"]),
        ("02", "文脈で評価する", ["前の文とのつながりから", "自然さを採点します。"]),
        ("03", "読みを確かめる", ["生成された候補の読みを", "Mozcの辞書で確認します。"]),
        ("04", "候補を表示する", ["評価に基づいて、", "変換結果を表示します。"]),
    ]
    f=Figure(970,"言葉を選ぶ、その仕組み。","候補づくりと文脈の評価を、PCの中で。")
    for i,(number,head,lines) in enumerate(steps):
        y=f.body_top+i*175
        f.panel(24,y-8,592,153)
        f.text(40,y+8,number,26,CREAM,bold=True)
        f.text(96,y+4,head,30,TEXT,bold=True)
        for j,line in enumerate(lines): f.text(40,y+60+j*35,line,24,TEXT)
    f.footer("AIが間に合わないときはMozcの結果で入力を続けます。")
    f.save("how.png")

if __name__=="__main__":
    for draw in (features,examples,accuracy,compare,lightness,how,ime_comparison): draw()
    print("ok: current development figures and dated historical IME comparison")
