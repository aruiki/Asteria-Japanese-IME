# 紹介動画の比較グラフの場面(27.7〜33.3秒)だけを、新しい測定値に描き直す。
# 元の動画を作った道具が残っていないので、元の映像のフレームに、同じ字体・同じ位置・同じ動きで
# 棒・数字・注記を描き直す。--identity で元の値を入れ、元の映像と一致するかを確かめる。
# 手順と確かめ方は docs/verification/promo-graph-20261111.md。作業フォルダーは環境変数 PROMO_WORK
# (all/a%03d.png = 10fps の全フレーム、f30/n%04d.png = 807〜1005 番のフレーム)。
import argparse, json, os, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(os.environ.get("PROMO_WORK", Path(__file__).parent))
DIGIT_FONT = "C:/Windows/Fonts/NotoSansJP-Regular.otf"
FOOT_FONT = "C:/Windows/Fonts/SourceHanSansJP-Normal.otf"
SS = 4  # 文字と角丸の描画の拡大率(縮めてなめらかにする)

ROWS = [  # 棒の上端 y、元の最終の値(%)
    dict(top=385, section=1, dark=True),
    dict(top=477, section=1, dark=False),
    dict(top=685, section=2, dark=True),
    dict(top=777, section=2, dark=False),
]
BAR_X0, TRACK_X1, BAR_H, RADIUS = 530, 1550, 54, 8
FULL = TRACK_X1 - BAR_X0
FOOT_OLD = ["同じPC・同じ打ち方で、調整に使っていない問題を変換し、Spaceで出た第一候補を数えました。",
            "前の文で正解が決まる同音語300問・日常の短文500問（表記ゆれは同じとみなす）。Asteria v3.0.0（3.34.6457、2026年10月25日）／Google日本語入力",
            "3.34.6260（10月23日）。"]


def load(path):
    return np.asarray(Image.open(path).convert("RGB")).astype(np.float64)


def plate():
    """紙だけの画像。表紙・終わり・最初の場面の紙を重ね、文字やマークの無い(明るい)方を画素ごとに選ぶ。"""
    groups = [range(5, 16), range(370, 386), range(1, 3)]
    cands = [np.median(np.stack([load(ROOT / f"all/a{i:03d}.png") for i in g]), axis=0) for g in groups]
    cands.append(load(ROOT / "f30/n0960.png"))  # グラフの場面の紙(文字・棒のない所)
    stack = np.stack(cands)
    lum = stack.mean(axis=3)
    pick = np.argmax(lum, axis=0)
    return np.take_along_axis(stack, pick[None, ..., None], axis=0)[0]


def rounded_mask(width, x0, x1, top):
    """[x0, x1) × [top, top+BAR_H) の角丸の棒の被覆率(0〜1)。画面全体の大きさで返す。"""
    m = np.zeros((1080, 1920))
    if x1 - x0 < 0.5:
        return m
    gx0, gx1 = int(np.floor(x0)) - 1, int(np.ceil(x1)) + 1
    im = Image.new("L", ((gx1 - gx0) * SS, BAR_H * SS), 0)
    r = min(RADIUS, (x1 - x0) / 2, BAR_H / 2) * SS
    ImageDraw.Draw(im).rounded_rectangle(
        [(x0 - gx0) * SS, 0, (x1 - gx0) * SS - 1, BAR_H * SS - 1], radius=r, fill=255)
    small = np.asarray(im.resize((gx1 - gx0, BAR_H), Image.BOX)).astype(np.float64) / 255
    m[top:top + BAR_H, gx0:gx1] = small
    return m


def text_mask(text, font_path, size, x, baseline, anchor):
    font = ImageFont.truetype(font_path, size * SS)
    l, t, r, b = font.getbbox(text, anchor=anchor)
    pad = 2 * SS
    w, h = r - l + 2 * pad, b - t + 2 * pad
    # 原点を SS の倍数にそろえ、小数の位置は拡大した側でずらす。
    ox, oy = x * SS + l - pad, baseline * SS + t - pad
    gx, gy = int(np.floor(ox / SS)), int(np.floor(oy / SS))
    W, H = (int(np.ceil((ox + w) / SS)) - gx + 1), (int(np.ceil((oy + h) / SS)) - gy + 1)
    im = Image.new("L", (W * SS, H * SS), 0)
    ImageDraw.Draw(im).text((x * SS - gx * SS, baseline * SS - gy * SS), text, font=font, fill=255, anchor=anchor)
    small = np.asarray(im.resize((W, H), Image.BOX)).astype(np.float64) / 255
    m = np.zeros((1080, 1920))
    m[gy:gy + H, gx:gx + W] = small
    return m


def over(img, mask, color, alpha=1.0):
    a = (mask * alpha)[..., None]
    return img * (1 - a) + np.asarray(color, dtype=np.float64) * a


