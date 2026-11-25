# Asteria日本語入力のインストーラー(1 つの exe)を作る(docs/adr/0091)。
#
# 使い方: pwsh mozc/bundle/build_bundle.ps1 -MainMsi Asteria64.msi -CudaMsi AsteriaCuda64.msi -Version 5.4.0 [-Out Asteria-Setup.exe] [-Wix wix]
# 画面(ui/、WPF)を dotnet で作ってから、WiX 5.0.2 の Burn で 2 つの MSI と一緒に 1 つの exe にする。
# GitHub のリリースに置けるのは 1 ファイル 2 GiB まで。超えたら止める(v5.4.0 で残り 36 MB)。
param(
  [Parameter(Mandatory = $true)][string]$MainMsi,
  [Parameter(Mandatory = $true)][string]$CudaMsi,
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Out = 'Asteria-Setup.exe',
  [string]$Wix = 'wix'
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot

# 1. 画面
& dotnet build (Join-Path $here 'ui/AsteriaSetupUI.csproj') -c Release -nologo
if ($LASTEXITCODE -ne 0) { throw '画面(ui)を作れない' }
$ui = Join-Path $here 'ui/bin/Release/net472'

# 2. WiX の拡張(本体の MSI と同じ 5.0.2)。名前で見つからないことがあるので DLL のパスで渡す。
& $Wix extension add -g 'WixToolset.Util.wixext/5.0.2' | Out-Null
$util = Get-ChildItem (Join-Path $HOME '.wix/extensions/WixToolset.Util.wixext/5.0.2') -Recurse -Filter '*.wixext.dll' |
  Select-Object -First 1
if (-not $util) { throw 'WiX の拡張 WixToolset.Util.wixext を入れられない' }

# 3. 1 つの exe
& $Wix build -nologo -arch x64 -ext $util.FullName `
  -d "Version=$Version" -d "MainMsi=$(Resolve-Path $MainMsi)" -d "CudaMsi=$(Resolve-Path $CudaMsi)" `
  -d "UiDir=$(Resolve-Path $ui)" `
  -o $Out (Join-Path $here 'AsteriaSetup.wxs')
if ($LASTEXITCODE -ne 0) { throw 'wix build に失敗した' }

$size = (Get-Item $Out).Length
$limit = 2GB
Write-Host "$Out $size バイト(GitHub の上限まで $([math]::Round(($limit - $size) / 1MB)) MB)"
if ($size -ge $limit) { throw "$Out が GitHub の 1 ファイルの上限(2 GiB)を超えた" }
