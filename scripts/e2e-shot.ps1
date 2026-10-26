# 端到端截图验证：vite(自动加载样本) + exe + 按 PID 找窗口截图，全程日志落盘
param(
  [string]$OutPng = "screenshot-app.png",
  [int]$WaitSeconds = 45
)

$ErrorActionPreference = "Continue"
$log = Join-Path $PSScriptRoot "e2e-shot.log"
"=== start $(Get-Date) ===" | Out-File $log -Encoding utf8

function Log($msg) {
  "$(Get-Date -Format HH:mm:ss) $msg" | Out-File $log -Append -Encoding utf8
}

function KillTree([int]$ProcId) {
  if ($ProcId -le 0) { return }
  & cmd /c "taskkill /PID $ProcId /T /F >nul 2>&1" | Out-Null
}

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public class W5 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")]
  public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")]
  public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")]
  public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")]
  public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")]
  public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  public struct RECT { public int Left, Top, Right, Bottom; }
  // 按进程 PID 找其可见主窗口
  public static IntPtr FindByPid(uint target) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      uint pid;
      GetWindowThreadProcessId(h, out pid);
      if (pid == target && found == IntPtr.Zero) {
        var sb = new StringBuilder(512);
        GetWindowTextW(h, sb, 512);
        if (sb.Length > 0) found = h;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  // 最小化标题包含指定关键字的窗口，返回被最小化的句柄列表（便于恢复）
  public static List<IntPtr> MinimizeByTitleContains(string kw) {
    var list = new List<IntPtr>();
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      var sb = new StringBuilder(512);
      GetWindowTextW(h, sb, 512);
      if (sb.Length > 0 && sb.ToString().Contains(kw)) { ShowWindow(h, 6); list.Add(h); } // SW_MINIMIZE
      return true;
    }, IntPtr.Zero);
    return list;
  }
  public static void Restore(List<IntPtr> handles) {
    foreach (var h in handles) ShowWindow(h, 9); // SW_RESTORE
  }
}
"@

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# 清理（进程名是中文，用路径匹配）
Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -eq "node.exe" -or $_.Name -eq "cargo.exe" -or $_.Name -eq "rustc.exe" -or $_.CommandLine -like "*ir-target\debug*" } |
  ForEach-Object { KillTree $_.ProcessId }
Start-Sleep -Seconds 2
Log "cleaned"

# vite
$sample = Join-Path $root "DJI_20260804234634_0002_T.JPG"
if (-not (Test-Path $sample)) { Log "SAMPLE NOT FOUND"; exit 1 }
$env:VITE_AUTO_LOAD_IMG = $sample
$vite = Start-Process -FilePath "cmd" -ArgumentList "/c","pnpm dev > vite-e2e.log 2>&1" -PassThru -WindowStyle Hidden
$up = $false
for ($i = 0; $i -lt 40; $i++) {
  Start-Sleep -Seconds 1
  if (Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue) { $up = $true; Log "vite up after $i s"; break }
}
if (-not $up) { Log "VITE FAILED"; KillTree $vite.Id; exit 1 }

# exe
$exe = (Get-ChildItem "D:\Rust\ir-target\debug" -Filter "*.exe" -ErrorAction SilentlyContinue |
  Sort-Object Length -Descending | Select-Object -First 1).FullName
if (-not $exe) { Log "EXE NOT FOUND"; exit 1 }
Log "launching $exe"
$app = Start-Process -FilePath $exe -PassThru
Log "app pid=$($app.Id)"

$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt $WaitSeconds; $i++) {
  Start-Sleep -Seconds 1
  if ($app.HasExited) { Log "APP EXITED code=$($app.ExitCode)"; break }
  $hwnd = [W5]::FindByPid([uint32]$app.Id)
  if ($hwnd -ne [IntPtr]::Zero) { Log "window after $i s"; break }
}
Start-Sleep -Seconds 6

if ($hwnd -eq [IntPtr]::Zero) {
  Log "WINDOW NOT FOUND"
} else {
  # 最小化可能遮挡的 IDE/DevTools 窗口，并把应用窗口移到左上角
  $minimized = [W5]::MinimizeByTitleContains("CodeBuddy")
  $minimized2 = [W5]::MinimizeByTitleContains("ir-target")
  $minimized3 = [W5]::MinimizeByTitleContains("DevTools")
  $allMin = @($minimized) + @($minimized2) + @($minimized3)
  [void][W5]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, 1200, 800, 0x0004) # SWP_NOZORDER
  [void][W5]::SetForegroundWindow($hwnd)
  Start-Sleep -Seconds 2
  $r = New-Object W5+RECT
  [void][W5]::GetWindowRect($hwnd, [ref]$r)
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $out = if ([IO.Path]::IsPathRooted($OutPng)) { $OutPng } else { Join-Path $root $OutPng }
  $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  Log "saved $out (${w}x${h})"
  # 恢复 IDE 窗口
  [W5]::Restore($allMin) | Out-Null
}

KillTree $app.Id
KillTree $vite.Id
Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -eq "node.exe" -or $_.CommandLine -like "*ir-target\debug*" } |
  ForEach-Object { KillTree $_.ProcessId }
Log "done"
