"""v5.5 の紹介動画と README の GIF を組み立てる(2026-11-25)。

場面: 表紙 → 実際の入力の録画(eval/imebench/PromoTyping.cs で撮ったもの)→ 精度の比較 → インストーラー → 終わり。
録画は「正直さの決まり」(promo-video-v3-requirements.md)に従い、Space 1 回の第一候補をそのまま使う。
比較の数字は 2026-11-11 の実入力の測定(docs/verification/accuracy-realinput-20261111.md)。音は入れない。

使い方: python docs/marketing/make_promo_v5.py <録画.mp4> <インストーラーの画像.png> <出力フォルダー>
"""
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
W, H = 1920, 1080
BG = (250, 250, 249)
INK = (24, 24, 27)
SUB = (113, 113, 122)
FAINT = (161, 161, 170)
NAVY = (19, 26, 42)
JP = "C:/Windows/Fonts/YuGothM.ttc"
JP_BOLD = "C:/Windows/Fonts/YuGothB.ttc"
LATIN = "C:/Windows/Fonts/segoeuisl.ttf" if Path("C:/Windows/Fonts/segoeuisl.ttf").exists() else JP


def font(path, size):
    return ImageFont.truetype(path, size)


def centered(draw, y, text, f, fill):
    w = draw.textlength(text, font=f)
    draw.text(((W - w) / 2, y), text, font=f, fill=fill)


def icon(size):
    im = Image.open(ROOT / "docs/images/asteria-icon-240.png").convert("RGBA")
    return im.resize((size, size), Image.LANCZOS)


def card_title(out):
    im = Image.new("RGB", (W, H), BG)
    im.paste(icon(200), ((W - 200) // 2, 300), icon(200))
    d = ImageDraw.Draw(im)
    centered(d, 545, "Asteria", font(JP_BOLD, 84), INK)
    centered(d, 665, "日本語入力", font(JP, 38), SUB)
    im.save(out)


def card_caption(out, clip_frame, caption):
    """録画の場面の地(録画は ffmpeg で上に重ねる)。"""
    im = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(im)
    centered(d, 700, caption, font(JP, 40), INK)
    centered(d, 770, "実際の入力の録画(Asteria 5.4.0)", font(JP, 26), FAINT)
    im.save(out)


def card_accuracy(out):
    im = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(im)
    centered(d, 170, "前の文で正解が決まる同音語", font(JP, 44), INK)
    rows = [("Asteria", 91, NAVY), ("Google日本語入力", 45, (190, 190, 196))]
    x0, x1 = 600, 1560
    for i, (name, pct, color) in enumerate(rows):
        y = 330 + i * 150
        f = font(JP, 34)
        d.text((x0 - 30 - d.textlength(name, font=f), y + 18), name, font=f, fill=INK)
        d.rounded_rectangle((x0, y, x1, y + 80), radius=10, fill=(236, 236, 238))
        d.rounded_rectangle((x0, y, x0 + (x1 - x0) * pct / 100, y + 80), radius=10, fill=color)
        d.text((x0 + (x1 - x0) * pct / 100 + 20, y + 14), f"{pct}%", font=font(JP_BOLD, 44), fill=INK)
    note = ("同じ PC・同じ打ち方で、調整に使っていない 300 問を変換し、Space で出た最初の候補を数えました。"
            "Asteria 開発版 3.34.6464、Google日本語入力 3.34.6260、2026 年 11 月。")
    centered(d, 720, note[:47], font(JP, 24), FAINT)
    centered(d, 760, note[47:], font(JP, 24), FAINT)
    im.save(out)


def card_installer(out, installer_png):
    im = Image.new("RGB", (W, H), BG)
    shot = Image.open(installer_png).convert("RGBA")
    shot = shot.resize((int(shot.width * 0.95), int(shot.height * 0.95)), Image.LANCZOS)
    im.paste(shot, ((W - shot.width) // 2, 40), shot)
    d = ImageDraw.Draw(im)
    centered(d, 40 + shot.height + 10, "ダウンロードして、インストールを押すだけ", font(JP, 40), INK)
    im.save(out)


def card_end(out):
    im = Image.new("RGB", (W, H), BG)
    im.paste(icon(140), ((W - 140) // 2, 330), icon(140))
    d = ImageDraw.Draw(im)
    centered(d, 500, "無料・オープンソース　Windows 10 / 11", font(JP, 40), INK)
    centered(d, 575, "github.com/aruiki/Asteria-Japanese-IME", font(JP, 30), SUB)
    centered(d, 880, "by aruiki　stemstudio.jp", font(JP, 24), FAINT)
    im.save(out)


def run(*args):
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", *args], check=True)


def main():
    clip, installer, outdir = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    outdir.mkdir(parents=True, exist_ok=True)
    work = outdir / "work"
    work.mkdir(exist_ok=True)
    card_title(work / "1.png")
    card_caption(work / "2bg.png", None, "ローマ字のまま続けて打って、Space を 1 回")
    card_accuracy(work / "3.png")
    card_installer(work / "4.png", installer)
    card_end(work / "5.png")

    # 録画の場面: 地の上に録画を拡大して重ねる(録画は 16 秒)。
    # 変換した文を読めるよう、録画の最後の画面を 2.5 秒止めて見せる。
    run("-loop", "1", "-t", "18.5", "-i", str(work / "2bg.png"), "-i", str(clip),
        "-filter_complex",
        "[1:v]tpad=stop_mode=clone:stop_duration=2.5,scale=1640:-2:flags=lanczos[c];"
        "[0:v][c]overlay=(W-w)/2:430,format=yuv420p",
        "-r", "30", "-t", "18.5", str(work / "2.mp4"))
    for n, sec in [(1, 3), (3, 5), (4, 4), (5, 3.5)]:
        run("-loop", "1", "-t", str(sec), "-i", str(work / f"{n}.png"), "-vf", "format=yuv420p", "-r", "30",
            str(work / f"{n}.mp4"))
    # つなぎは 0.4 秒のクロスフェード。
    parts = [work / f"{n}.mp4" for n in (1, 2, 3, 4, 5)]
    durs = [3, 18.5, 5, 4, 3.5]
    inputs = sum([["-i", str(p)] for p in parts], [])
    chain, offset, last = [], 0.0, "[0:v]"
    for i in range(1, len(parts)):
        offset += durs[i - 1] - 0.4
        tag = f"[x{i}]"
        chain.append(f"{last}[{i}:v]xfade=transition=fade:duration=0.4:offset={offset:.2f}{tag}")
        last = tag
    video = outdir / "asteria-promo.mp4"
    run(*inputs, "-filter_complex", ";".join(chain), "-map", last, "-c:v", "libx264", "-crf", "18",
        "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(video))
    # README の GIF: 960 px 幅、12fps、パレットを作ってから。
    gif = outdir / "promo-preview.gif"
    run("-i", str(video), "-vf", "fps=12,scale=960:-1:flags=lanczos,palettegen=max_colors=96", str(work / "pal.png"))
    run("-i", str(video), "-i", str(work / "pal.png"), "-lavfi",
        "fps=12,scale=960:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4", str(gif))
    # ポスター(最初の場面)。
    run("-i", str(work / "1.png"), "-vf", "scale=1280:-1", str(outdir / "promo-poster.jpg"))
    for p in (video, gif):
        print(p, p.stat().st_size)


if __name__ == "__main__":
    main()
