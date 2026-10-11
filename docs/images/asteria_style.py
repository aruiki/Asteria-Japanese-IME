"""GitHubの縮小表示で読めるAsteria図版。紺の紙と生成りを維持する。"""
from pathlib import Path
import random
from PIL import Image, ImageDraw, ImageFont
HERE = Path(__file__).resolve().parent
SCALE = 2
NAVY = "#111c2d"
PANEL = "#192638"
CREAM = "#fff0d7"
TEXT = "#faf5eb"
SUB = "#cbd3dc"
LINE = "#526174"
GRAY = "#bbc8d6"
GRAY2 = "#91a7bd"
FONTS = Path("C:/Windows/Fonts")
SANS = FONTS / "YuGothM.ttc"
BOLD = FONTS / "YuGothB.ttc"
SERIF = FONTS / "NotoSerifJP-VF.ttf"

class Figure:
    def __init__(self, height, title, subtitle, width=640):
        self.width, self.height = width, height
        self.image = Image.new("RGB", (width*SCALE, height*SCALE), NAVY)
        self.draw = ImageDraw.Draw(self.image)
        rng = random.Random(5)
        for _ in range(width*height//50):
            x, y = rng.randrange(width*SCALE), rng.randrange(height*SCALE)
            self.draw.line((x, y, x+rng.randrange(1,7), y+1), fill=(22,33,48))
        y = self.paragraph(32, 24, title, 30, bold=True, line_height=43)
        y = self.paragraph(32, y+12, subtitle, 18, SUB, line_height=28)
        self.rule(32, y+16, width-32)
        self.body_top = y+38

    def font(self, size, bold=False, serif=False):
        return ImageFont.truetype(str(SERIF if serif else BOLD if bold else SANS), round(size*SCALE))

    def text(self, x, y, value, size=24, color=TEXT, bold=False, serif=False, max_width=None):
        font = self.font(size, bold, serif)
        width = self.draw.textlength(value, font=font)/SCALE
        # 読めなくなる縮小で押し込まない。必要なら呼出し側で改行する。
        assert width <= (max_width if max_width is not None else self.width-32-x)+.5, (value, size, width)
        box = self.draw.textbbox((x*SCALE,y*SCALE), value, font=font)
        assert box[3] <= self.height*SCALE, value
        self.draw.text((x*SCALE,y*SCALE), value, font=font, fill=color)
        return width

    def paragraph(self, x, y, value, size=24, color=TEXT, bold=False, width=None, line_height=None):
        available = width or self.width-32-x
        height = line_height or round(size*1.5)
        font = self.font(size, bold)
        for line in value.split("\n"):
            current = ""
            for char in line:
                if current and self.draw.textlength(current+char,font=font)>available*SCALE:
                    self.text(x,y,current,size,color,bold=bold,max_width=available)
                    y += height
                    current = ""
                current += char
            if current:
                self.text(x,y,current,size,color,bold=bold,max_width=available)
                y += height
        return y

    def rule(self,x,y,right,color=LINE):
        self.draw.line((x*SCALE,y*SCALE,right*SCALE,y*SCALE),fill=color,width=2)
    def panel(self,x,y,w,h):
        self.draw.rectangle((x*SCALE,y*SCALE,(x+w)*SCALE,(y+h)*SCALE),fill=PANEL)
    def bar(self,x,y,width,height,value,maximum=100,color=CREAM):
        assert 0<=value<=maximum
        if value:
            self.draw.rectangle((round(x*SCALE),round(y*SCALE),round((x+width*value/maximum)*SCALE)-1,round((y+height)*SCALE)-1),fill=color)
    def axis(self,x,y,width,ticks=(0,20,40,60,80,100),maximum=100):
        self.rule(x,y,x+width)
        for value in ticks:
            tx=x+width*value/maximum
            self.text(tx-8,y+8,str(value),16,SUB)
    def footer(self,*lines):
        wrapped=[]
        font=self.font(16)
        for line in lines:
            current=""
            for char in line:
                if current and self.draw.textlength(current+char,font=font)>(self.width-64)*SCALE:
                    wrapped.append(current); current=""
                current+=char
            wrapped.append(current)
        start=self.height-24-len(wrapped)*25
        self.rule(32,start-16,self.width-32)
        for i,line in enumerate(wrapped):
            self.text(32,start+i*25,line,16,SUB)
    def save(self,filename):
        self.image.save(HERE/filename)
