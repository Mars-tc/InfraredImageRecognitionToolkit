# 真机 IPC 验证：独立 vite(带 VITE_AUTO_LOAD_IMG) + 直接运行 exe，截取窗口
# 用法: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-app.ps1 -WindowTitle <title> -OutPng out.png
param(
  [string]$WindowTitle = "",
  [string]$OutPng = "screenshot-app.png",
  [int]$WaitSeconds = 60
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern IntPtr FindWindowW(string cls, string title);
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr h);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

if (-not $WindowTitle) { Write-Output "MISSING -WindowTitle"; exit 1 }

function KillTree([int]$ProcId) {
  if ($ProcId -le 0) { return }
  & cmd /c "taskkill /PID $ProcId /T /F >nul 2>&1" | Out-Null
}

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# 清理残留进程（node/cargo/app）
Get-Process -Name node,cargo,rustc,tauri -ErrorAction SilentlyContinue | ForEach-Object { KillTree $_.Id }
Get-Process -Name "ir-inspection-tool" -ErrorAction SilentlyContinue | ForEach-Object { KillTree $_.Id }
Start-Sleep -Seconds 2

# 1. 独立启动 vite（自动加载样本）
$sample = Join-Path $root "DJI_20260804234634_0002_T.JPG"
if (-not (Test-Path $sample)) { Write-Output "SAMPLE NOT FOUND"; exit 1 }
$env:VITE_AUTO_LOAD_IMG = $sample
$vite = Start-Process -FilePath "cmd" -ArgumentList "/c","pnpm dev > vite-verify.log 2>&1" -PassThru -WindowStyle Hidden

$ok = $false
for ($i = 0; $i -lt 30; $i++) {
  Start-Sleep -Seconds 1
  $c = Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
  if ($c) { $ok = $true; Write-Output "vite up after $i s"; break }
}
if (-not $ok) { Write-Output "VITE FAILED"; Get-Content (Join-Path $root "vite-verify.log") -Tail 5; taskkill /PID $vite.Id /T /F 2>&1 | Out-Null; exit 1 }

# 2. 直接运行应用 exe（加载 dev server 页面，走真实 IPC）
$exe = (Get-ChildItem "D:\Rust\ir-target\debug" -Filter "*.exe" -ErrorAction SilentlyContinue |
  Sort-Object Length -Descending | Select-Object -First 1).FullName
if (-not $exe) { Write-Output "EXE NOT FOUND"; exit 1 }
Write-Output "launching: $exe"
$app = Start-Process -FilePath $exe -PassThru

$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt $WaitSeconds; $i++) {
  Start-Sleep -Seconds 1
  $hwnd = [Win]::FindWindowW($null, $WindowTitle)
  if ($hwnd -ne [IntPtr]::Zero) { Write-Output "window found after $i s"; break }
}
Start-Sleep -Seconds 6

if ($hwnd -eq [IntPtr]::Zero) {
  Write-Output "WINDOW NOT FOUND"
  if ($app.HasExited) { Write-Output "app exited: $($app.ExitCode)" }
} else {
  [void][Win]::SetForegroundWindow($hwnd)
  Start-Sleep -Seconds 1
  $rect = New-Object Win+RECT
  [void][Win]::GetWindowRect($hwnd, [ref]$rect)
  $w = $rect.Right - $rect.Left
  $h = $rect.Bottom - $rect.Top
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
  $outPath = if ([IO.Path]::IsPathRooted($OutPng)) { $OutPng } else { Join-Path $root $OutPng }
  $bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
  Write-Output "screenshot saved: $outPath ($($w)x$($h))"
}

# 3. 清理
if (-not $app.HasExited) { KillTree $app.Id }
KillTree $vite.Id
Get-Process -Name node -ErrorAction SilentlyContinue | ForEach-Object { KillTree $_.Id }
Get-Process -Name "ir-inspection-tool" -ErrorAction SilentlyContinue | ForEach-Object { KillTree $_.Id }
Write-Output "done"
