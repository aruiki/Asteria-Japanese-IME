# 画面つきの MSI(docs/adr/0072)の画面を記録し、最初の画面のボタン(「インストール」か「更新」)を押す。
# 管理者の確認(UAC)は安全なデスクトップに出るので、この道具では押せない(利用者が承認する)。
# 使い方: powershell -File installer_ui_driver.ps1 -Out <フォルダ> [-NoClick]
#   msiexec の窓が出るまで待ち、出た窓ごとに画像(PNG)と文字・チェックの状態を screens.json に残す。
param([Parameter(Mandatory = $true)][string]$Out, [switch]$NoClick, [int]$TimeoutSec = 900,
      [string]$ClickPattern = '^(インストール|更新)')
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing
# MSI の画面の部品は UI Automation から見えないことがあるので、Win32 の子ウィンドウでも探す。
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class MsiWin {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, int m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern IntPtr PostMessage(IntPtr h, int m, IntPtr w, IntPtr l);
  public static List<string> Texts(IntPtr top) {
    var list = new List<string>();
    EnumChildWindows(top, (h, l) => {
      if (!IsWindowVisible(h)) return true;
      var t = new StringBuilder(512); GetWindowText(h, t, 512);
      var c = new StringBuilder(64); GetClassName(h, c, 64);
      if (t.Length > 0) {
        string state = "";
        if (c.ToString() == "Button") {
          int checkState = (int)SendMessage(h, 0x00F0, IntPtr.Zero, IntPtr.Zero);  // BM_GETCHECK
          state = checkState == 1 ? "[x]" : "";
        }
        list.Add(c + ":" + state + t);
      }
      return true;
    }, IntPtr.Zero);
    return list;
  }
  public static string Click(IntPtr top, string pattern) {
    string clicked = null;
    var re = new System.Text.RegularExpressions.Regex(pattern);
    EnumChildWindows(top, (h, l) => {
      var t = new StringBuilder(512); GetWindowText(h, t, 512);
      var c = new StringBuilder(64); GetClassName(h, c, 64);
      if (c.ToString() == "Button" && IsWindowVisible(h) && IsWindowEnabled(h) && re.IsMatch(t.ToString())) {
        PostMessage(h, 0x00F5, IntPtr.Zero, IntPtr.Zero);  // BM_CLICK(画面を止めないよう非同期)
        clicked = t.ToString();
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return clicked;
  }
}
"@
New-Item -ItemType Directory -Force $Out | Out-Null
$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]

function Capture($element, $path) {
  $r = $element.Current.BoundingRectangle
  if ($r.Width -le 0 -or $r.Height -le 0) { return }
  $bmp = New-Object System.Drawing.Bitmap ([int]$r.Width), ([int]$r.Height)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bmp.Size)
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

function Describe($window) {
  $items = @()
  $all = $window.FindAll($TS::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($e in $all) {
    $c = $e.Current
    if (-not $c.Name) { continue }
    $item = [ordered]@{ type = $c.ControlType.ProgrammaticName -replace '^ControlType\.', ''; name = $c.Name }
    if ($item.type -eq "CheckBox") {
      try { $item.checked = ($e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -eq "On") } catch {}
    }
    $items += $item
  }
  return $items
}

$screens = @()
$seen = @{}
$clicked = $false
$sawInstaller = $false
$deadline = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $deadline) {
  $pids = @(Get-Process msiexec -ErrorAction SilentlyContinue | ForEach-Object Id)
  if ($sawInstaller -and $pids.Count -eq 0) { break }
  $windows = $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($w in $windows) {
    if ($pids -notcontains $w.Current.ProcessId) { continue }
    $sawInstaller = $true
    $items = Describe $w
    $hwnd = [IntPtr]$w.Current.NativeWindowHandle
    $win32 = [MsiWin]::Texts($hwnd)
    # 窓の題名とボタンの並びで画面を見分ける(進行表示の文字は毎回変わる)。
    $buttons = (@($items | Where-Object { $_.type -eq "Button" } | ForEach-Object { $_.name }) +
                @($win32 | Where-Object { $_ -like "Button:*" })) -join "|"
    $key = $w.Current.Name + "#" + $buttons
    if (-not $seen.ContainsKey($key)) {
      $seen[$key] = $true
      $png = Join-Path $Out ("screen-{0:D2}.png" -f ($screens.Count + 1))
      Start-Sleep -Milliseconds 400
      Capture $w $png
      $screens += [ordered]@{ at = (Get-Date).ToString("o"); title = $w.Current.Name; buttons = $buttons; image = $png; items = $items; win32 = $win32 }
      $screens | ConvertTo-Json -Depth 6 | Out-File -Encoding utf8 (Join-Path $Out "screens.json")
    }
    if (-not $clicked -and -not $NoClick) {
      $name = [MsiWin]::Click($hwnd, $ClickPattern)
      if ($name) { $clicked = $true; "clicked: " + $name; continue }
      $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
      foreach ($b in $w.FindAll($TS::Descendants, $cond)) {
        if ($b.Current.Name -match $ClickPattern) {
          $b.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          $clicked = $true
          "clicked: " + $b.Current.Name
          break
        }
      }
    }
  }
  Start-Sleep -Milliseconds 300
}
"screens: " + $screens.Count
$screens | ForEach-Object { "  " + $_.title + " [" + $_.buttons + "]" }
