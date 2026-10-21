"""GitHubの使い捨てWindowsランナーで、MSIの導入・実入力・削除を確認する。

開発者の普段のIMEを変更しないため、GitHub Actions外では実行を拒否する。
既存のKotoriがあるランナーでも実行しない。結果とMSIログを出力先へ保存する。
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def run(command, timeout=180):
    p = subprocess.run(command, capture_output=True, timeout=timeout)
    if p.returncode:
        raise RuntimeError(f"exit {p.returncode}: {command[0]}\n" +
                           p.stdout.decode("utf-8", "replace") +
                           p.stderr.decode("utf-8", "replace"))
    return p


def msi_property(msi, name):
    # MSI のファイルパスを /x に渡すと、Windows Installer が再び媒体を
    # 開こうとして時間がかかることがある。ProductCodeで削除する。
    script = ("$wi=New-Object -ComObject WindowsInstaller.Installer;"
              "$db=$wi.GetType().InvokeMember('OpenDatabase','InvokeMethod',$null,$wi,@('"
              + str(msi).replace("'", "''") + "',0));"
              "$v=$db.OpenView(\"SELECT Value FROM Property WHERE Property='"
              + name + "'\");$v.Execute();$r=$v.Fetch();$r.StringData(1)")
    p = subprocess.run(["powershell", "-NoProfile", "-Command", script],
                       capture_output=True, text=True, check=True, timeout=30)
    value = p.stdout.strip()
    if not value:
        raise RuntimeError(f"MSIの{name}が空です")
    return value


def run_msi(arguments, log):
    """ランナーが停止しても原因の手掛かりが残るようMSIの段階を逐次記録する。"""
    # CIのホストもTIPを読み込み得るため、Restart Managerにホスト終了を依頼しない。
    # 使用中ファイルの削除失敗・再起動要求は通常どおり結果に記録する。
    process = subprocess.Popen(["msiexec", *arguments, "/qn", "/norestart",
                                "MSIRESTARTMANAGERCONTROL=Disable",
                                "/L*vx!", str(log)])
    start = time.monotonic()
    offset = 0
    try:
        while True:
            if log.exists():
                with log.open("r", encoding="utf-16", errors="replace") as stream:
                    stream.seek(offset)
                    for line in stream:
                        if any(key in line for key in ("Action start", "Action ended",
                                "RESTART MANAGER", "Restart", "Return value 3",
                                "MainEngineThread")):
                            print(line.rstrip(), flush=True)
                    offset = stream.tell()
            code = process.poll()
            if code is not None:
                return code
            if time.monotonic() - start > 300:
                raise subprocess.TimeoutExpired("msiexec", 300)
            time.sleep(1)
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=15)


def stop_installed_processes(install):
    # この検証で導入したフォルダのプロセスだけ。開発PCではmainのguardが拒否する。
    script = "$root = [IO.Path]::GetFullPath($env:KOTORI_SMOKE_INSTALL) + '\\'; " + \
             "Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force"
    env = dict(os.environ, KOTORI_SMOKE_INSTALL=str(install))
    subprocess.run(["powershell", "-NoProfile", "-Command", script], env=env,
                   capture_output=True, timeout=30, check=True)


def verify_input(exe, cases, output):
    run([str(exe), "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
         "{455A87E7-275F-4582-92CB-6600917A436D}", str(cases), str(output), "3000"])
    error = Path(str(output) + ".error.txt")
    if error.exists():
        raise RuntimeError(error.read_text(encoding="utf-8-sig"))
    got = dict(line.split("\t", 1) for line in output.read_text(encoding="utf-8-sig").splitlines()
               if "\t" in line)
    if got.get("0") != "日本語" or got.get("1") != "東京":
        raise RuntimeError(f"実入力の結果が一致しません: {got}")
    return got


ACCEPTANCE_CHECKS = {
    "plain_preedit_space_commit", "richedit_preedit_space_commit",
    "candidate_down_page_roundtrip_commit", "cancel_and_resume",
    "backspace_and_continue", "f6_to_f10", "focus_between_ordinary_fields",
    "password_host_visible_behavior",
}


def validate_acceptance(report, install):
    """空の結果・一部だけの成功・違うTIPを受け入れない。内部privacyの証明ではない。"""
    checks = report.get("checks", [])
    if (report.get("complete") is not True or report.get("native_tsf") is not True or
            report.get("error") or len(checks) != len(ACCEPTANCE_CHECKS) or
            {c.get("name") for c in checks} != ACCEPTANCE_CHECKS or
            any(c.get("passed") is not True for c in checks)):
        raise RuntimeError("実入力の追加検証が未完了: " + json.dumps(report, ensure_ascii=False))
    expected = (install / "mozc_tip64.dll").resolve()
    if Path(report.get("loaded_tip", "")).resolve() != expected:
        raise RuntimeError("入力ホストが導入したTIPを読み込んでいません")
    if report.get("secure_field_internal_ai_or_learning_verified") is not False:
        raise RuntimeError("保護欄の内部AI/学習を、この表示試験で検証済みにしてはいけません")


def validate_duplicate_stress(report):
    stress = report.get("duplicate_stress", {})
    trials = stress.get("trials", [])
    expected = {(host, mode, delay) for host in ("EDIT", "RichEdit")
                for mode in ("space_enter", "enter", "down_enter", "focus")
                for delay in (0, 30, 250)}
    expected.update((host, mode, 0) for host in ("EDIT", "RichEdit")
                    for mode in ("click", "intentional_repeat"))
    actual = {(row.get("host"), row.get("mode"), row.get("space_to_action_ms", 0))
              for row in trials}
    if (stress.get("complete") is not True or len(trials) != len(expected) or
            actual != expected or any(row.get("passed") is not True for row in trials)):
        raise RuntimeError("二重入力の実TSF検査が未完了: " + json.dumps(stress, ensure_ascii=False))


def run_acceptance(compiler, source, install, out, duplicate_stress=False):
    exe, output = out / "ImeAcceptance.exe", out / "acceptance.json"
    run([str(compiler), "/nologo", "/target:winexe", "/platform:x64", "/main:Acceptance",
         "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
         "/reference:System.Web.Extensions.dll", "/out:" + str(exe),
         str(source), str(source.with_name("ImeAcceptance.cs")),
         str(source.with_name("ImeDuplicateStress.cs"))])
    arguments = [str(exe), str(install), str(output)]
    if duplicate_stress:
        arguments.append("--duplicate-stress")
    process = subprocess.run(arguments, capture_output=True, timeout=360 if duplicate_stress else 180)
    if not output.is_file():
        raise RuntimeError(f"実入力レポートがありません: exit {process.returncode}")
    report = json.loads(output.read_text(encoding="utf-8"))
    validate_acceptance(report, install)
    if duplicate_stress:
        validate_duplicate_stress(report)
    if process.returncode != 0:
        raise RuntimeError(f"実入力検証の終了コード: {process.returncode}")
    return report


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--msi", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--acceptance", action="store_true", help="EDIT/RichEditの候補・取消・フォーカスを追加確認する")
    ap.add_argument("--duplicate-stress", action="store_true", help="確定/候補クリック/フォーカスの二重挿入を実TSFで検査する")
    ap.add_argument("--reinstall", action="store_true", help="削除後、同じMSIを再導入して入力する（再起動はしない）")
    args = ap.parse_args()
    if (sys.platform != "win32" or os.environ.get("GITHUB_ACTIONS") != "true" or
            os.environ.get("KOTORI_DISPOSABLE_RUNNER") != "github-hosted"):
        raise RuntimeError("使い捨てのGitHub Actions Windowsランナーでのみ実行できます")
    install = Path(os.environ["ProgramFiles(x86)"]) / "Kotori"
    if install.exists():
        raise RuntimeError("既存のKotoriがあるので停止します")
    msi, out = args.msi.resolve(), args.out.resolve()
    if not msi.is_file():
        raise FileNotFoundError(msi)
    out.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).resolve().parents[2] / "eval" / "imebench" / "ImeBench.cs"
    # __file__ は repo/mozc/tools。リポジトリ直下は parents[2]。
    with msi.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    result = {"msi": msi.name, "sha256": digest,
              "source_commit": os.environ.get("GITHUB_SHA"),
              "run_id": os.environ.get("GITHUB_RUN_ID"),
              "install": False, "input": False, "uninstall": False}
    errors = []
    result["product_code"] = msi_property(msi, "ProductCode")
    result["product_version"] = msi_property(msi, "ProductVersion")

    def checkpoint(stage):
        result["stage"] = stage
        result["errors"] = errors
        text = json.dumps(result, ensure_ascii=False, indent=2)
        (out / "result.json").write_text(text, encoding="utf-8")
        print(text, flush=True)

    attempted = False
    try:
        attempted = True
        checkpoint("installing")
        code = run_msi(["/i", str(msi)], out / "install.log")
        result["install_exit"] = code
        if code != 0:
            raise RuntimeError(f"導入の終了コード: {code}（3010は再起動が必要）")
        for name in ("mozc_server.exe", "mozc_renderer.exe", "mozc_tip64.dll"):
            if not (install / name).is_file():
                raise RuntimeError(f"導入後のファイルがありません: {name}")
        result["install"] = True
        result["installed_sha256"] = {
            name: hashlib.sha256((install / name).read_bytes()).hexdigest()
            for name in ("mozc_server.exe", "mozc_renderer.exe", "mozc_tip64.dll")
        }
        checkpoint("installed")
        exe = out / "ImeBench.exe"
        compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
        run([str(compiler), "/nologo", "/target:winexe", "/platform:x64",
             "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
             "/out:" + str(exe), str(source)])
        cases = out / "input.tsv"
        cases.write_text("warm\tnihonngo\n0\tnihonngo\n1\ttoukyou\n", encoding="utf-8")
        output = out / "output.tsv"
        result["committed_text"] = verify_input(exe, cases, output)
        result["input"] = True
        checkpoint("input_verified")
        if args.acceptance or args.duplicate_stress:
            result["acceptance"] = run_acceptance(compiler, source, install, out, args.duplicate_stress)
            checkpoint("acceptance_verified")
        if args.reinstall:
            stop_installed_processes(install)
            code = run_msi(["/x", result["product_code"]], out / "before-reinstall-uninstall.log")
            result["before_reinstall_uninstall_exit"] = code
            result["before_reinstall_reboot_required"] = code == 3010
            if code not in (0, 3010) or (install / "mozc_tip64.dll").exists():
                raise RuntimeError(f"再導入前の削除に失敗: {code}")
            checkpoint("reinstalling")
            code = run_msi(["/i", str(msi)], out / "reinstall.log")
            result["reinstall_exit"] = code
            if code != 0:
                raise RuntimeError(f"再導入の終了コード: {code}")
            for name, expected in result["installed_sha256"].items():
                if hashlib.sha256((install / name).read_bytes()).hexdigest() != expected:
                    raise RuntimeError(f"再導入後のファイルが違います: {name}")
            result["reinstalled_committed_text"] = verify_input(exe, cases, out / "reinstalled-output.tsv")
            result["reinstall"] = True
            result["reinstall_scope"] = "same MSI, same boot; reboot recovery and previous-version upgrade are unverified"
            checkpoint("reinstall_verified")
    except Exception as exc:
        errors.append(str(exc))
    finally:
        if attempted:
            try:
                checkpoint("uninstalling")
                # この検証で入れたフォルダのプロセスだけを停止し、削除時のロックを外す。
                stop_installed_processes(install)
                code = run_msi(["/x", result["product_code"]], out / "uninstall.log")
                result["uninstall_exit"] = code
                result["reboot_required"] = code == 3010
                result["uninstall"] = code in (0, 3010) and not (install / "mozc_tip64.dll").exists()
                if not result["uninstall"]:
                    errors.append("アンインストールの完了を確認できません")
            except Exception as exc:
                errors.append("削除確認: " + str(exc))
        checkpoint("finished")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
