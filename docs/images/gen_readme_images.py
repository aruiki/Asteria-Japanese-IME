"""README図版。500px幅でも結果を読めるサイズで公開済みデータを描く。"""
import difflib
from asteria_style import Figure, CREAM, TEXT, SUB, GRAY, GRAY2
from gen_ime_comparison import published_rows, main as ime_comparison


def examples():
    rows = [
        ("かれはこうえんでこうえんをした", "", "彼は公園で公園をした", "彼は公園で公演をした"),
        ("おんせいにんしきのせいどがあがった", "", "音声認識の制度が上がった", "音声認識の精度が上がった"),
        ("あしたのかいぎはじゅうじからです", "", "明日の会議は従事からです", "明日の会議は１０時からです"),
        ("このやくはむずかしい", "英語の文を日本語にしています。", "この薬は難しい", "この訳は難しい"),
        ("ここではきものをぬいでください", "玄関で靴を脱いで。", "ここでは着物を脱いでください", "ここで履物を脱いでください"),
    ]
    f = Figure(1470, "同じ読みでも、意味で選ぶ。", "変換例 ／ Standard ／ 2026-10-01")
    for i,(yomi,context,mozc,kotori) in enumerate(rows):
        y=f.body_top+i*244
        f.panel(24,y-8,592,228)
        f.text(40,y,yomi,20,SUB)
        yy=y+40
        if context:
            f.text(40,yy,"前の文："+context,18,SUB)
            yy+=32
        f.text(40,yy,"Mozc",20,SUB,bold=True)
        f.text(40,yy+29,mozc,28,TEXT,bold=True)
        f.text(40,yy+73,"Asteria",20,CREAM,bold=True)
        x=40
        for op,_,_,j1,j2 in difflib.SequenceMatcher(None,mozc,kotori).get_opcodes():
            value=kotori[j1:j2]
            if value:
                x+=f.text(x,yy+102,value,28,TEXT if op=="equal" else CREAM,bold=True)
    f.footer("Kotori（現Asteria）の公開済み出力を保持。", "文脈を使う例は、記載した前の文を渡しています。")
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
    rows = [
        ("Kotori Unreal", 93.0, CREAM), ("Kotori High", 92.5, CREAM),
        ("Kotori Standard", 91.5, CREAM), ("Kotori Low（GPU）", 88.0, CREAM),
        ("Kotori Low（CPU）", 83.0, CREAM), ("azooKey + Zenzai（参考）", 85.0, GRAY),
        ("Mozc", 51.0, GRAY2),
    ]
    f=Figure(1175,"品質設定ごとの正解率","AJIMEE-Bench 200問 ／ 前の文あり")
    for i,(name,value,color) in enumerate(rows):
        y=f.body_top+i*107
        f.text(32,y,name,24,TEXT,bold=True)
        f.text(473,y-7,f"{value:.1f}%",34,color,bold=True)
        f.bar(32,y+46,560,24,value,color=color)
    f.text(32,925,"日常の文 81問",24,TEXT,bold=True)
    f.text(32,967,"Standard 97.5% ／ Mozc 80.2%",28,CREAM,bold=True)
    f.footer("2026-10-01 ／ RTX 3060（CPU行を除く）", "azooKey参考：zenz-v3.2-small・推論5回。", "実入力比較とは別条件。現在版の再測定ではありません。")
    f.save("accuracy.png")


def compare():
    groups = [
        ("AJIMEE-Bench", "198問", "ajimee_198_percent"),
        ("日常の文", "81問", "daily_81_percent"),
        ("同音語の使い分け", "40問", "homophones_40_percent"),
        ("人名・地名・新語", "40問", "names_40_percent"),
    ]
    data=published_rows()
    colors=[CREAM,GRAY,GRAY2]
    names=["Asteria（測定時：Kotori）","Microsoft IME","Google 日本語入力"]
    f=Figure(1700,"同じ入力で、四つの比較。","実際のIME ／ Space 1回の第1候補 ／ 前の文なし")
    for i,(name,count,key) in enumerate(groups):
        y=f.body_top+i*350
        f.text(32,y,name+" · "+count,28,TEXT,bold=True)
        for j,(r,color,label) in enumerate(zip(data,colors,names)):
            yy=y+53+j*90
            value=float(r[key])
            f.text(32,yy,label,22,TEXT,bold=j==0)
            f.text(480,yy-5,f"{value:.1f}%",29,color,bold=True)
            f.bar(32,yy+37,560,23,value,color=color)
        if i<3: f.rule(32,y+324,608)
    f.footer("2026-10-01 ／ RTX 3060 ／ Kotori beta.8・Unreal", "Microsoft IME：Windows 11 ／ Google：3.34.6260", "全角・半角の差は正規化。現在版の再測定ではありません。")
    f.save("compare.png")


def lightness():
    panels = [
        ("入力中のAI稼働率", "GPUを使っている時間の割合", [("beta.4", 68.6), ("改善後", 42.6)], "%", 100),
        ("待機中のVRAM", "Standard・入力をやめて10分後", [("解放前", 1.66), ("解放後", 0.0)], "GB", 2),
        ("CPUでの変換", "長い文・打ってすぐSpaceの中央値", [("改善前", 380), ("改善後", 162)], "ms", 400),
    ]
    f=Figure(1150,"使わないときは、休む。","負荷・待ち時間の改善記録 ／ RTX 3060・Ryzen 7 5700X")
    for i,(head,sub,vals,unit,vmax) in enumerate(panels):
        y=f.body_top+i*270
        f.panel(24,y-8,592,248)
        f.text(40,y+4,head,30,TEXT,bold=True)
        f.text(40,y+49,sub,20,SUB)
        for j,(label,value) in enumerate(vals):
            yy=y+91+j*67
            color=GRAY if j==0 else CREAM
            f.text(40,yy,label,22,TEXT,bold=True)
            formatted=f"{value:.2f}" if unit=="GB" else f"{value:.1f}" if unit=="%" else f"{value:.0f}"
            f.text(418,yy-4,formatted+(" "+unit if unit!="%" else unit),30,color,bold=True)
            f.bar(40,yy+37,552,17,value,vmax,color)
        if i==1: f.text(40,y+221,"0はモデル解放後の増分",16,SUB)
    f.footer("2026-09-30〜10-01 ／ ADR0024・0033・0035", "CPUはデスクトップで測定。機器や文の長さで変わります。", "公開済みの値を保持。現在版の再測定ではありません。")
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
    print("ok: 7 readable figures; original measurement values preserved")
