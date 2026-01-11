Output ../reorder.gif

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

Type "r"
Sleep 0.5s

Up@300ms 3
Sleep 0.5s

Enter
Sleep 2s

Hide
Type "r"
Sleep 0.3s
Down@300ms 4
Sleep 0.3s
Enter
Sleep 0.3s
