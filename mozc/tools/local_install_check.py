"""利用者本人のPCで、候補MSIへの更新・実入力・元の版への復帰を段階的に確認する。

GitHubの使い捨てランナーが使えない間の手順（GOAL.md 2026-10-22、利用者が実PCでの導入を許可）。
普段使いのIMEを置き換えるので、各段階の前に元の版・プロファイル・言語設定を保存し、
rollbackで同じ版へ戻す。導入中のTIPは各アプリが読み込んでいるため、更新/復帰には
再起動が必要になり得る（終了コード3010）。その場合は再起動後に次の段階を実行する。
実入力する段階（verify/clickprobe/browser）は、直前に利用者の学習履歴を保存し、終わったら自動で
戻して全ファイルのSHA一致を記録する（検査の入力を利用者の学習に残さない）。

  python -X utf8 mozc/tools/local_install_check.py precheck --candidate C.msi --rollback R.msi --expected-dir D --out O
  python -X utf8 mozc/tools/local_install_check.py install  --out O
  python -X utf8 mozc/tools/local_install_check.py verify   --out O   # 実入力。キーボード/マウスに触らない
  python -X utf8 mozc/tools/local_install_check.py rollback --out O
  python -X utf8 mozc/tools/local_install_check.py restored --out O   # 復帰後の版/ファイル/プロファイル確認
"""
import argparse
import contextlib
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import installer_smoke as smoke  # noqa: E402  導入ログ・実入力・受入れ検査を共有する
import asteria_paths  # noqa: E402

# 導入先と保存先は版によって違う(v3.0.0 で Kotori → Asteria)。試験では PROFILE / INSTALL を差し替える。
INSTALL = None
# Windows専用の道具だが、試験(CIのLinux)でも読み込めるよう環境変数がなくても止めない。
PROFILE = None
CORE_FILES = ("mozc_server.exe", "mozc_renderer.exe", "mozc_tip64.dll", "mozc_tip32.dll")


def install_root():
    return INSTALL or asteria_paths.install_dir()


def profile():
    return PROFILE or asteria_paths.profile_dir()


def core_paths(directory):
    """主要4ファイルの実際の場所(IME の DLL は版ごとの名前のことがある)。"""
    directory = Path(directory)
    return {"mozc_server.exe": directory / "mozc_server.exe",
            "mozc_renderer.exe": directory / "mozc_renderer.exe",
            "mozc_tip64.dll": asteria_paths.tip_path(directory, "64"),
            "mozc_tip32.dll": asteria_paths.tip_path(directory, "32")}
