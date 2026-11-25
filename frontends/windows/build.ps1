# Asteria の Windows 版を作り、配布フォルダ・MSI・統合インストーラ (Asteria-Setup.exe) にまとめる(docs/SPEC.md 16.2, ADR 0072/0088/0091)。
#   ./frontends/windows/build.ps1 [-OutDir dist] [-Dict target/asteria/system.dict] [-BuildMsi] [-BuildBundle] [-MsiVersion 5.5.0] [-MsiOut dist/Asteria64.msi] [-BundleOut dist/Asteria-Setup.exe] [-RuntimeSource <dir>] [-ModelSource <dir>] [-CudaSource <dir>]
param(
  [string] $OutDir = 'dist',
  [string] $Dict = 'target/asteria/system.dict',
  [switch] $BuildMsi,
  [switch] $BuildBundle,
  [string] $MsiVersion = '5.5.0',
  [string] $MsiOut = '',
  [string] $BundleOut = '',
  [string] $RuntimeSource = '',
  [string] $ModelSource = '',
  [string] $CudaSource = ''
)
$ErrorActionPreference = 'Stop'
if ($BuildBundle) { $BuildMsi = $true }
$msiCore = ($MsiVersion -split '[-+]')[0]
$msiVer = if ($msiCore -match '^\d+\.\d+\.\d+$') { $msiCore } else { '5.5.0' }

