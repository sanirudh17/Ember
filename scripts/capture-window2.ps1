# Captures the Ember main window via PrintWindow even when the tray added small
# helper windows to the same process: enumerate top-level windows of every
# ember.exe process and pick the largest one. Occlusion-safe, focus-safe.
param(
  [string]$Out = "C:\Users\sanir\Windows Tool\screenshots\verify-live-app.png"
)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class WinEnum {
  public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

  public static List<IntPtr> ForPids(HashSet<uint> pids) {
    var found = new List<IntPtr>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pids.Contains(pid)) found.Add(h);
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@
[WinEnum]::SetProcessDPIAware() | Out-Null

$pids = New-Object 'System.Collections.Generic.HashSet[uint32]'
foreach ($p in (Get-Process -Name ember -ErrorAction SilentlyContinue)) { [void]$pids.Add([uint32]$p.Id) }
if ($pids.Count -eq 0) { Write-Output "EMBER_NOT_RUNNING"; exit 2 }

$best = [IntPtr]::Zero
$bestArea = 0
$bestRect = New-Object 'WinEnum+RECT'
foreach ($h in [WinEnum]::ForPids($pids)) {
  $r = New-Object 'WinEnum+RECT'
  if (-not [WinEnum]::GetWindowRect($h, [ref]$r)) { continue }
  $w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
  if ($w -lt 400 -or $hh -lt 300) { continue }
  $area = $w * $hh
  if ($area -gt $bestArea) { $bestArea = $area; $best = $h; $bestRect = $r }
}
if ($best -eq [IntPtr]::Zero) { Write-Output "NO_MAIN_WINDOW_FOUND (windows: $($pids.Count) pids)"; exit 3 }

$w = $bestRect.Right - $bestRect.Left
$h = $bestRect.Bottom - $bestRect.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# PW_RENDERFULLCONTENT = 2
$ok = [WinEnum]::PrintWindow($best, $hdc, 2)
$g.ReleaseHdc($hdc)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$samples = @()
foreach ($xy in @(@(20, 20), @([int]($w / 2), 60), @([int]($w / 2), [int]($h / 2)), @(30, [int]($h / 2)))) {
  $c = $bmp.GetPixel($xy[0], $xy[1]); $samples += "$($c.R),$($c.G),$($c.B)"
}
$g.Dispose(); $bmp.Dispose()
Write-Output "PrintWindow ok=$ok size=${w}x${h} samples=$($samples -join ' | ') -> $Out"
