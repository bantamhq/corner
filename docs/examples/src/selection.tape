Output ../selection.gif

Set Padding 16
Set Width 1600
Set Height 800
Set TypingSpeed 0.2
Set Theme "DoomOne"

Hide
Type "cd ../../.."
Enter
Sleep 0.3s
Type "cargo run docs/examples/src/example_journal.md"
Enter
Sleep 1s
Show

Sleep 2s

Type "v"
Sleep 0.5s

Up@300ms 2
Sleep 0.3s
Type "v"
Sleep 0.5s

Up 3
Sleep 0.3s
Type "V"
Sleep 1s

Type "c"
Sleep 2s

Hide
Type "c"
Sleep 0.3s
Escape
Sleep 0.3s
