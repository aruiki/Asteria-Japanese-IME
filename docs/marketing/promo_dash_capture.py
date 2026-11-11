# 開いている「Asteria の設定」の窓を、指定の大きさにしてから PrintWindow で撮る(元の大きさに戻す)。
import ctypes, sys, time
from ctypes import wintypes
from PIL import Image
user32, gdi32 = ctypes.windll.user32, ctypes.windll.gdi32
user32.SetProcessDPIAware()
out, W, H = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
h = user32.FindWindowW(None, "Asteria の設定")
if not h: print("no window"); sys.exit(1)
was_min = user32.IsIconic(h)
if was_min: user32.ShowWindow(h, 9); time.sleep(1.5)
r0 = wintypes.RECT(); user32.GetWindowRect(h, ctypes.byref(r0))
user32.SetWindowPos(h, 0, r0.left, r0.top, W, H, 0x0014)
time.sleep(2.5)
class BMI(ctypes.Structure):
    _fields_ = [("biSize", wintypes.DWORD), ("biWidth", wintypes.LONG), ("biHeight", wintypes.LONG),
                ("biPlanes", wintypes.WORD), ("biBitCount", wintypes.WORD), ("biCompression", wintypes.DWORD),
                ("biSizeImage", wintypes.DWORD), ("biXPelsPerMeter", wintypes.LONG),
                ("biYPelsPerMeter", wintypes.LONG), ("biClrUsed", wintypes.DWORD), ("biClrImportant", wintypes.DWORD)]
r = wintypes.RECT(); user32.GetWindowRect(h, ctypes.byref(r))
w, hh = r.right - r.left, r.bottom - r.top
hdc = user32.GetWindowDC(h); mdc = gdi32.CreateCompatibleDC(hdc)
bmp = gdi32.CreateCompatibleBitmap(hdc, w, hh); gdi32.SelectObject(mdc, bmp)
user32.PrintWindow(h, mdc, 2)
bi = BMI(ctypes.sizeof(BMI), w, -hh, 1, 32, 0, 0, 0, 0, 0, 0)
buf = ctypes.create_string_buffer(w * hh * 4)
gdi32.GetDIBits(mdc, bmp, 0, hh, buf, ctypes.byref(bi), 0)
Image.frombuffer("RGBA", (w, hh), buf, "raw", "BGRA", 0, 1).convert("RGB").save(out)
user32.SetWindowPos(h, 0, r0.left, r0.top, r0.right - r0.left, r0.bottom - r0.top, 0x0014)
print("saved", out, w, hh)
if was_min: user32.ShowWindow(h, 6)