TIP = "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}"


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def powershell(script, timeout=60):
    p = subprocess.run(["powershell", "-NoProfile", "-Command", script],
                       capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    if p.returncode:
        raise RuntimeError(f"PowerShell exit {p.returncode}: {p.stderr}")
    return p.stdout


def installed_products():
    """Asteriaの登録（ProductCode/版）。更新後に旧版が残っていないかも見る。"""
    script = ("[Console]::OutputEncoding=[Text.Encoding]::UTF8;"
              "Get-ItemProperty HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*,"
              "HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\* -ErrorAction SilentlyContinue"
              " | Where-Object { $_.DisplayName -match 'Asteria|Kotori' }"
              " | ForEach-Object { [pscustomobject]@{code=$_.PSChildName; name=$_.DisplayName; version=$_.DisplayVersion} }"
              " | ConvertTo-Json -Compress")
    text = powershell(script).strip()
    if not text:
        return []
    data = json.loads(text)
    return data if isinstance(data, list) else [data]


def language_list():
    script = ("[Console]::OutputEncoding=[Text.Encoding]::UTF8;"
              "Get-WinUserLanguageList | ForEach-Object { [pscustomobject]@{tag=$_.LanguageTag; tips=@($_.InputMethodTips)} }"
              " | ConvertTo-Json -Compress -Depth 3")
    data = json.loads(powershell(script).strip())
    return data if isinstance(data, list) else [data]


def is_runtime(path):
    # 実行中のserver/rendererが開くIPC・ロック・状態表示。利用者データではなく、開けないか常に変わる。
    path = Path(path)
    return path.suffix in (".ipc", ".lock") or path.name == "kotori_ai_status.txt"


def tree_hashes(root):
    root = Path(root)
    if not root.is_dir():
        return {}
    return {p.relative_to(root).as_posix(): sha256(p) for p in sorted(root.rglob("*"))
            if p.is_file() and not is_runtime(p)}


def copy_profile(source, destination):
    shutil.copytree(source, destination,
                    ignore=lambda folder, names: [n for n in names if is_runtime(n)])


def restore_profile(snapshot):
    """保存した内容へ戻す。サーバーが開いていると書き戻されるので、先に止めてから入れ替える。"""
    smoke.stop_installed_processes(install_root())
    if profile().is_dir():
        for path in profile().rglob("*"):
            if path.is_file() and not is_runtime(path):
                # 読み取り専用の履歴ファイルも消せるように(POSIX では読み取りの許可も残す)
                os.chmod(path, stat.S_IREAD | stat.S_IWRITE)
                path.unlink()
    shutil.copytree(snapshot, profile(), dirs_exist_ok=True)


@contextlib.contextmanager
def profile_restored_after(r, stage):
    """検査の入力（日本語・東京など）が利用者の学習履歴に残らないようにする。

    入力の直前にサーバーを止めて（ディスクへ書き終えた状態で）プロファイルを保存し、終わったら成否に
    かかわらず戻して全ファイルのSHAが一致することを確かめる。止めた時点でまだ書き出されていない学習は
    保存に入らない（Mozcは一定間隔で書き出す）。
    """
    smoke.stop_installed_processes(install_root())
    snapshot = r.out / f"profile-before-{stage}"
    if snapshot.exists():
        shutil.rmtree(snapshot)
    if profile().is_dir():
        copy_profile(profile(), snapshot)
    else:
        snapshot.mkdir(parents=True)
    expected = tree_hashes(snapshot)
    try:
        yield
    finally:
        restore_profile(snapshot)
        same = tree_hashes(profile()) == expected
        r.save(f"{stage}_profile", profile_restored=same, profile_files=len(expected))
        if not same:
            raise RuntimeError(f"{stage}の後に学習履歴を元へ戻せませんでした（保存先 {snapshot}）")


def core_hashes():
    return {name: sha256(path) for name, path in core_paths(install_root()).items() if path.is_file()}


class Run:
    def __init__(self, out):
        self.out = Path(out).resolve()
        self.path = self.out / "state.json"
        self.state = json.loads(self.path.read_text(encoding="utf-8")) if self.path.exists() else {}

    def save(self, stage, **values):
        self.state.setdefault("stages", []).append(
            {"stage": stage, "at": datetime.datetime.now().astimezone().isoformat(timespec="seconds"), **values})
        self.state.update(values)
        self.out.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.state, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"stage": stage, **values}, ensure_ascii=False, indent=2), flush=True)

    def require(self, stage):
        done = {s["stage"] for s in self.state.get("stages", [])}
        if stage not in done:
            raise RuntimeError(f"先に {stage} を実行してください")


def precheck(args):
    r = Run(args.out)
    if r.path.exists():
        raise RuntimeError(f"既に記録があります。新しい --out を使ってください: {r.path}")
    products = installed_products()
    if len(products) != 1:
        raise RuntimeError(f"導入済みAsteriaがちょうど1件ではありません: {products}")
    candidate, rollback = Path(args.candidate).resolve(), Path(args.rollback).resolve()
    info = {}
    for key, msi in (("candidate", candidate), ("rollback", rollback)):
        info[key] = {"path": str(msi), "sha256": sha256(msi),
                     "product_code": smoke.msi_property(msi, "ProductCode"),
                     "product_version": smoke.msi_property(msi, "ProductVersion")}
    if info["rollback"]["product_code"] != products[0]["code"]:
        raise RuntimeError("復帰用MSIが導入済みの版と一致しません: "
                           f"{info['rollback']['product_code']} != {products[0]['code']}")
    if info["candidate"]["product_code"] == products[0]["code"]:
        raise RuntimeError("候補MSIが導入済みと同じProductCodeです")
    expected = Path(args.expected_dir).resolve()
    expected_core = {name: sha256(path) for name, path in core_paths(expected).items()}
    r.save("precheck", baseline_product=products[0], msi=info, expected_dir=str(expected),
           expected_core=expected_core, baseline_core=core_hashes(),
           baseline_profile=tree_hashes(profile()), baseline_languages=language_list())


