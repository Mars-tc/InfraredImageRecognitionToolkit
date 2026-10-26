# 启动 tauri dev，等待窗口出现，截取应用窗口保存为 PNG
# 用法: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\capture-app.ps1 -WaitSeconds 150 -WindowTitle <title> -OutPng out.png
param(
  [int]$WaitSeconds = 150,
  [string]$WindowTitle = "",
  [string]$OutPng = "screenshot-app.png"
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

# 用脚本位置派生项目根，避免路径中含不可见字符导致解析问题
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:CARGO_TARGET_DIR = "D:\Rust\ir-target"

# 自动加载样本（前端 vite 环境变量钩子）
$sample = Join-Path $root "DJI_20260804234634_0002_T.JPG"
if (Test-Path $sample) { $env:VITE_AUTO_LOAD_IMG = $sample } else { Write-Output "sample not found: $sample" }

$proc = Start-Process -FilePath "cmd" -ArgumentList "/c","pnpm tauri dev > tauri-dev.log 2>&1" -PassThru -WindowStyle Hidden

$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt $WaitSeconds; $i++) {
  Start-Sleep -Seconds 1
  $hwnd = [Win]::FindWindowW($null, $WindowTitle)
  if ($hwnd -ne [IntPtr]::Zero) { Write-Output "window found after $i s"; break }
}
Start-Sleep -Seconds 5

if ($hwnd -eq [IntPtr]::Zero) {
  Write-Output "WINDOW NOT FOUND"
  Get-Content (Join-Path $root "tauri-dev.log") -Tail 4
} else {
  [void][Win]::SetForegroundWindow($hwnd)
  Start-Sleep -Seconds 1
  $rect = New-Object Win+RECT
  [void][Win]::GetWindowRect($hwnd, [ref]$rect)
  $w = $rect.Right - $rect.Left
  $h = $rect.Bottom - $rect.Top
  Write-Output "window rect: $($rect.Left),$($rect.Top) ${w}x${h}"
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
  $outPath = if ([IO.Path]::IsPathRooted($OutPng)) { $OutPng } else { Join-Path $root $OutPng }
  $bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
  Write-Output "screenshot saved: $outPath"
}

taskkill /PID $proc.Id /T /F 2>&1 | Out-Null
Get-Process -Name "ir-inspection-tool" -ErrorAction SilentlyContinue | ForEach-Object { taskkill /PID $_.Id /T /F 2>&1 | Out-Null }
# 清理属于本项目的 vite/node 子进程（避免残留占用 5173 端口）
Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction SilentlyContinue |
  Where-Object { $_.CommandLine -like "*$root*" } |
  ForEach-Object { taskkill /PID $_.ProcessId /T /F 2>&1 | Out-Null }
Write-Output "done"
