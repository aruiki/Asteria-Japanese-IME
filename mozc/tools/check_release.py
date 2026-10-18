"""公開した MSI を確かめる(作業カード 35)。リリースの後に必ず回す。

GitHub Releases から `Asteria64.msi`（旧版は `Kotori64.msi`） と `.sha256` を取り、次を確かめる。1 つでも外れたら終了コード 1。
- SHA-256 が `.sha256` と一致する
- MSI の大きさが 1.8 GiB 以下(GitHub Releases の 1 ファイルの上限 2 GiB の手前で気づく。カード 40)
- MSI の ProductVersion が、比べる版(既定は 1 つ前のリリース)より大きい(上書きで入れ替わるための条件)
- 展開した中身に、同梱するモデルと実行ファイルと第三者の表示がそろっていて、入れない物(通信の部品)がない
- `mozc_server.exe` のファイルの版が ProductVersion と一致する

使い方: python mozc/tools/check_release.py [タグ(既定は Latest)] [--work フォルダ]
       python mozc/tools/check_release.py --local <MSI>(手元でビルドした MSI の中身だけを確かめる)
  公開済みMSIからの更新も確認: --local <候補MSI> --previous-local <公開済みMSI>
  gh(GitHub CLI)と Windows の msiexec を使う。
"""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys

REPO = "aruiki/Asteria-Japanese-IME"
BUNDLED = ["mozc_server.exe", "mozc_tool.exe", "mozc_renderer.exe", "mozc_tip64.dll", "llama.dll",
           "zenz-v2.5-small-q8_0.gguf", "zenz-v2.5-medium-q8_0.gguf", "tinyswallow-1.5b-q5_k_m.gguf",
           "NOTICE-zenz.txt", "NOTICE-tinyswallow.txt",
           "KotoriEnglish.tsv", "KotoriEnglish-source.json", "NOTICE-katakana-english.txt",
           # 第三者の表示(作業カード 41、REQ-16-1)
           "NOTICE-third-party.txt", "LICENSE-llama.cpp.txt", "LICENSE-llvm-openmp.txt", "LICENSE-GPL-3.0.txt",
           "documents/credits_en.html"]
# 入れない物。ggml-rpc.dll は別の PC に計算を任せる通信の部品(REQ-15-1、作業カード 41)。
ABSENT = ["ggml-rpc.dll"]
MAX_MSI_BYTES = int(1.8 * 2**30)
RELEASE_LIST_LIMIT = 100


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, check=True, **kw).stdout.strip()


def ps(script: str) -> str:
    return run(["powershell", "-NoProfile", "-Command", script])


def msi_version(msi: pathlib.Path) -> str:
    return ps(
        "$wi = New-Object -ComObject WindowsInstaller.Installer;"
        f"$db = $wi.GetType().InvokeMember('OpenDatabase','InvokeMethod',$null,$wi,@('{msi}',0));"
        "$v = $db.GetType().InvokeMember('OpenView','InvokeMethod',$null,$db,"
        "@(\"SELECT Value FROM Property WHERE Property='ProductVersion'\"));"
        "$v.GetType().InvokeMember('Execute','InvokeMethod',$null,$v,$null);"
        "$r = $v.GetType().InvokeMember('Fetch','InvokeMethod',$null,$v,$null);"
        "$r.GetType().InvokeMember('StringData','GetProperty',$null,$r,1)")


def choose_asset_name(names):
    """新しい配布名を優先し、旧版の添付名をそのまま確認する。"""
    for name in ("Asteria64.msi", "Kotori64.msi"):
        if name in names and name + ".sha256" in names:
            return name
    raise ValueError("MSIと対応するsha256の添付がない")


def download(tag: str, dest: pathlib.Path) -> pathlib.Path:
    dest.mkdir(parents=True, exist_ok=True)
    release = json.loads(run(["gh", "release", "view", tag, "--repo", REPO,
                              "--json", "assets"]))
    name = choose_asset_name([asset["name"] for asset in release["assets"]])
    run(["gh", "release", "download", tag, "--repo", REPO, "--pattern", name + "*",
         "--dir", str(dest), "--clobber"])
    return dest / name


def as_tuple(v: str):
    return tuple(int(x) for x in v.split("."))


