# install.ps1 と uninstall.ps1 を試す(CI の x64 のジョブで実行する。管理者権限が要る)。
# 引数: -Server <asteria-server.exe> -Dll <x64 の asteria_tip.dll>。辞書は中身のない偽物を置く。
param(
  [Parameter(Mandatory)] [string] $Server,
  [Parameter(Mandatory)] [string] $Dll,
  [string] $Renderer = '',
  [string] $Config = ''
)
$ErrorActionPreference = 'Stop'

$source = Join-Path ([IO.Path]::GetTempPath()) 'asteria-dist'
$dest = Join-Path ([IO.Path]::GetTempPath()) 'asteria-install-test'
New-Item -ItemType Directory -Force (Join-Path $source 'data') | Out-Null
Copy-Item $Server, $Dll $source
$expected = @('asteria-server.exe', 'asteria_tip.dll', 'data\system.dict')
if ($Renderer) {
  Copy-Item $Renderer $source
  $expected += 'asteria-renderer.exe'
}
if ($Config) {
  Copy-Item $Config $source
  $expected += 'asteria-config.exe'
}
Set-Content (Join-Path $source 'data\system.dict') 'dummy'

$clsidKey = 'HKLM:\SOFTWARE\Classes\CLSID\{797AD563-3368-4DCB-B3E7-8B0306B42B67}\InprocServer32'
& "$PSScriptRoot/../install.ps1" -Source $source -Destination $dest
foreach ($f in $expected) {
  if (-not (Test-Path (Join-Path $dest $f))) { throw "$f が置かれていない" }
}
$registered = (Get-ItemProperty $clsidKey).'(default)'
if ($registered -ne (Join-Path $dest 'asteria_tip.dll')) { throw "InprocServer32 が $registered になっている" }
Write-Host 'インストールを確かめた'

& "$PSScriptRoot/../uninstall.ps1" -Destination $dest
if ($LASTEXITCODE -ne 0) { throw "uninstall.ps1 が $LASTEXITCODE で終わった" }
if (Test-Path $dest) { throw "$dest が残っている" }
if (Test-Path $clsidKey) { throw 'COM の登録が残っている' }
Write-Host 'アンインストールを確かめた'