class Model:
    """最終のフレームから、紙・棒の帯・棒・文字の色と位置を合わせる。"""

    def __init__(self, final, paper):
        self.paper = paper
        # 帯(半透明): 帯の部分の画素 = paper*(1-a) + c*a。棒の無い右端で合わせる。
        ys, xs = [], []
        for row in ROWS:
            ys.append(slice(row["top"] + 10, row["top"] + BAR_H - 10))
        sel_p, sel_f = [], []
        for row, s in zip(ROWS, ys):
            sel_p.append(paper[s, 1480:1540].reshape(-1, 3)); sel_f.append(final[s, 1480:1540].reshape(-1, 3))
        P, Fv = np.concatenate(sel_p), np.concatenate(sel_f)
        self.track_k, self.track_b = [], []
        for ch in range(3):
            k, b = np.polyfit(P[:, ch], Fv[:, ch], 1)
            self.track_k.append(k); self.track_b.append(b)
        self.track_k, self.track_b = np.array(self.track_k), np.array(self.track_b)
        self.bar_color = {True: np.median(final[400:420, 600:900].reshape(-1, 3), axis=0),
                          False: np.median(final[492:512, 600:900].reshape(-1, 3), axis=0)}
        self.final_len = [self.measure(final, i, 1.0) for i in range(4)]
        self.digit_color = self.fit_color(final, [self.digit_mask(i, s) for i, s in enumerate(["91", "44", "92", "87"])])
        # 注記は細いので、太い画の色(暗い方から 2%)をそのまま使う。
        foot = final[905:1003, 200:1700].reshape(-1, 3)
        self.foot_color = foot[np.argsort(foot.mean(1))[int(len(foot) * 0.02)]]

    def track(self, base, alpha):
        full = base * self.track_k + self.track_b
        return base + alpha * (full - base)

    def measure(self, frame, i, alpha):
        row = ROWS[i]
        y = row["top"] + BAR_H // 2
        line = frame[y - 3:y + 4, BAR_X0 - 2:TRACK_X1 + 2].mean(axis=(0, 2))
        base = self.paper[y - 3:y + 4, BAR_X0 - 2:TRACK_X1 + 2].mean(axis=(0, 2))
        trk = base + alpha * ((base[:, None] * self.track_k + self.track_b).mean(1) - base)
        bar = base + alpha * (self.bar_color[row["dark"]].mean() - base)
        frac = np.clip((trk - line) / np.maximum(trk - bar, 1e-6), 0, 1)
        if alpha < 0.05:
            return 0.0
        # 棒の端: 始まりから続く「棒」の画素の数と、端の画素の割合(帯の雑音は数えない)。
        start = 2
        k = start
        while k < len(frac) and frac[k] >= 0.5:
            k += 1
        if k == start:
            return 0.0
        return float(k - start + (frac[k] - 0.5 if k < len(frac) else 0))

    def digit_mask(self, i, text):
        row = ROWS[i]
        return text_mask(text, DIGIT_FONT, 44, DIGIT_RIGHT, row["top"] + DIGIT_BASE, "rs")

    def foot_mask(self, lines):
        m = np.zeros((1080, 1920))
        font = ImageFont.truetype(FOOT_FONT, 22 * SS)
        for k, line in enumerate(lines):
            # ブラウザーと同じく、続く約物(「）。」)の間を半角分詰める。
            x = FOOT_X
            pieces = line.split("）。")
            for j, piece in enumerate(pieces):
                text = piece + ("）" if j < len(pieces) - 1 else "")
                if text:
                    m = np.maximum(m, text_mask(text, FOOT_FONT, 22, x, FOOT_BASE + k * FOOT_PITCH, "ls"))
                x += font.getlength(text) / SS
                if j < len(pieces) - 1:
                    x -= 11
                    m = np.maximum(m, text_mask("。", FOOT_FONT, 22, x, FOOT_BASE + k * FOOT_PITCH, "ls"))
                    x += font.getlength("。") / SS
        return m

    def fit_color(self, final, masks):
        m = np.maximum.reduce(masks)
        sel = m > 0.6
        num = ((final - self.paper * (1 - m[..., None])) * m[..., None])[sel].sum(0)
        den = (m[sel] ** 2).sum()
        return num / den


# 位置(--fit で元の映像から求めた値)
DIGIT_RIGHT, DIGIT_BASE = 1696.0, 46.0
FOOT_X, FOOT_BASE, FOOT_PITCH = 199.0, 926.0, 37.5


def opacity(frame, paper, final, box):
    x0, y0, x1, y1 = box
    p = paper[y0:y1, x0:x1].mean(2); f = final[y0:y1, x0:x1].mean(2); a = frame[y0:y1, x0:x1].mean(2)
    return float(np.clip(((p - a).clip(0).mean()) / max((p - f).clip(0).mean(), 1e-6), 0, 1))


PCT_BOX = {1: (1699, 413, 1719, 432), 2: (1699, 713, 1719, 732)}
FOOT_BOX = (202, 905, 1153, 932)