def product_release_pair(releases, requested="", listing_truncated=False):
    """モデル専用releaseとdraftを除き、製品タグ同士でMSIの版を比較する。"""
    pattern = re.compile(r"v\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?")
    products = [r for r in releases
                if not r.get("isDraft", False) and pattern.fullmatch(r["tagName"])]
    tag = requested or next((r["tagName"] for r in products if r.get("isLatest")), "")
    tags = [r["tagName"] for r in products]
    if not tag or tag not in tags:
        raise ValueError("公開済みの製品タグが取得一覧にない。モデル専用release/draftはMSI検査の対象外")
    index = tags.index(tag)
    previous = tags[index + 1] if index + 1 < len(tags) else ""
    if not previous and listing_truncated:
        raise ValueError("取得範囲の末尾に達したため、前の製品releaseがないとは確認できない")
    return tag, previous


class Checks:
    """確かめた結果を OK / NG で出し、1 つでも NG があったかを覚える。"""

    def __init__(self):
        self.ok = True

    def __call__(self, cond: bool, msg: str):
        print(("OK  " if cond else "NG  ") + msg)
        self.ok = self.ok and cond


def check_generator(root, check, required=False, identity=None):
    """同梱モデルの自己申告を信用せず、検証用ソースの固定SHAと照合する。"""
    if identity is None:
        manifest = pathlib.Path(__file__).resolve().parents[2] / "training/contrastive/native_generator.json"
        identity = json.loads(manifest.read_text(encoding="utf-8"))
    model = root / identity["filename"]
    metadata = root / "native_generator.json"
    if not required and not model.exists() and not metadata.exists():
        return  # 追加モデル導入前の公開版も検証可能にする。
    check(model.is_file(), "同梱: " + model.name)
    check(metadata.is_file(), "同梱: native_generator.json")
    if metadata.is_file():
        try:
            packaged = json.loads(metadata.read_text(encoding="utf-8"))
        except (ValueError, UnicodeError):
            packaged = None
        check(packaged == identity, "追加生成モデルの固定情報が検証ソースと一致")
    models = {identity["filename"]: identity["sha256"], **identity.get("base_models", {})}
    for name, expected in models.items():
        path = root / name
        check(path.is_file(), "固定モデルの同梱: " + name)
        if path.is_file():
            with path.open("rb") as stream:
                got = hashlib.file_digest(stream, "sha256").hexdigest()
            check(got == expected, "モデルSHA-256が評価対象と一致: " + name)


def check_collocations(root, check, required=False, identity=None):
    """原データとmanifestを同時に差し替えても固定した採用データとは認めない。"""
    if identity is None:
        path = pathlib.Path(__file__).resolve().parents[2] / 'training/collocations/sources.json'
        identity = json.loads(path.read_text(encoding='utf-8'))
    names = ('fast_collocations.txt', 'fast_collocations.json', 'NOTICE-fast-collocations.txt')
    if not required and not any((root / name).exists() for name in names):
        return  # 導入前の公開版を引き続き検証する。
    for name in names:
        check((root / name).is_file(), '連語データ同梱: ' + name)
    if not all((root / name).is_file() for name in names):
        return
    data = (root / names[0]).read_bytes()
    expected = identity['result_sha256']
    check(hashlib.sha256(data).hexdigest() == expected, '連語SHA-256が比較対象と一致')
    try:
        metadata = json.loads((root / names[1]).read_text(encoding='utf-8'))
        valid = (metadata['result_sha256'] == expected and
                 metadata['filename'] == names[0] and
                 metadata['source'] == identity['source'] and
                 metadata['revision'] == identity['revision'] and
                 metadata['corpus_sha256'] == identity['sample_sha256'] and
                 metadata['evaluation_labels_used'] is False and
                 metadata['combined_entries'] == len(data.splitlines()))
        notice = (root / names[2]).read_text(encoding='utf-8')
        attribution = all(value in notice for value in
                          ('CC BY-SA 4.0', identity['source'], identity['revision'], expected))
        if 'usage_source' in identity:
            usage = identity['usage_source']
            valid = valid and metadata.get('usage_source') == usage
            attribution = attribution and all(value in notice for value in
                (usage['repository'], usage['tag'], usage['usage_sha256'], usage['cforms_sha256'],
                 'BSD-2-Clause', 'Hiroyuki Komatsu', 'Shoji Ogura', 'Tsuyoshi Horo'))
    except (ValueError, KeyError, TypeError, UnicodeError):
        valid = attribution = False
    check(valid, '連語manifestの出典・学習区間・結果SHAが固定情報と一致')
    check(attribution, '連語の帰属・ライセンス・変更対象の表示')


