' Starts scale-check.ps1 with no console window. Arguments: the test scale (0 for the real one) and a tag for the files.
Set a = WScript.Arguments
here = CreateObject("Scripting.FileSystemObject").GetParentFolderName(WScript.ScriptFullName)
CreateObject("WScript.Shell").Run "powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File """ & here & "\scale-check.ps1"" -Scale " & a(0) & " -Tag " & a(1), 0, True
