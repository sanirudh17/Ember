# Captures the live Ember window to a PNG. Waits for the window to appear first.
param(
  [string]$Out = "C:\Users\sanir\Windows Tool\screenshots\verify-live-app.png",
  [int]$WaitSeconds = 360
)
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinCap {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
[WinCap]::SetProcessDPIAware() | Out-Null

$deadline = (Get-Date).AddSeconds($WaitSeconds)
$p = $null
while ((Get-Date) -lt $deadline) {
  $c = Get-Process -Name ember -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if ($c) { $p = $c; break }
  Start-Sleep -Seconds 4
}
if (-not $p) { Write-Output "EMBER_WINDOW_NOT_FOUND"; exit 2 }
Start-Sleep -Seconds 3
$hwnd = $p.MainWindowHandle
[WinCap]::ShowWindow($hwnd, 9) | Out-Null
[WinCap]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 1200
$r = New-Object 'WinCap+RECT'
[WinCap]::GetWindowRect($hwnd, [ref]$r) | Out-Null
$vs = [System.Windows.Forms.SystemInformation]::VirtualScreen
$left   = [Math]::Max($vs.Left, $r.Left)
$top    = [Math]::Max($vs.Top, $r.Top)
$right  = [Math]::Min($vs.Right, $r.Right)
$bottom = [Math]::Min($vs.Bottom, $r.Bottom)
$w = $right - $left
$hgt = $bottom - $top
if ($w -le 0 -or $hgt -le 0) { Write-Output "EMBER_WINDOW_RECT_INVALID $($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"; exit 3 }
$bmp = New-Object System.Drawing.Bitmap($w, $hgt)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($left, $top, 0, 0, (New-Object System.Drawing.Size($w, $hgt)))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "EMBER_RUNNING pid=$($p.Id) rect=$($r.Left),$($r.Top),$($r.Right),$($r.Bottom) captured=$($w)x$($hgt) -> $Out"
