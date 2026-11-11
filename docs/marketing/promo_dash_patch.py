# 紹介動画の「使い方が、ひと目で。」の場面(33.0〜37.4秒)の設定画面を、今の設定画面の撮影に差し替える。
# 元の場面は表題・紙・窓が動かず、前後とは重ねて切り替わるだけなので、静止した場面の差し替え分を
# 各フレームの場面の不透明度だけ足す。
import argparse, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageFilter
import promo_graph_patch as pg

ROOT = pg.ROOT


def load_frame(n):
    for d in ("new", "f30", "f30b"):
        p = ROOT / d / f"n{n:04d}.png"
        if p.exists():
            return pg.load(p)
    raise FileNotFoundError(n)


def orig_frame(n):
    for d in ("f30", "f30b"):
        p = ROOT / d / f"n{n:04d}.png"
        if p.exists():
            return pg.load(p)
    raise FileNotFoundError(n)


def window_image(path, scale=1.25):
    im = Image.open(path).convert("RGB")
    a = np.asarray(im).astype(int)
    # PrintWindow は窓の外の縁(左右と下の 8px ほど)を黒で返す。黒の縁を落とす。
    W, H = im.size
    dark = a.sum(2) < 30
    left = 0
    while left < 20 and dark[H // 2, left]:
        left += 1
    right = W
    while right > W - 20 and dark[H // 2, right - 1]:
        right -= 1
    bottom = H
    while bottom > H - 20 and dark[bottom - 1, W // 2]:
        bottom -= 1
    im = im.crop((left, 0, right, bottom))
    w, h = im.size
    return im.resize((round(w / scale), round(h / scale)), Image.LANCZOS)


def shadow_mask(x, y, w, h, dy, sigma, size=(1920, 1080)):
    m = Image.new("L", size, 0)
    m.paste(255, (x, y + dy, x + w, y + h + dy))
    return np.asarray(m.filter(ImageFilter.GaussianBlur(sigma))).astype(np.float64) / 255


def compose(static, paper, win, x, y, k, dy, sigma, border):
    # 元の窓の影を割り戻し(表題も影で少し暗くなっている)、元の窓(1px の縁を含む)だけを紙に戻す。
    old_sm = shadow_mask(494, 228, 932, 816, dy, sigma)
    out = static / np.maximum(1 - k * old_sm, 0.5)[..., None]
    ox0, oy0, ox1, oy1 = 489, 223, 1431, 1049
    # 紙の明るさを周りに合わせる。外側 4〜16px の帯での差(各辺に沿ってならす)を、四辺から内側へ
    # なめらかにつなぐ(Coons の面)。
    d = out - paper
    def smooth(a, n=61):
        k = np.ones(n) / n
        return np.stack([np.convolve(np.pad(a[:, c], n // 2, mode="edge"), k, mode="valid") for c in range(3)], 1)
    top = smooth(d[oy0 - 12:oy0 - 3, ox0:ox1].mean(0))
    bot = smooth(d[oy1 + 8:min(oy1 + 30, 1080), ox0:ox1].mean(0))
    lef = smooth(d[oy0:oy1, ox0 - 48:ox0 - 8].mean(1))
    rig = smooth(d[oy0:oy1, ox1 + 8:ox1 + 48].mean(1))
    H, W = oy1 - oy0, ox1 - ox0
    u = (np.arange(W) / (W - 1))[None, :, None]; v = (np.arange(H) / (H - 1))[:, None, None]
    c00, c01, c10, c11 = (top[0] + lef[0]) / 2, (top[-1] + rig[0]) / 2, (bot[0] + lef[-1]) / 2, (bot[-1] + rig[-1]) / 2
    field = ((1 - v) * top[None] + v * bot[None] + (1 - u) * lef[:, None] + u * rig[:, None]
             - ((1 - u) * (1 - v) * c00 + u * (1 - v) * c01 + (1 - u) * v * c10 + u * v * c11))
    fill = paper[oy0:oy1, ox0:ox1] + field
    out[oy0:oy1, ox0:ox1] = fill
    w, h = win.size
    sm = shadow_mask(x, y, w, h, dy, sigma)
    out = out * (1 - (k * sm)[..., None])
    # 1px の縁
    out[y - 1:y + h + 1, x - 1:x + w + 1] = border
    out[y:y + h, x:x + w] = np.asarray(win).astype(np.float64)
    return out


def fit_shadow(static, paper):
    """元の窓(494,228)-(1426,1044)の影に合わせる。"""
    P, D = paper.mean(2), static.mean(2)
    best = None
    for k in (0.16, 0.2, 0.24, 0.28):
        for dy in (6, 10, 14, 18):
            for sigma in (12, 16, 20, 26):
                sm = shadow_mask(494, 228, 932, 816, dy, sigma)
                pred = P * (1 - k * sm)
                e = 0
                for sl in ((slice(1046, 1080), slice(500, 1420)), (slice(240, 1040), slice(1428, 1490)),
                           (slice(240, 1040), slice(430, 492)), (slice(214, 226), slice(500, 1420))):
                    e += np.abs(pred[sl] - D[sl]).mean()
                if best is None or e < best[0]:
                    best = (e, k, dy, sigma)
    return best


def alpha(frame, other_static, static, box):
    x0, y0, x1, y1 = box
    f = frame[y0:y1, x0:x1].ravel(); o = other_static[y0:y1, x0:x1].ravel(); d = static[y0:y1, x0:x1].ravel()
    return float(np.clip(np.dot(f - o, d - o) / max(np.dot(d - o, d - o), 1e-6), 0, 1))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--window", required=True)
    ap.add_argument("--out", default="dashnew")
    ap.add_argument("--top", type=int, default=None)
    ap.add_argument("--scale", type=float, default=1.25)
    args = ap.parse_args()
    paper = pg.plate()
    static = np.median(np.stack([orig_frame(n) for n in range(1010, 1071, 6)]), axis=0)
    fit = fit_shadow(static, paper)
    print("shadow fit", fit, file=sys.stderr)
    _, k, dy, sigma = fit
    border = static[600, 494]
    win = window_image(args.window, args.scale)
    w, h = win.size
    x = (1920 - w) // 2
    y = args.top if args.top is not None else 228 + max(0, (816 - h) // 2)
    new_static = compose(static, paper, win, x, y, k, dy, sigma, border)
    delta = new_static - static
    out = ROOT / args.out; out.mkdir(exist_ok=True)
    Image.fromarray(np.clip(new_static + .5, 0, 255).astype(np.uint8)).save(out / "static.png")
    # 前の場面(グラフ、差し替え後)と次の場面(終わり)の静止画
    graph = pg.load(ROOT / "new/n0984.png")
    end = np.median(np.stack([orig_frame(n) for n in range(1140, 1166, 5)]), axis=0)
    log = []
    for n in range(985, 1167):
        frame = load_frame(n)
        if n < 1040:
            a = alpha(orig_frame(n), pg.load(ROOT / "f30/n0984.png"), static, (520, 1012, 1400, 1040))
        else:
            a = alpha(orig_frame(n), end, static, (500, 790, 1420, 1040))
        img = frame + a * delta
        Image.fromarray(np.clip(img + .5, 0, 255).astype(np.uint8)).save(out / f"n{n:04d}.png")
        log.append((n, round(a, 3)))
    print(log[::4], file=sys.stderr)


if __name__ == "__main__":
    main()
