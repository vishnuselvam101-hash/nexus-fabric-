NEXUS FABRIC - README (development build)
=========================================

Development build. Not validated. Outputs must not be used for manufacturing.
All rights reserved. Evaluation use only — terms to be provided by the owner.

WHAT IT IS
NEXUS FABRIC runs a server on this computer and shows it in your web browser.
Your data stays in the data folder described below.

STARTING IT
- Windows, installed: Start menu > NEXUS FABRIC.
- Windows, portable: open a command prompt in this folder and run
      nexus-fabric.exe --desktop
- Linux: run  ./nexus-fabric.sh  in this folder.

The browser opens the address shown in the console window (normally
http://127.0.0.1:8787/, or another free port if that one is busy). Keep the
console window open while you work. To stop, press Ctrl+C in it or close the
window. The database is closed cleanly when the program stops.

FIRST RUN
The setup page opens with its one-time token already filled in. Enter a user
name for the administrator and a password of at least 10 characters. The token
is used once and is not shown in the console. If the program is started again
later, the setup link changes; setup happens only while no administrator exists.

WHERE YOUR DATA IS
- Windows: %LOCALAPPDATA%\NexusFabric\data
- Linux:   $XDG_DATA_HOME/nexus-fabric (or ~/.local/share/nexus-fabric)

The server listens on 127.0.0.1 only, so only this computer can reach it. A
second copy of the program on the same data folder is refused with a message.

UNINSTALLING
- Installed version: Settings > Apps > NEXUS FABRIC, or the "Uninstall NEXUS
  FABRIC" shortcut in the Start menu. The data folder is kept unless you tick
  "Also delete my data".
- Portable version: delete this folder. Your data stays in the data folder
  above until you delete it.

SMARTSCREEN AND CHECKING THE FILES
The files are not code-signed. Windows may show "Windows protected your PC".
Only continue if the file came from the place you expected. Check it against
SHA256SUMS before running it:
- Windows (PowerShell):  Get-FileHash <file> -Algorithm SHA256
- Linux:                 sha256sum -c SHA256SUMS

This program does not update itself.
