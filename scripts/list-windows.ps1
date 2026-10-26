# 枚举当前可见窗口标题
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public class WinEnum {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")]
  public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")]
  public static extern bool IsWindowVisible(IntPtr h);
  public static List<string> List() {
    var res = new List<string>();
    EnumWindows((h, l) => {
      if (IsWindowVisible(h)) {
        var sb = new StringBuilder(512);
        GetWindowTextW(h, sb, 512);
        if (sb.Length > 0) res.Add(sb.ToString());
      }
      return true;
    }, IntPtr.Zero);
    return res;
  }
}
"@
[WinEnum]::List() | Where-Object { $_ -notlike "*Microsoft*" -and $_ -notlike "*Chrome*" -and $_ -notlike "*Edge*" -and $_ -notlike "*Windows*" -and $_ -notlike "*任务栏*" -and $_ -notlike "*Program*" -and $_ -notlike "*设置*" -and $_ -notlike "*文件资源*" } | Select-Object -First 25