def install(args):
    r = Run(args.out)
    r.require("precheck")
    # 履歴は利用者の入力で常に変わるため、導入直前の内容を保存してそれを基準にする。
    backup = r.out / "profile-backup"
    if not backup.exists():
        copy_profile(profile(), backup)
    r.save("profile_backup", backup_profile=tree_hashes(backup))
    msi = Path(r.state["msi"]["candidate"]["path"])
    if sha256(msi) != r.state["msi"]["candidate"]["sha256"]:
        raise RuntimeError("候補MSIがprecheck後に変わりました")
    code = smoke.run_msi(["/i", str(msi)], r.out / "install.log")
    if code not in (0, 3010):
        r.save("install_failed", install_exit=code)
        raise RuntimeError(f"導入の終了コード: {code}")
    r.save("install", install_exit=code, install_reboot_required=code == 3010,
           products_after_install=installed_products())


def verify(args):
    r = Run(args.out)
    r.require("install")
    products = installed_products()
    want = r.state["msi"]["candidate"]
    errors = []
    if [p["code"] for p in products] != [want["product_code"]]:
        errors.append(f"登録が候補1件ではありません: {products}")
    core = core_hashes()
    if core != r.state["expected_core"]:
        errors.append("導入ファイルが候補と一致しません（3010なら再起動後に再実行）: " + json.dumps(core))
    profile_kept = all((profile() / name).is_file() for name in r.state["backup_profile"])
    if not profile_kept:
        errors.append("更新後に利用者プロファイルのファイルが欠けています")
    if errors:
        r.save("verify_failed", verify_errors=errors)
        raise RuntimeError("\n".join(errors))
    with profile_restored_after(r, "verify"):
        verify_input(r, args)


def verify_input(r, args):
    compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
    source = Path(__file__).resolve().parents[2] / "eval" / "imebench" / "ImeBench.cs"
    exe = r.out / "ImeBench.exe"
    smoke.run([str(compiler), "/nologo", "/target:winexe", "/platform:x64",
               "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
               "/out:" + str(exe), str(source)])
    cases = r.out / "input.tsv"
    cases.write_text("warm\tnihonngo\n0\tnihonngo\n1\ttoukyou\n", encoding="utf-8")
    committed = smoke.verify_input(exe, cases, r.out / "output.tsv")
    r.save("input", committed_text=committed)
    # ImeAcceptanceは普段のPCでの実行を拒否する。利用者の許可（GOAL.md 2026-10-22）をここでだけ伝える。
    os.environ["KOTORI_LOCAL_ACCEPTANCE"] = "owner-approved"
    if not args.skip_click:
        report = smoke.run_acceptance(compiler, source, install_root(), r.out, duplicate_stress=True)
        r.save("verify", acceptance_checks=len(report["checks"]),
               duplicate_trials=len(report["duplicate_stress"]["trials"]),
               scope="this PC, upgrade from installed baseline; EDIT/RichEdit only")
        return
    # クリック試験は未検証として残す。それ以外は CI と同じ基準で全件合格を要求する。
    os.environ["KOTORI_SKIP_CLICK"] = "environment"
    try:
        smoke.run_acceptance(compiler, source, install_root(), r.out, duplicate_stress=True)
    except RuntimeError as error:
        if "二重入力" not in str(error):
            raise
    report = json.loads((r.out / "acceptance.json").read_text(encoding="utf-8"))
    smoke.validate_acceptance(report, install_root())
    stress = report.get("duplicate_stress", {})
    trials = stress.get("trials", [])
    clicks = [t for t in trials if t.get("mode") == "click"]
    others = [t for t in trials if t.get("mode") != "click"]
    if (stress.get("complete") is not True or stress.get("click_skipped") is not True or len(trials) != 28 or
            len(clicks) != 2 or any(not t.get("skipped") for t in clicks) or
            any(t.get("passed") is not True for t in others)):
        r.save("verify_failed", verify_errors=["二重入力の実TSF検査（クリック以外）が未完了"])
        raise RuntimeError("二重入力の実TSF検査（クリック以外）が未完了")
    r.save("verify", acceptance_checks=len(report["checks"]), duplicate_trials_passed=len(others),
           click_trials="not verified: synthetic clicks drop the foreground window on this PC",
           scope="this PC, upgrade from installed baseline; EDIT/RichEdit only")


