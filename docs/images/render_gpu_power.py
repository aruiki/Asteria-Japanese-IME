"""gpu-power.html を Edge のヘッドレスで撮影し、README 用の gpu-power.jpg を作る。"""
from pathlib import Path
import subprocess
import tempfile
from PIL import Image

HERE = Path(__file__).resolve().parent
EDGE = Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe")


def main():
    with tempfile.TemporaryDirectory() as tmp:
        shot = Path(tmp) / "shot.png"
        subprocess.run([
            str(EDGE), "--headless=new", "--disable-gpu", "--hide-scrollbars",
            "--force-device-scale-factor=2", "--window-size=1600,900",
            "--virtual-time-budget=3000", f"--user-data-dir={tmp}",
            f"--screenshot={shot}", (HERE / "gpu-power.html").as_uri(),
        ], check=True, capture_output=True)
        image = Image.open(shot).convert("RGB")
    image.save(HERE / "gpu-power.jpg", quality=90, optimize=True, progressive=True)
    print(image.size, (HERE / "gpu-power.jpg").stat().st_size)


if __name__ == "__main__":
    main()
