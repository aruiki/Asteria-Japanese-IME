# Asteria日本語入力の追加パッケージ「NVIDIA の高速化(CUDA)」を作る(docs/adr/0088)。
#
# llama.cpp b11259 の公式 CUDA 13.4 版から ggml-cuda.dll と CUDA の実行時ライブラリ(cudart・cuBLAS)を取り、
# 版の札(asteria-cuda.txt)と NOTICE を添えて AsteriaCuda64.msi にする。zip は版と SHA-256 を固定する。
# 本体の llama.cpp を上げたら、ここの版・URL・SHA-256 と rewriter/llama_runtime.cc の kLlamaBuild を
# 一緒に変え、docs/verification/cuda-backend-20261110.md と同じ手順で測り直す。
#
# 使い方: pwsh mozc/cuda-addon/build_addon.ps1 -Out AsteriaCuda64.msi [-Wix wix]
param(
  [string]$Out = 'AsteriaCuda64.msi',
  [string]$Wix = 'wix',
  [string]$Work = (Join-Path ([IO.Path]::GetTempPath()) 'asteria-cuda-addon')
)
$ErrorActionPreference = 'Stop'
# 進み具合の表示を出すと、Windows PowerShell 5.1 のダウンロードが何倍も遅くなる。
$ProgressPreference = 'SilentlyContinue'

$LlamaBuild = 'b11259'
# MSI の版: 1.<作り直しの回数>.<llama.cpp の版の番号>。llama.cpp が同じまま作り直すときは真ん中を上げる。
$AddonVersion = '1.0.11259'
$Base = "https://github.com/ggml-org/llama.cpp/releases/download/$LlamaBuild"
$Zips = @(
  @{ Name = "llama-$LlamaBuild-bin-win-cuda-13.4-x64.zip"
     Sha = '7e93d79ed0dfacb67a7a5448eab38b60511ec3ad623022259b08490d5cf01404'
     Files = @('ggml-cuda.dll') },
  @{ Name = 'cudart-llama-bin-win-cuda-13.4-x64.zip'
     Sha = '738f8c251ac22b70c3ae6f83a10cf222725df0395246a2cf58f32bdb85fbe668'
     Files = @('cudart64_13.dll', 'cublas64_13.dll', 'cublasLt64_13.dll') }
)

$here = $PSScriptRoot
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
$dl = New-Item -ItemType Directory -Force (Join-Path $Work 'dl')
$src = New-Item -ItemType Directory -Force (Join-Path $Work 'cuda')

foreach ($z in $Zips) {
  $zip = Join-Path $dl $z.Name
  Invoke-WebRequest -Uri "$Base/$($z.Name)" -OutFile $zip
  $got = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
  if ($got -ne $z.Sha) { throw "$($z.Name) の SHA-256 が違う: $got" }
  $x = Join-Path $dl ([IO.Path]::GetFileNameWithoutExtension($z.Name))
  Expand-Archive -Path $zip -DestinationPath $x
  foreach ($f in $z.Files) {
    $p = Join-Path $x $f
    if (-not (Test-Path $p)) { throw "$($z.Name) に $f がない" }
    Copy-Item $p $src
  }
}
# 本体は、この札が自分の llama.cpp の版と合うときだけ cuda フォルダを読む。
[IO.File]::WriteAllText((Join-Path $src 'asteria-cuda.txt'), "llama=$LlamaBuild`nversion=$AddonVersion`n",
  [Text.UTF8Encoding]::new($false))
Copy-Item (Join-Path $here 'NOTICE-cuda.txt') $src

& $Wix build -nologo -arch x64 -d "AddonVersion=$AddonVersion" -d "SrcDir=$src" `
  -o $Out (Join-Path $here 'AsteriaCuda.wxs')
if ($LASTEXITCODE -ne 0) { throw 'wix build に失敗した' }
Get-Item $Out | ForEach-Object { Write-Host "$($_.FullName) $([math]::Round($_.Length / 1MB)) MB" }
