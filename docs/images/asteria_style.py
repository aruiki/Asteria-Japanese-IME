"""Asteriaの図版をデータから描く共通スタイル。紺の紙、生成り、細い罫線。"""
from pathlib import Path
import random
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
SCALE = 2
NAVY = "#111c2d"
PANEL = "#192638"
CREAM = "#efe0c7"
TEXT = "#eee7dc"
SUB = "#b0b9c3"
LINE = "#364355"
GRAY = "#9babb9"
GRAY2 = "#7a8da1"
FONTS = Path("C:/Windows/Fonts")
SANS = FONTS / "YuGothM.ttc"
BOLD = FONTS / "YuGothB.ttc"
SERIF = FONTS / "NotoSerifJP-VF.ttf"


class Figure:
    def __init__(self, height, title, subtitle, width=880):
        self.width, self.height = width, height
        self.image = Image.new("RGB", (width * SCALE, height * SCALE), NAVY)
        self.draw = ImageDraw.Draw(self.image)
        rng = random.Random(5)
        # 新規の図版に薄い紙の繊維を描く。数値と棒の端は別に正確に描く。
        for _ in range(width * height // 50):
            x, y = rng.randrange(width * SCALE), rng.randrange(height * SCALE)
            color = (22, 33, 48) if rng.random() > .5 else (14, 25, 42)
            self.draw.line((x, y, x + rng.randrange(1, 7), y + 1), fill=color)
        self.text(36, 25, title, 27, serif=True)
        self.text(36, 71, subtitle, 12, SUB, max_width=width - 72)
        self.rule(36, 100, width - 36)

    def font(self, size, bold=False, serif=False):
        return ImageFont.truetype(str(SERIF if serif else BOLD if bold else SANS), round(size * SCALE))

    def text(self, x, y, value, size=14, color=TEXT, bold=False, serif=False, max_width=None):
        font = self.font(size, bold, serif)
        if max_width is not None:
            while self.draw.textlength(value, font=font) > max_width * SCALE and size > 10:
                size -= .5
                font = self.font(size, bold, serif)
            assert self.draw.textlength(value, font=font) <= max_width * SCALE, value
        self.draw.text((x * SCALE, y * SCALE), value, font=font, fill=color)
        return self.draw.textlength(value, font=font) / SCALE

    def rule(self, x, y, right, color=LINE):
        self.draw.line((x * SCALE, y * SCALE, right * SCALE, y * SCALE), fill=color, width=1)

    def panel(self, x, y, w, h):
        self.draw.rectangle((x*SCALE, y*SCALE, (x+w)*SCALE, (y+h)*SCALE), fill=PANEL)

    def bar(self, x, y, width, height, value, maximum=100, color=CREAM):
        assert 0 <= value <= maximum
        if value:
            self.draw.rectangle((round(x*SCALE), round(y*SCALE),
                                 round((x+width*value/maximum)*SCALE)-1,
                                 round((y+height)*SCALE)-1), fill=color)

    def axis(self, x, y, width, ticks=(0, 20, 40, 60, 80, 100), maximum=100):
        self.rule(x, y, x+width)
        for value in ticks:
            tx = x + width * value / maximum
            self.draw.line((tx*SCALE, y*SCALE, tx*SCALE, (y+4)*SCALE), fill=LINE)
            self.text(tx-8, y+8, str(value), 10, SUB)

    def footer(self, *lines):
        start = self.height - 24 - len(lines)*19
        self.rule(36, start - 10, self.width - 36)
        for i, line in enumerate(lines):
            self.text(36, start+i*19, line, 11, SUB, max_width=self.width-72)

    def save(self, filename):
        self.image.save(HERE / filename)
