Output ../hero.gif

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

Sleep 1s

Enter
Sleep 0.7s

Type "Add config validation on startup"
Sleep 0.2s
Type@500ms " #fe"
Sleep 0.2s
Right 1
Sleep 0.5s

Enter
Sleep 0.5s

Up@750ms 5
Down@750ms 1
Sleep 0.5s

Type "c"
Sleep 1.5s

Type "/"
Sleep 0.5s

Type "!tasks"
Sleep 0.3s
Enter
Sleep 2.5s

Type "/"
Sleep 1.5s

Type " #fea"
Sleep 0.3s
Right 1
Sleep 0.7s
Enter
Sleep 4s

Hide
Escape
Sleep 0.3s
Type "G"
Sleep 0.3s
Type "d"
Sleep 0.3s
Up 3
Sleep 0.3s
Type "c"
Sleep 0.3s
