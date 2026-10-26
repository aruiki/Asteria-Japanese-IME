"""gpu-local.html を Edge のヘッドレスで撮影して JPG にする。"""
from pathlib import Path
import subprocess
import sys
import tempfile
from PIL import Image

HERE = Path(__file__).resolve().parent
EDGE = Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe")

src = HERE / (sys.argv[1] if len(sys.argv) > 1 else "gpu-local.html")
out = src.with_suffix(".jpg")
with tempfile.TemporaryDirectory() as tmp:
    shot = Path(tmp) / "shot.png"
    subprocess.run([str(EDGE), "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=2", "--window-size=1600,900",
                    "--virtual-time-budget=3000", f"--user-data-dir={tmp}",
                    f"--screenshot={shot}", src.as_uri()], check=True, capture_output=True)
    image = Image.open(shot).convert("RGB")
image.save(out, quality=90, optimize=True, progressive=True)
print(out, image.size, out.stat().st_size)
