# Opens Shook's settings and hello card on the signed-in desktop, logs where each lands against the screen's work area and
# captures only those windows. No keys, no clicks. It won't start if the PC was used in the last two minutes unless told
# to (-AnyTime, for input that isn't a person). A throwaway config, removed after.
# Shook.exe here is the release; a --features scale-test build reads SHOOK_TEST_SCALE and draws as if Windows were set to it.
param([string]$Scale = '0', [string]$Tag = 'real', [switch]$AnyTime)
$ErrorActionPreference = 'Continue'
if ($Scale -eq '0') { $Scale = '' }
$Here = $PSScriptRoot
$Exe = "$Here\Shook.exe"
$log = "$Here\scale-log.txt"
function Log($m) { "$(Get-Date -Format HH:mm:ss) [$Tag] $m" | Add-Content $log -Encoding UTF8 }

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class S {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  public static IntPtr Titled(string t) { return FindWindow(null, t); }
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int w, int hh, uint f);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }
  [DllImport("user32.dll")] public static extern bool GetLastInputInfo(ref LASTINPUTINFO p);
  [DllImport("kernel32.dll")] public static extern uint GetTickCount();
  public static uint Idle() { var i = new LASTINPUTINFO(); i.cbSize = 8; GetLastInputInfo(ref i); return GetTickCount() - i.dwTime; }
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
  public static RECT Bounds(IntPtr h) { RECT r; DwmGetWindowAttribute(h, 9, out r, 16); return r; }
}
"@
[S]::SetProcessDPIAware() | Out-Null
if (-not $AnyTime -and [S]::Idle() -lt 120000) { Log 'Someone used this PC in the last 2 minutes: not testing.'; 'ABORT' | Set-Content "$Here\SCALE-DONE"; exit }
$screen = [System.Windows.Forms.Screen]::PrimaryScreen
$w = $screen.WorkingArea
Log "screen $($screen.Bounds.Width)x$($screen.Bounds.Height) work $($w.Width)x$($w.Height) dpi $([S]::GetDpiForSystem()) test scale '$Scale'"

function Grab($h, $name) {
    # A scale-test build saves what it really drew: a screen capture sees OpenGL windows as blank when the panel is off.
    if ($Scale) { Start-Sleep -Milliseconds 800; Copy-Item "$Here\frame.bmp" "$Here\scale-$Tag-$name.bmp" -ErrorAction SilentlyContinue }
    $r = [S]::Bounds($h)
    $inside = $r.L -ge $w.Left -and $r.T -ge $w.Top -and $r.R -le $w.Right -and $r.B -le $w.Bottom
    Log ("{0}: {1},{2} to {3},{4} ({5}x{6}) inside work area: {7}" -f $name, $r.L, $r.T, $r.R, $r.B, ($r.R - $r.L), ($r.B - $r.T), $inside)
    $left = [Math]::Max($r.L, 0); $top = [Math]::Max($r.T, 0)
    $bmp = New-Object System.Drawing.Bitmap ([Math]::Min($r.R, $screen.Bounds.Width) - $left), ([Math]::Min($r.B, $screen.Bounds.Height) - $top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($left, $top, 0, 0, $bmp.Size)
    $bmp.Save("$Here\scale-$Tag-$name.png")
    $g.Dispose(); $bmp.Dispose()
}
function Titled($title) {
    foreach ($i in 1..50) { Start-Sleep -Milliseconds 100; $h = [S]::Titled($title); if ($h -ne [IntPtr]::Zero) { return $h } }
    [IntPtr]::Zero
}

$tmp = Join-Path $env:TEMP 'shook-scale-check'
Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$tmp\appdata\Shook" | Out-Null
$env:APPDATA = "$tmp\appdata"
if ($Scale) { $env:SHOOK_TEST_SCALE = $Scale; $env:SHOOK_TEST_SHOT = "$Here\frame.bmp" }
# Settings: as it opens, maximised, and dragged to its smallest.
$p = Start-Process $Exe -ArgumentList '--settings' -PassThru
$h = Titled 'Shook Settings'
if ($h -eq [IntPtr]::Zero) {
    $box = [S]::FindWindow('#32770', 'Shook')
    Log "settings: no window; OpenGL message box up: $($box -ne [IntPtr]::Zero)"
    if ($box -ne [IntPtr]::Zero) { [S]::PostMessage($box, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null }
} else {
    Start-Sleep -Milliseconds 1800
    [S]::SetForegroundWindow($h) | Out-Null; Start-Sleep -Milliseconds 300
    Grab $h 'settings-open'
    [S]::ShowWindow($h, 3) | Out-Null; Start-Sleep -Milliseconds 900
    Grab $h 'settings-max'
    [S]::ShowWindow($h, 9) | Out-Null; Start-Sleep -Milliseconds 500
    # Its minimum: 560 x 520 points, or less on a screen with less room (a call like this isn't held to the minimum).
    $s = if ($Scale) { [double]$Scale } else { [S]::GetDpiForSystem() / 96.0 }
    $small = [int](560 * $s) + 16; $tall = [int]([Math]::Min(520, $w.Height / $s - 56) * $s) + 39
    [S]::SetWindowPos($h, [IntPtr]::Zero, 40, 0, $small, $tall, 0x14) | Out-Null; Start-Sleep -Milliseconds 900
    Grab $h 'settings-small'
    # Every card at that width: taller than the screen, which a call like this is allowed. A scale-test build saves it all.
    [S]::SetWindowPos($h, [IntPtr]::Zero, 40, 0, $small, 1500, 0x14) | Out-Null; Start-Sleep -Milliseconds 900
    Grab $h 'settings-tall'
    [S]::PostMessage($h, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
}
$p.WaitForExit(4000) | Out-Null
if (-not $p.HasExited) { $p | Stop-Process -Force }

# The hello card.
$p = Start-Process $Exe -ArgumentList '--hello' -PassThru
$h = Titled 'Shook hello'
if ($h -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 1500; Grab $h 'hello' }
$p | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
Log 'finished'
'DONE' | Set-Content "$Here\SCALE-DONE"