def clickprobe(args):
    """候補窓クリックの単独診断。合否は付けず、画像と時刻つきの観測を残す。"""
    r = Run(args.out)
    r.require("install")
    compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
    source = Path(__file__).resolve().parents[2] / "eval" / "imebench"
    exe, output = r.out / "ImeClickProbe.exe", r.out / "click-probe.json"
    smoke.run([str(compiler), "/nologo", "/target:winexe", "/platform:x64", "/main:Acceptance",
               "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
               "/reference:System.Web.Extensions.dll", "/out:" + str(exe),
               *(str(source / name) for name in ("ImeBench.cs", "ImeAcceptance.cs",
                                                 "ImeDuplicateStress.cs", "ImeClickProbe.cs"))])
    os.environ["KOTORI_LOCAL_ACCEPTANCE"] = "owner-approved"
    with profile_restored_after(r, "clickprobe"):
        process = subprocess.run([str(exe), str(install_root()), str(output), "--click-probe"], timeout=300)
    report = json.loads(output.read_text(encoding="utf-8")) if output.is_file() else {}
    r.save("clickprobe", clickprobe_exit=process.returncode,
           clickprobe_committed=[t.get("committed_by_click") for t in report.get("click_probe", [])])


def browser(args):
    """Edgeのtextarea/contenteditableで二重入力を確かめる（14件、--skip-clickでクリック2件は未検証）。"""
    r = Run(args.out)
    r.require("install")
    if not args.python:
        raise RuntimeError("--python にplaywright==1.58.0を入れたPythonを指定してください")
    compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
    source = Path(__file__).resolve().parents[2] / "eval" / "imebench"
    exe = r.out / "BrowserDuplicateDriver.exe"
    smoke.run([str(compiler), "/nologo", "/target:exe", "/platform:x64", "/main:BrowserDuplicateDriver",
               "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
               "/reference:System.Web.Extensions.dll", "/out:" + str(exe),
               *(str(source / name) for name in ("ImeBench.cs", "ImeAcceptance.cs",
                                                 "ImeDuplicateStress.cs", "BrowserDuplicateDriver.cs"))])
    os.environ["KOTORI_LOCAL_ACCEPTANCE"] = "owner-approved"
    if args.skip_click:
        os.environ["KOTORI_SKIP_CLICK"] = "environment"
    destination = r.out / "browser-duplicate"
    with profile_restored_after(r, "browser"):
        process = subprocess.run([args.python, "-X", "utf8",
                                  str(Path(__file__).with_name("browser_duplicate_probe.py")),
                                  "--driver", str(exe), "--install", str(install_root()), "--out", str(destination)],
                                 timeout=600)
    report = json.loads((destination / "summary.json").read_text(encoding="utf-8"))
    trials = report.get("trials", [])
    passed = [t for t in trials if t.get("passed") is True]
    skipped = [t for t in trials if t.get("skipped")]
    ok = (process.returncode == 0 and report.get("complete") is True and len(trials) == 14 and
          len(passed) + len(skipped) == 14 and all(t["mode"] == "click" for t in skipped) and
          (args.skip_click or not skipped))
    r.save("browser" if ok else "browser_failed", browser_passed=len(passed), browser_skipped=len(skipped),
           browser_error=report.get("error"), browser_scope="Edge temporary profile, textarea/contenteditable")
    if not ok:
        raise RuntimeError("Edgeの二重入力試験が未完了: " + str(report.get("error")))


