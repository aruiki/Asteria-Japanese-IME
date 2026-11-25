# Asteria日本語入力のインストーラー(1 つの exe)を作る(docs/adr/0091)。
#
# 使い方: pwsh mozc/bundle/build_bundle.ps1 -MainMsi Asteria64.msi -CudaMsi AsteriaCuda64.msi -Version 5.4.0 [-Out Asteria-Setup.exe] [-Wix wix]
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

# WiX の拡張(本体の MSI と同じ 5.0.2)。5.0.2 の Bal は DLL の名前が拡張の名前と違い、名前では
# 見つからない(wix extension list で damaged と出る)ので、DLL のパスで渡す。
$exts = @()
foreach ($name in 'WixToolset.Bal.wixext', 'WixToolset.Util.wixext') {
  & $Wix extension add -g "$name/5.0.2" | Out-Null
  $dir = Join-Path $HOME ".wix/extensions/$name/5.0.2"
  $dll = Get-ChildItem $dir -Recurse -Filter '*.wixext.dll' -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $dll) { throw "WiX の拡張 $name を入れられない" }
  $exts += @('-ext', $dll.FullName)
}
& $Wix build -nologo -arch x64 @exts `
  -d "Version=$Version" -d "MainMsi=$(Resolve-Path $MainMsi)" -d "CudaMsi=$(Resolve-Path $CudaMsi)" `
  -o $Out (Join-Path $here 'AsteriaSetup.wxs')
if ($LASTEXITCODE -ne 0) { throw 'wix build に失敗した' }

$size = (Get-Item $Out).Length
$limit = 2GB
Write-Host "$Out $size バイト(GitHub の上限まで $([math]::Round(($limit - $size) / 1MB)) MB)"
if ($size -ge $limit) { throw "$Out が GitHub の 1 ファイルの上限(2 GiB)を超えた" }
