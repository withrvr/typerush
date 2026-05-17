Output assets/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1000
Set Height 550
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 80ms
Set Framerate 30
Set PlaybackSpeed 1.0
Set CursorBlink false

# Launch your app
Hide
Type "./target/release/typerush --words 25"
Enter
Sleep 2s
Show

# Type some demo text (this simulates you typing in the app)
Type "the quick brown fox jumps over the lazy dog"
Sleep 500ms
Type " rust is blazingly fast and memory safe"
Sleep 1s

# Let results screen show
Sleep 3s
Screenshot assets/screenshot-results.png
Sleep 1s
