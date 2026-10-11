"""公開済みの実入力比較を、Asteriaの図版スタイルで再描画する。再測定ではない。"""
import csv
from pathlib import Path
from asteria_style import Figure, CREAM, GRAY, GRAY2, SUB

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def published_rows():
    with (ROOT / "eval/imebench/comparison-2026-10-01.csv").open(encoding="utf-8", newline="") as f:
        rows = list(csv.DictReader(f))
    assert [r["ime"] for r in rows] == ["Kotori", "Microsoft IME", "Google Japanese Input"]
    assert all(r["date"] == "2026-10-01" and r["previous_context"] == "none" for r in rows)
    assert [float(r["ajimee_198_percent"]) for r in rows] == [88.4, 59.6, 58.1]
    return rows


def main():
    rows = published_rows()
    f = Figure(525, "日本語入力の正確さを、同じ条件で。", "AJIMEE-Bench 198問 ／ Space 1回で選ばれた第1候補の正解率")
    names = ["Kotori 日本語入力", "Microsoft IME", "Google 日本語入力"]
    left, width = 260, 490
    for i, (r, name, color) in enumerate(zip(rows, names, [CREAM, GRAY, GRAY2])):
        y = 136 + i*72
        f.text(36, y+6, name, 16, bold=i == 0)
        if i == 0:
            f.text(36, y+32, "現 Asteria・測定時の名称", 10, SUB)
        value = float(r["ajimee_198_percent"])
        f.bar(left, y+6, width, 29, value, color=color)
        f.text(left+width*value/100+12, y+2, f"{value:.1f}%", 21, color if i == 0 else SUB, bold=True)
    f.axis(left, 353, width)
    f.text(779, 361, "%", 10, SUB)
    ours, msime, google = [float(r["ajimee_198_percent"]) for r in rows]
    f.text(36, 391, f"Microsoft IMEとの差 +{ours-msime:.1f}ポイント ／ Google 日本語入力との差 +{ours-google:.1f}ポイント", 13, CREAM)
    f.footer("2026-10-01 ／ RTX 3060 ／ 全IMEで前の文なし",
             "Kotori beta.8・Unreal ／ Microsoft IME（Windows 11）／ Google 日本語入力 3.34.6260",
             "公開済みの測定結果を再描画。現在版の再測定値ではありません。")
    f.save("ime-comparison.png")


if __name__ == "__main__":
    main()