def rollback(args):
    r = Run(args.out)
    r.require("precheck")
    want = r.state["msi"]
    products = installed_products()
    codes = [p["code"] for p in products]
    if want["candidate"]["product_code"] in codes:
        smoke.stop_installed_processes(install_root())
        code = smoke.run_msi(["/x", want["candidate"]["product_code"]], r.out / "uninstall.log")
        if code not in (0, 3010):
            r.save("rollback_failed", uninstall_exit=code)
            raise RuntimeError(f"候補の削除に失敗: {code}")
        r.save("uninstall", uninstall_exit=code, uninstall_reboot_required=code == 3010)
    if want["rollback"]["product_code"] not in [p["code"] for p in installed_products()]:
        msi = Path(want["rollback"]["path"])
        if sha256(msi) != want["rollback"]["sha256"]:
            raise RuntimeError("復帰用MSIがprecheck後に変わりました")
        code = smoke.run_msi(["/i", str(msi)], r.out / "rollback-install.log")
        if code not in (0, 3010):
            r.save("rollback_failed", rollback_install_exit=code)
            raise RuntimeError(f"元の版の導入に失敗: {code}")
        r.save("rollback_install", rollback_install_exit=code, rollback_reboot_required=code == 3010)
    # 検査の入力は各段階の後に戻してあるので、既定ではプロファイルに触れない（導入後に利用者が
    # 打った履歴を残す）。--restore-profile のときだけ導入直前の保存内容へ戻す。
    restored_profile = False
    backup = r.out / "profile-backup"
    if args.restore_profile and backup.exists():
        restore_profile(backup)
        restored_profile = tree_hashes(profile()) == r.state["backup_profile"]
    r.save("rollback", profile_restored=restored_profile)


def restored(args):
    r = Run(args.out)
    r.require("rollback")
    checks = {
        "product": [p["code"] for p in installed_products()] == [r.state["baseline_product"]["code"]],
        "core_files": core_hashes() == r.state["baseline_core"],
        "profile_files_kept": all((profile() / name).is_file() for name in r.state.get("backup_profile", {})),
        "languages": language_list() == r.state["baseline_languages"],
    }
    r.save("restored", restored_checks=checks, restored_all=all(checks.values()))
    if not all(checks.values()):
        raise RuntimeError(f"元の状態と違う項目があります（3010なら再起動後に再実行）: {checks}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("stage", choices=["precheck", "install", "verify", "clickprobe", "browser", "rollback", "restored"])
    ap.add_argument("--out", required=True)
    ap.add_argument("--candidate")
    ap.add_argument("--rollback")
    ap.add_argument("--expected-dir", help="候補MSIを管理展開した導入先のフォルダ(PFiles/Asteria)")
    ap.add_argument("--restore-profile", action="store_true", help="rollbackで導入直前のプロファイルへ戻す")
    ap.add_argument("--skip-click", action="store_true", help="verify/browserで候補クリック試験を未検証として残す")
    ap.add_argument("--python", help="browser段階でplaywrightを入れたPython")
    args = ap.parse_args()
    if sys.platform != "win32":
        raise RuntimeError("Windows専用です")
    if args.stage == "precheck" and not (args.candidate and args.rollback and args.expected_dir):
        ap.error("precheckには --candidate --rollback --expected-dir が必要です")
    globals()[args.stage](args)


if __name__ == "__main__":
    main()
