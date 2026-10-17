# Captures the Ember window contents directly via PrintWindow (works occluded, without stealing focus).
param(
  [string]$Out = "C:\Users\sanir\Windows Tool\screenshots\verify-live-app.png"
)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class PWin {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
[PWin]::SetProcessDPIAware() | Out-Null
$p = Get-Process -Name ember -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { Write-Output "EMBER_NOT_RUNNING"; exit 2 }
$hwnd = $p.MainWindowHandle
$r = New-Object 'PWin+RECT'
[PWin]::GetWindowRect($hwnd, [ref]$r) | Out-Null
$w = $r.Right - $r.Left
$h = $r.Bottom - $r.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# PW_RENDERFULLCONTENT = 2
$ok = [PWin]::PrintWindow($hwnd, $hdc, 2)
$g.ReleaseHdc($hdc)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
# Sample a few pixels to detect an all-black (failed) render.
$samples = @()
foreach ($xy in @(@(20,20), @([int]($w/2), 60), @([int]($w/2), [int]($h/2)), @(30, [int]($h/2)))) {
  $c = $bmp.GetPixel($xy[0], $xy[1]); $samples += "$($c.R),$($c.G),$($c.B)"
}
$g.Dispose(); $bmp.Dispose()
Write-Output "PrintWindow ok=$ok size=${w}x${h} samples=$($samples -join ' | ') -> $Out"
