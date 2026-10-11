"""公開済みの実入力比較を、縮小表示でも読める図版にする。再測定ではない。"""
import csv
from pathlib import Path
from asteria_style import Figure, CREAM, GRAY, GRAY2, SUB, TEXT
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]

def published_rows():
    with (ROOT/"eval/imebench/comparison-2026-10-01.csv").open(encoding="utf-8",newline="") as f:
        rows=list(csv.DictReader(f))
    assert [r["ime"] for r in rows]==["Kotori","Microsoft IME","Google Japanese Input"]
    assert all(r["date"]=="2026-10-01" and r["previous_context"]=="none" for r in rows)
    assert [float(r["ajimee_198_percent"]) for r in rows]==[88.4,59.6,58.1]
    return rows

def main():
    rows=published_rows()
    f=Figure(790,"同じ条件で、正確さを比較。","AJIMEE-Bench 198問 ／ Space 1回の第1候補")
    names=["Asteria（測定時：Kotori）","Microsoft IME","Google 日本語入力"]
    for i,(r,name,color) in enumerate(zip(rows,names,[CREAM,GRAY,GRAY2])):
        y=f.body_top+i*133
        value=float(r["ajimee_198_percent"])
        f.text(32,y,name,25,TEXT,bold=True)
        f.text(453,y-9,f"{value:.1f}%",38,color,bold=True)
        f.bar(32,y+49,560,34,value,color=color)
    ours,msime,google=[float(r["ajimee_198_percent"]) for r in rows]
    f.text(32,570,f"Microsoft IMEとの差 +{ours-msime:.1f}ポイント",24,CREAM,bold=True)
    f.text(32,612,f"Google 日本語入力との差 +{ours-google:.1f}ポイント",24,CREAM,bold=True)
    f.footer("2026-10-01 ／ RTX 3060 ／ 全IMEで前の文なし", "Kotori beta.8・Unreal ／ Microsoft IME（Windows 11）", "Google 日本語入力：3.34.6260。現在版の再測定ではありません。")
    f.save("ime-comparison.png")
if __name__=="__main__": main()
