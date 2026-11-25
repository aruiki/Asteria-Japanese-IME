# Asteria日本語入力のインストーラー(Asteria-Setup.exe)を作る(docs/adr/0091、0092)。
#
# 使い方: pwsh frontends/windows/bundle/build_bundle.ps1 -MainMsi Asteria64.msi -CudaMsi AsteriaCuda64.msi -Version 5.5.0 [-Out Asteria-Setup.exe] [-Wix wix] [-DownloadBase <URL>]
# 画面(ui/、WPF)を dotnet で作ってから、WiX 5.0.2 の Burn で exe にする。2 つの MSI は exe に入れず、
# インストールのときに DownloadBase から受け取る(0092)。exe は数 MB になる。
# DownloadBase の既定は配布用のリポジトリ aruiki/asteria-downloads の、版と同じ名前のリリース(v<版>)。
# 作ったあと、2 つの MSI をそのリリースに上げる(手順は最後に表示する)。MSI の中身を変えたら exe も作り直す
# (exe には MSI のハッシュが入っていて、違うものは受け取っても使わない)。
param(
  [Parameter(Mandatory = $true)][string]$MainMsi,
  [Parameter(Mandatory = $true)][string]$CudaMsi,
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Out = 'Asteria-Setup.exe',
  [string]$Wix = 'wix',
  [string]$DownloadBase = ''
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$downloadRepo = 'aruiki/asteria-downloads'
if (-not $DownloadBase) { $DownloadBase = "https://github.com/$downloadRepo/releases/download/v$Version" }

# GitHub のリリースに置けるのは 1 ファイル 2 GiB まで。MSI ごとに確かめる(MSI 自体の上限は 1.8 GiB、0086)。
foreach ($msi in @($MainMsi, $CudaMsi)) {
  $len = (Get-Item $msi).Length
  if ($len -ge 2GB) { throw "$msi が GitHub の 1 ファイルの上限(2 GiB)を超えた" }
}

# 1. 画面
& dotnet build (Join-Path $here 'ui/AsteriaSetupUI.csproj') -c Release -nologo
if ($LASTEXITCODE -ne 0) { throw '画面(ui)を作れない' }
$ui = Join-Path $here 'ui/bin/Release/net472'

# 2. WiX の拡張(本体の MSI と同じ 5.0.2)。名前で見つからないことがあるので DLL のパスで渡す。
$wixExtRoot = Join-Path $HOME '.wix/extensions/WixToolset.Util.wixext/5.0.2'
$util = if (Test-Path $wixExtRoot) {
  Get-ChildItem $wixExtRoot -Recurse -Filter '*.wixext.dll' | Select-Object -First 1
}
if (-not $util) {
  & $Wix extension add -g 'WixToolset.Util.wixext/5.0.2' | Out-Null
  $util = Get-ChildItem $wixExtRoot -Recurse -Filter '*.wixext.dll' | Select-Object -First 1
}
if (-not $util) { throw 'WiX の拡張 WixToolset.Util.wixext を入れられない' }

# 3. exe。wix は MSI を exe の隣にも写すが、配るのは exe だけ(MSI は下の手順で別のリポジトリへ)。
& $Wix build -nologo -arch x64 -ext $util.FullName `
  -d "Version=$Version" -d "MainMsi=$(Resolve-Path $MainMsi)" -d "CudaMsi=$(Resolve-Path $CudaMsi)" `
  -d "UiDir=$(Resolve-Path $ui)" -d "DownloadBase=$DownloadBase" `
  -o $Out (Join-Path $here 'AsteriaSetup.wxs')
if ($LASTEXITCODE -ne 0) { throw 'wix build に失敗した' }

$size = (Get-Item $Out).Length
Write-Host "$Out $([math]::Round($size / 1MB, 1)) MB(MSI は $DownloadBase から受け取る)"
if ($size -ge 100MB) { throw "$Out が 100 MB を超えた。MSI が exe に入っていないか確かめる" }

Write-Host ''
Write-Host '次に、2 つの MSI を配布用のリリースに上げる:'
Write-Host "  gh release create v$Version --repo $downloadRepo --title `"v$Version`" --notes `"Installer payloads for Asteria v$Version. Asteria-Setup.exe downloads these automatically.`" `"$MainMsi`" `"$CudaMsi`""