$root = (Resolve-Path "$PSScriptRoot/../..").Path
Push-Location $root
try {
  function Invoke-Checked([string] $File, [string[]] $Arguments) {
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File $Arguments が $LASTEXITCODE で失敗した" }
  }

  # DLL・各 EXE とも VC ランタイムを静的にリンクする(/MT、docs/adr/0009)。
  $env:RUSTFLAGS = '-C target-feature=+crt-static'
  try {
    Invoke-Checked cargo @('build', '--release', '--locked', '-p', 'asteria-server', '-p', 'asteria-renderer', '-p', 'asteria-config', '-p', 'asteria-msi-action', '--target', 'x86_64-pc-windows-msvc')
    $targets = @('x86_64-pc-windows-msvc', 'i686-pc-windows-msvc')
    foreach ($t in $targets) {
      Invoke-Checked cargo @('build', '--release', '--locked', '-p', 'asteria-tip-win', '--target', $t)
      Invoke-Checked cargo @('test', '--locked', '-p', 'asteria-tip-win', '--target', $t)
    }
  } finally {
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
  }

  New-Item -ItemType Directory -Force "$OutDir/x86", "$OutDir/data", "$OutDir/runtime" | Out-Null
  Copy-Item target/x86_64-pc-windows-msvc/release/asteria-server.exe $OutDir
  Copy-Item target/x86_64-pc-windows-msvc/release/asteria-renderer.exe $OutDir
  Copy-Item target/x86_64-pc-windows-msvc/release/asteria-config.exe $OutDir
  Copy-Item target/x86_64-pc-windows-msvc/release/asteria_msi_action.dll $OutDir
  # 版ごとの名前 (docs/adr/0072)。古い版や無印の DLL が残らないように入れ替える。
  Remove-Item "$OutDir/asteria_tip.dll", "$OutDir/asteria_tip64-*.dll" -Force -ErrorAction SilentlyContinue
  Remove-Item "$OutDir/x86/asteria_tip.dll", "$OutDir/x86/asteria_tip32-*.dll" -Force -ErrorAction SilentlyContinue
  Copy-Item target/x86_64-pc-windows-msvc/release/asteria_tip.dll "$OutDir/asteria_tip64-$msiVer.dll"
  Copy-Item target/i686-pc-windows-msvc/release/asteria_tip.dll "$OutDir/x86/asteria_tip32-$msiVer.dll"
  if (Test-Path $Dict) {
    Copy-Item $Dict "$OutDir/data/system.dict"
  } else {
    Write-Warning "辞書 $Dict がない。先に just dict で作り、$OutDir\data\system.dict に置く"
  }

  # GPU/CPU 動的 llama.cpp DLL・モデル・補助語彙のステージング ($OutDir/runtime へ配置 -> MSI で INSTALLDIR 直下へ展開)
  $rtSrc = if ($RuntimeSource) { $RuntimeSource } elseif (Test-Path 'C:\Program Files (x86)\Asteria\llama.dll') { 'C:\Program Files (x86)\Asteria' } else { '' }
  $mdlSrc = if ($ModelSource) { $ModelSource } elseif (Test-Path 'C:\Program Files (x86)\Asteria\genmai-310M-q8_0.gguf') { 'C:\Program Files (x86)\Asteria' } elseif (Test-Path 'target/kotori') { 'target/kotori' } else { '' }

  $runtimePatterns = @(
    'llama.dll', 'ggml.dll', 'ggml-base.dll', 'ggml-vulkan.dll', 'ggml-cpu-*.dll',
    'libomp.dll', 'vcruntime140*.dll', 'msvcp140*.dll',
    'fast_collocations.txt', 'fast_collocations.json', 'NOTICE-fast-collocations.txt',
    'short_lexicon.tsv', 'short_lexicon.json', 'NOTICE-short-lexicon.txt',
    'NOTICE-katakana-english.txt', 'LICENSE-llama.cpp.txt', 'LICENSE-llvm-openmp.txt'
  )
  if ($rtSrc) {
    foreach ($pat in $runtimePatterns) {
      Get-ChildItem -Path $rtSrc -Filter $pat -File -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item $_.FullName "$OutDir/runtime/$($_.Name)" -Force
      }
    }
    if (Test-Path "$rtSrc/AsteriaEnglish.tsv") {
      Copy-Item "$rtSrc/AsteriaEnglish.tsv" "$OutDir/runtime/AsteriaEnglish.tsv" -Force
    } elseif (Test-Path "$rtSrc/KotoriEnglish.tsv") {
      Copy-Item "$rtSrc/KotoriEnglish.tsv" "$OutDir/runtime/AsteriaEnglish.tsv" -Force
    }
    if (Test-Path "$rtSrc/AsteriaEnglish-source.json") {
      Copy-Item "$rtSrc/AsteriaEnglish-source.json" "$OutDir/runtime/AsteriaEnglish-source.json" -Force
    } elseif (Test-Path "$rtSrc/KotoriEnglish-source.json") {
      Copy-Item "$rtSrc/KotoriEnglish-source.json" "$OutDir/runtime/AsteriaEnglish-source.json" -Force
    }
  }
  if ($mdlSrc) {
    foreach ($m in @('zenz-v2.5-small-q8_0.gguf', 'genmai-310M-q8_0.gguf', 'tinyswallow-1.5b-q5_k_m.gguf')) {
      $src = Join-Path $mdlSrc $m
      if (Test-Path $src) {
        Copy-Item $src "$OutDir/runtime/$m" -Force
      }
    }
  }
  # 直接実行テスト用に $OutDir 直下へもランタイム DLL・語彙をミラーする（MSI 内は runtime\** -> INSTALLDIR）
  Get-ChildItem "$OutDir/runtime" -File | ForEach-Object {
    Copy-Item $_.FullName "$OutDir/$($_.Name)" -Force
  }

  if ($BuildMsi) {
    if (-not (Test-Path "$OutDir/data/system.dict")) {
      throw "MSI の作成には $OutDir\data\system.dict が必要です"
    }
    Invoke-Checked cargo @('build', '--release', '--locked', '--bin', 'asteria-tools', '--target', 'x86_64-pc-windows-msvc')
    $tools = 'target/x86_64-pc-windows-msvc/release/asteria-tools.exe'
    $assetsDir = 'target/wix-assets'
    Invoke-Checked $tools @('gen-assets', $assetsDir)
    Invoke-Checked $tools @('gen-icons', $assetsDir, '--brand-only')
    if (-not (Test-Path "$OutDir/THIRD_PARTY_NOTICES.txt")) {
      Copy-Item "$assetsDir/NOTICE-third-party.txt" "$OutDir/THIRD_PARTY_NOTICES.txt"
    }
    $msiDest = if ($MsiOut) { $MsiOut } else { Join-Path $OutDir 'Asteria64.msi' }
    Invoke-Checked wix @(
      'build',
      'frontends/windows/installer/Asteria64.wxs',
      '-ext', 'WixToolset.UI.wixext',
      '-d', "AsteriaVersion=$msiVer",
      '-d', "DistDir=$((Resolve-Path $OutDir).Path)",
      '-d', "AssetsDir=$((Resolve-Path $assetsDir).Path)",
      '-d', "RepoRoot=$root",
      '-o', $msiDest
    )
    $msiHash = (Get-FileHash $msiDest -Algorithm SHA256).Hash.ToLower()
    $msiLeaf = Split-Path $msiDest -Leaf
    [IO.File]::WriteAllText("$msiDest.sha256", "$msiHash  $msiLeaf`n", [Text.Encoding]::ASCII)

    if ($BuildBundle) {
      $cudaSrc = if ($CudaSource) {
        $CudaSource
      } elseif (Test-Path 'C:\Program Files (x86)\Asteria\cuda\ggml-cuda.dll') {
        'C:\Program Files (x86)\Asteria\cuda'
      } elseif (Test-Path 'C:\Program Files\Asteria\cuda\ggml-cuda.dll') {
        'C:\Program Files\Asteria\cuda'
      } else {
        ''
      }
      $cudaMsi = Join-Path $OutDir 'AsteriaCuda64.msi'
      if ($cudaSrc) {
        & (Join-Path $PSScriptRoot 'cuda-addon/build_addon.ps1') -Out $cudaMsi -CudaSource $cudaSrc
      } else {
        & (Join-Path $PSScriptRoot 'cuda-addon/build_addon.ps1') -Out $cudaMsi
      }
      if ($LASTEXITCODE -ne 0) { throw "build_addon.ps1 が $LASTEXITCODE で失敗した" }

      $bundleDest = if ($BundleOut) { $BundleOut } else { Join-Path $OutDir 'Asteria-Setup.exe' }
      & (Join-Path $PSScriptRoot 'bundle/build_bundle.ps1') -MainMsi $msiDest -CudaMsi $cudaMsi -Version $msiVer -Out $bundleDest
      if ($LASTEXITCODE -ne 0) { throw "build_bundle.ps1 が $LASTEXITCODE で失敗した" }
      $bundleHash = (Get-FileHash $bundleDest -Algorithm SHA256).Hash.ToLower()
      $bundleLeaf = Split-Path $bundleDest -Leaf
      [IO.File]::WriteAllText("$bundleDest.sha256", "$bundleHash  $bundleLeaf`n", [Text.Encoding]::ASCII)
    }
  }
} finally {
  Pop-Location
}