def render(model, frame, alphas, lengths, numbers, foot_lines, foot_alpha):
    """紙に戻してから、帯・棒・数字・注記を描き直した画像を返す。"""
    out = frame.copy()
    P = model.paper
    for i, row in enumerate(ROWS):
        a = alphas[row["section"]]
        y0, y1 = row["top"] - 3, row["top"] + BAR_H + 3
        region = P[y0:y1].copy()
        tm = rounded_mask(None, BAR_X0, TRACK_X1, row["top"])[y0:y1]
        full_trk = region * model.track_k + model.track_b
        region = region + (tm * a)[..., None] * (full_trk - region)
        bm = rounded_mask(None, BAR_X0, BAR_X0 + lengths[i], row["top"])[y0:y1]
        region = over(region, bm, model.bar_color[row["dark"]], a)
        out[y0:y1, BAR_X0 - 4:TRACK_X1 + 4] = region[:, BAR_X0 - 4:TRACK_X1 + 4]
        # 数字(% は元のまま)
        dy0, dy1 = row["top"] - 2, row["top"] + BAR_H + 2
        out[dy0:dy1, 1610:1697] = P[dy0:dy1, 1610:1697]
        dm = model.digit_mask(i, str(numbers[i]))
        dm[:, 1697:] = 0
        out = over(out, dm, model.digit_color, a)
    out[896:1014, 190:1760] = P[896:1014, 190:1760]
    if foot_alpha > 0:
        out = over(out, model.foot_mask(foot_lines), model.foot_color, foot_alpha)
    return out


_CACHE = {}


def read_number(model, frame, i, alpha):
    """元の映像に出ている数字を、描いた数字との照合で読む。"""
    row = ROWS[i]
    y0, y1 = row["top"] - 2, row["top"] + BAR_H + 2
    x0, x1 = 1610, 1697
    P = model.paper[y0:y1, x0:x1]
    F = frame[y0:y1, x0:x1]
    best = None
    for n in range(0, 101):
        key = (i, n)
        if key not in _CACHE:
            _CACHE[key] = model.digit_mask(i, str(n))[y0:y1, x0:x1]
        m = _CACHE[key][..., None] * alpha
        e = np.abs(P * (1 - m) + model.digit_color * m - F).mean()
        if best is None or e < best[0]:
            best = (e, n)
    return best[1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--identity", action="store_true")
    ap.add_argument("--values", default="91.0,45.0,94.8,86.8")
    ap.add_argument("--foot", default=None)
    ap.add_argument("--out", default="patched")
    ap.add_argument("--first", type=int, default=830)
    ap.add_argument("--last", type=int, default=1000)
    args = ap.parse_args()
    paper = plate()
    final = load(ROOT / "f30/n0960.png")
    model = Model(final, paper)
    old_pct = [l / FULL * 100 for l in model.final_len]
    new_pct = old_pct if args.identity else [float(v) for v in args.values.split(",")]
    foot = FOOT_OLD if (args.identity or not args.foot) else json.loads(Path(args.foot).read_text(encoding="utf-8"))
    print("final lengths", [round(l, 1) for l in model.final_len], "old %", [round(p, 2) for p in old_pct],
          "colors", model.bar_color, model.digit_color, model.foot_color, file=sys.stderr)
    out_dir = ROOT / args.out; out_dir.mkdir(exist_ok=True)
    last_full = 984
    report = []
    full_new = None; full_old = None
    for n in range(args.first, args.last + 1):
        frame = load(ROOT / f"f30/n{n:04d}.png")
        if n <= last_full:
            alphas = {s: opacity(frame, paper, final, PCT_BOX[s]) for s in (1, 2)}
            fa = opacity(frame, paper, final, FOOT_BOX)
            lens = [model.measure(frame, i, alphas[ROWS[i]["section"]]) for i in range(4)]
            prog = [l / model.final_len[i] for i, l in enumerate(lens)]
            new_len = [p * new_pct[i] / 100 * FULL for i, p in enumerate(prog)]
            shown = [read_number(model, frame, i, alphas[ROWS[i]["section"]]) if alphas[ROWS[i]["section"]] > 0.05 else 0
                     for i in range(4)]
            nums = [int(np.floor(s / old_pct[i] * new_pct[i] + 0.5)) for i, s in enumerate(shown)]
            img = render(model, frame, alphas, new_len, nums, foot, fa)
            if n == last_full:
                full_new, full_old = img, frame
        else:
            # 全体が消えていくところ: 元の映像に、最終の差分を不透明度の分だけ足す。
            a = opacity(frame, paper, final, PCT_BOX[1])
            img = frame + a * (full_new - full_old)
            nums, alphas, fa = None, {1: a}, a
        Image.fromarray(np.clip(img + 0.5, 0, 255).astype(np.uint8)).save(out_dir / f"n{n:04d}.png")
        report.append(dict(n=n, nums=nums, shown=shown if n <= last_full else None, alphas=alphas, foot=fa,
                           diff=float(np.abs(img - frame).mean())))
    (out_dir / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
