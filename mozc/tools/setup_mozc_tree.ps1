<#
.SYNOPSIS
Asteria のパッチを当てた Mozc の作業ツリーを作り、評価用のバイナリを名前を付けて保存する(2026-11-23)。

.DESCRIPTION
手で作ると、固定コミットの取り出し・パッチ・third_party の接合点・未追跡のデータ(モデル・短文語彙・連語)・
BAZEL_LLVM・runfiles の写しを毎回そろえることになる。この 1 本でそろえる。

  1. 作業ツリー: -MozcRepo(google/mozc の clone)から、ワークフローの MOZC_COMMIT を git worktree で取り出し、
     -AsteriaRef の mozc/patches/0001〜0003 を当てる。既にあれば作り直さない(-Recreate で作り直す)。
  2. third_party(llvm・msys64・ninja・qt)を -ThirdParty への接合点にする(数 GB を写さない)。
  3. src/data/kotori の未追跡のデータを -DataFrom から写す(同じドライブならハードリンク)。
  4. -Snapshot <名前>: converter_main と session_handler_main をビルドし、runfiles ごと -BinRoot\<名前> に写して
     Bazel のサーバーを止める(make_patches.sh の前に止める必要がある)。

例:
  # v5.2.1 のパッチの木を作って保存する
  powershell -File mozc/tools/setup_mozc_tree.ps1 -Name mz-v521 -AsteriaRef v5.2.1 -Snapshot v521
  # 既存の木(手で変えた後)をビルドして保存する
  powershell -File mozc/tools/setup_mozc_tree.ps1 -Name mz-p1 -Snapshot p3-try1
#>
param(
  [Parameter(Mandatory = $true)][string]$Name,
  [string]$AsteriaRef = 'origin/main',
  [string]$Root = 'D:\dev',
  [string]$MozcRepo = 'D:\dev\mz',
  [string]$ThirdParty = 'D:\dev\mz\src\third_party',
  [string]$DataFrom = 'D:\dev\mz\src\data\kotori',
  [string]$BinRoot = 'D:\dev\bins',
  [string]$DiskCache = 'D:/bazel-disk-cache-local',
  [string]$Snapshot = '',
  [switch]$Recreate
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$tree = Join-Path $Root $Name
$src = Join-Path $tree 'src'

function Invoke-Checked([string]$what, [scriptblock]$block) {
  & $block
  if ($LASTEXITCODE -ne 0) { throw "$what に失敗した(終了コード $LASTEXITCODE)" }
}

# --- 1. 作業ツリー ---
if ($Recreate -and (Test-Path $tree)) {
  Invoke-Checked 'worktree の削除' { git -C $MozcRepo worktree remove --force $tree }
}
if (-not (Test-Path $tree)) {
  $workflow = git -C $repo show "${AsteriaRef}:.github/workflows/mozc-windows.yml"
  if ($LASTEXITCODE -ne 0) { throw "$AsteriaRef のワークフローを読めない" }
  $commit = ($workflow | Select-String -Pattern '^\s*MOZC_COMMIT:\s*([0-9a-f]{40})').Matches[0].Groups[1].Value
  if (-not $commit) { throw 'MOZC_COMMIT が見つからない' }
  Write-Host "作業ツリー: $tree (google/mozc $commit + $AsteriaRef のパッチ)"
  Invoke-Checked 'worktree の作成' { git -C $MozcRepo worktree add --detach $tree $commit }
  $tmp = New-Item -ItemType Directory -Force (Join-Path ([IO.Path]::GetTempPath()) "asteria-patches-$Name")
  foreach ($p in '0001-kotori-branding.patch', '0002-kotori-lm-rerank.patch', '0003-kotori-bundle-model.patch') {
    $file = Join-Path $tmp $p
    # git show の出力をそのままのバイトで書く(PowerShell の文字列を通すと改行と文字コードが変わる)。
    cmd /c "git -C `"$repo`" show ${AsteriaRef}:mozc/patches/$p > `"$file`""
    if ($LASTEXITCODE -ne 0) { throw "$p を取り出せない" }
    Invoke-Checked "$p の適用" { git -C $tree apply --whitespace=nowarn $file }
    Write-Host "  当てた: $p"
  }
} else {
  Write-Host "作業ツリーは既にある: $tree"
}

# --- 2. third_party ---
$tp = Join-Path $src 'third_party'
New-Item -ItemType Directory -Force $tp | Out-Null
foreach ($d in 'llvm', 'msys64', 'ninja', 'qt') {
  $link = Join-Path $tp $d
  if (-not (Test-Path $link)) {
    New-Item -ItemType Junction -Path $link -Target (Join-Path $ThirdParty $d) | Out-Null
    Write-Host "  接合点: third_party\$d"
  }
}

# --- 3. 未追跡のデータ ---
$data = Join-Path $src 'data\kotori'
$tracked = @(git -C $tree ls-files -- 'src/data/kotori' | ForEach-Object { Split-Path $_ -Leaf })
$sameDrive = ([IO.Path]::GetPathRoot($data) -eq [IO.Path]::GetPathRoot($DataFrom))
foreach ($f in Get-ChildItem $DataFrom -File) {
  if ($tracked -contains $f.Name) { continue }
  if ($f.Name -eq 'native_generator.json' -or $f.Name -like 'zenz-v2.5-native-generator*') { continue }
  $dest = Join-Path $data $f.Name
  if (Test-Path $dest) { continue }
  if ($sameDrive) { New-Item -ItemType HardLink -Path $dest -Target $f.FullName | Out-Null }
  else { Copy-Item -LiteralPath $f.FullName -Destination $dest }
  Write-Host "  データ: $($f.Name)"
}

# --- 4. ビルドと保存 ---
if ($Snapshot) {
  $llvm = (Join-Path $ThirdParty 'llvm') -replace '\\', '/'
  Push-Location $src
  try {
    Invoke-Checked 'ビルド' {
      bazelisk build //converter:converter_main //session:session_handler_main --config release_build `
        "--repo_env=BAZEL_LLVM=$llvm" "--disk_cache=$DiskCache"
    }
    $out = Join-Path $BinRoot $Snapshot
    if (Test-Path $out) { Remove-Item -Recurse -Force $out }
    foreach ($t in @(@('converter', 'converter_main'), @('session', 'session_handler_main'))) {
      $dir = Join-Path $out $t[0]
      New-Item -ItemType Directory -Force $dir | Out-Null
      $bin = Join-Path $src "bazel-bin\$($t[0])\$($t[1]).exe"
      Copy-Item -LiteralPath $bin -Destination $dir
      # runfiles は記号リンクの木。robocopy で実体を写す(/SL を付けないので中身を写す)。
      robocopy "$bin.runfiles" (Join-Path $dir "$($t[1]).exe.runfiles") /E /NFL /NDL /NJH /NJS /NP | Out-Null
      if ($LASTEXITCODE -ge 8) { throw "runfiles の写しに失敗した($($t[1]))" }
    }
    $commit = git -C $repo rev-parse --short $AsteriaRef
    @{ name = $Snapshot; tree = $tree; asteria_ref = $AsteriaRef; asteria_commit = $commit;
       built = (Get-Date).ToString('s'); note = '作業ツリーに手の変更があれば asteria_commit とは一致しない' } |
      ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $out 'snapshot.json')
    Write-Host "保存した: $out"
  } finally {
    bazelisk shutdown 2>$null | Out-Null
    Pop-Location
  }
}
$global:LASTEXITCODE = 0
