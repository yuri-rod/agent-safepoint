# VHS Tape for safepoint demo
Output demo.gif

Set FontSize 16
Set Width 900
Set Height 550
Set Padding 20
Set Theme "Catppuccin Mocha"

Type "safepoint run -- codex 'Refactor database models'"
Enter
Sleep 3s

Type "safepoint timeline"
Enter
Sleep 2s

Type "safepoint diff @1"
Enter
Sleep 2.5s

Type "safepoint undo @1"
Enter
Sleep 3s
