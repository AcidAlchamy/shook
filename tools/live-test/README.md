# Window check

`scale-check.ps1` (started by `scale.vbs`) opens the settings window and the hello card on a test PC's signed-in desktop
and logs where each lands against the work area. It sends no keys or clicks and uses a throwaway config.

1. Copy `Shook.exe`, `scale-check.ps1` and `scale.vbs` to one folder on the test PC.
2. Run `scale.vbs 0 real` inside the signed-in desktop session (an interactive scheduled task works: `schtasks /Create /TN ShookScale /TR "wscript.exe <folder>\scale.vbs 0 real" /SC ONCE /SD 01/01/2035 /ST 12:00 /IT /F`, then `schtasks /Run /TN ShookScale`, and `schtasks /Delete /TN ShookScale /F` when done).
3. Read `scale-log.txt` there. The `scale-*.png` files show whatever is on that PC's screen: delete them when done.

It refuses to start if the PC was used in the last two minutes, unless told to with `-AnyTime` (for input that isn't a person).

For other display scales, build `cargo build --release --features scale-test` and copy that exe in as `Shook.exe`: it reads
`SHOOK_TEST_SCALE`, draws as if Windows were set to it, and saves what it really drew to `frame.bmp`, because a screen
capture shows OpenGL windows as blank white on a PC whose panel isn't showing. A test build stands in for layout and
sizes only: the title bar stays at the real scale. A scale under 1 fits the whole settings window in one frame.
