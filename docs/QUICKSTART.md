# Welcome to Mote

Extract the whole folder and open **Mote.exe**. Pick an animal, add up to four
pets to your lineup, then choose **Bring them home**. No account, installation,
downloads, or internet connection are needed.

The pets live on your taskbar. Quiet mode is on initially: they keep away from
your application windows and leave your cursor alone. They hide automatically
while a fullscreen application is active.

- Double-click the tray icon, or open Mote again, to choose your pets.
- Right-click a pet or its tray icon for sleep, hide, size, and movement settings.
- **Ctrl+Alt+M** hides or shows the pets, unless another app uses that shortcut.
- Enable **Let clicks pass through pets** when you want no mouse interaction.
- Click gently to pet; drag and release to move a pet.
- **Quit Mote** in the tray closes the application completely.

The picker supports arrow keys, Space, Tab, Enter, and Escape. Its Quiet and
Reduce motion options apply when you bring the pets home. Turning Quiet off
enables optional cursor, window, system-load, and audio reactions; these can
also be adjusted individually from the tray.

Settings are saved in `%APPDATA%\Mote\settings.json`. A small local diagnostic
log is in `%LOCALAPPDATA%\Mote\mote.log`. Audio reactions read playback status
and a volume meter; no sound is recorded. No data leaves the computer.

Launch at startup is optional and off by default. If enabled, Windows starts
your saved pets quietly without reopening the picker. Keep the executable at
the same path, or toggle startup off and on after moving it.

This portable build is unsigned. To remove it, turn off Launch at startup,
quit Mote, and delete the extracted folder. Your settings remain available
if you reinstall.