def check_contents(msi: pathlib.Path, work: pathlib.Path, check: Checks, require_generator=False,
                   require_collocations=False):
    """MSI の大きさと、展開した同梱物・入れない物・実行ファイルの版を確かめる。"""
    size = msi.stat().st_size
    check(size <= MAX_MSI_BYTES, f"大きさ {size / 2**30:.2f} GiB ≦ {MAX_MSI_BYTES / 2**30:.1f} GiB(上限 2 GiB)")
    ver = msi_version(msi)
    ext = work / "ext"
    shutil.rmtree(ext, ignore_errors=True)
    subprocess.run(["msiexec", "/a", str(msi), "/qn", f"TARGETDIR={ext}"], check=True)
    root = ext / "PFiles" / "Kotori"
    for name in BUNDLED:
        check((root / name).exists(), f"同梱: {name}")
    check_generator(root, check, require_generator)
    check_collocations(root, check, require_collocations)
    for name in ABSENT:
        check(not (root / name).exists(), f"入れない: {name}")
    fv = ps(f"(Get-Item '{root / 'mozc_server.exe'}').VersionInfo.FileVersion")
    check(fv == ver, f"mozc_server.exe の版 {fv} = ProductVersion")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("tag", nargs="?", default="")
    ap.add_argument("--work", default=str(pathlib.Path.home() / "kotori-dev" / "work" / "check-release"))
    ap.add_argument("--local", default="", help="手元の MSI の中身だけを確かめる(ハッシュと前の版は見ない)")
    ap.add_argument("--previous-local", default="", help="比較する公開済みMSI。--localと併用し、ProductVersionの先頭3項目で更新可能か確認する")
    ap.add_argument("--require-generator", action="store_true", help="新リリースの学習済み追加モデルを必須とする")
    ap.add_argument("--require-collocations", action="store_true", help="固定した初期連語と出典の同梱を必須とする")
    args = ap.parse_args()
    if args.previous_local and not args.local:
        ap.error("--previous-local requires --local")
    check = Checks()
    work = pathlib.Path(args.work).resolve()
    if args.local:
        msi = pathlib.Path(args.local).resolve()
        if args.previous_local:
            ver = msi_version(msi)
            previous = msi_version(pathlib.Path(args.previous_local).resolve())
            check(as_tuple(ver)[:3] > as_tuple(previous)[:3],
                  f"ProductVersion {ver} > 公開済みMSI({previous})(先頭 3 つで比べる)")
        if check.ok:
            check_contents(msi, work, check, args.require_generator,
                           args.require_collocations)
    else:
        releases = json.loads(run(["gh", "release", "list", "--repo", REPO,
                                   "--limit", str(RELEASE_LIST_LIMIT),
                                   "--json", "tagName,isLatest,isDraft"]))
        tag, prev = product_release_pair(releases, args.tag,
                                         listing_truncated=len(releases) == RELEASE_LIST_LIMIT)
        shutil.rmtree(work, ignore_errors=True)
        msi = download(tag, work / tag)
        want = msi.with_name(msi.name + ".sha256").read_text().split()[0]
        got = hashlib.sha256(msi.read_bytes()).hexdigest()
        check(got == want, f"SHA-256 {got[:16]}…")
        ver = msi_version(msi)
        if prev:
            prev_ver = msi_version(download(prev, work / prev))
            check(as_tuple(ver)[:3] > as_tuple(prev_ver)[:3],
                  f"ProductVersion {ver} > {prev}({prev_ver})(先頭 3 つで比べる)")
        else:
            print(f"--  ProductVersion {ver}(比べる前の版がない)")
        check_contents(msi, work, check, args.require_generator, args.require_collocations)
    print("すべて OK" if check.ok else "確かめに失敗した項目がある")
    return 0 if check.ok else 1


if __name__ == "__main__":
    sys.exit(main())
