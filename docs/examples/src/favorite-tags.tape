Output ../favorite-tags.gif

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

Type "["
Sleep 1s

Type "g"
Sleep 0.5s

Type "v"
Sleep 0.5s

Down@350ms 4
Sleep 0.5s
Type "v"
Sleep 1s

Type "!"
Sleep .5s

Escape
Sleep 2s

Type "1"
Sleep 3s

Hide
Type "v"
Sleep 0.3s
Up
Sleep 0.3s
Type "v"
Sleep 0.3s
Type "x"
Sleep 0.3s
