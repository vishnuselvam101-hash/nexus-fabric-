; NEXUS FABRIC per-user Windows installer. No administrator rights needed.
; Development build. Not validated. Outputs must not be used for manufacturing.
;
; Build (normally through packaging/windows/build.sh):
;   makensis -DVERSION=0.1.0 -DSTAGE=<folder with nexus-fabric.exe, README.txt, NOTICE.txt, THIRD_PARTY_NOTICES.txt> -DOUTFILE=<setup.exe> packaging/windows/installer.nsi

Unicode true
Target amd64-unicode
SetCompressor /SOLID lzma
RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show

!ifndef VERSION
  !error "VERSION is not defined: pass -DVERSION=x.y.z"
!endif
!ifndef STAGE
  !error "STAGE is not defined: pass -DSTAGE=<folder>"
!endif
!ifndef OUTFILE
  !error "OUTFILE is not defined: pass -DOUTFILE=<file>"
!endif

!define APPNAME "NEXUS FABRIC"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\NexusFabric"
!define APP_KEY "Software\NexusFabric"

Name "${APPNAME} ${VERSION} (development build)"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\NexusFabric"
InstallDirRegKey HKCU "${APP_KEY}" "InstallDir"
BrandingText "NEXUS FABRIC ${VERSION} - development build. Not validated."

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "NEXUS FABRIC"
VIAddVersionKey "FileDescription" "NEXUS FABRIC installer (development build)"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "All rights reserved. Evaluation use only — terms to be provided by the owner."

!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"

Var DeleteData
Var DeleteCheckbox

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "NEXUS FABRIC ${VERSION}: development build"
!define MUI_WELCOMEPAGE_TEXT "Development build. Not validated. Outputs must not be used for manufacturing.$\r$\n$\r$\nAll rights reserved. Evaluation use only — terms to be provided by the owner.$\r$\n$\r$\nThis installer runs without administrator rights and installs for the current user only. Your data is kept in %LOCALAPPDATA%\NexusFabric\data when you uninstall, unless you choose to delete it.$\r$\n$\r$\nClick Next to continue."
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Start NEXUS FABRIC now (opens the browser)"
!define MUI_FINISHPAGE_RUN_FUNCTION RunNexusFabric

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
UninstPage custom un.DataPage un.DataPageLeave
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function RunNexusFabric
  Exec '"$INSTDIR\nexus-fabric.exe" --desktop'
FunctionEnd

Section "NEXUS FABRIC (required)" SEC_APP
  SectionIn RO
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "${STAGE}\nexus-fabric.exe"
  File "${STAGE}\README.txt"
  File "${STAGE}\NOTICE.txt"
  File "${STAGE}\THIRD_PARTY_NOTICES.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\NEXUS FABRIC"
  CreateShortCut "$SMPROGRAMS\NEXUS FABRIC\NEXUS FABRIC.lnk" "$INSTDIR\nexus-fabric.exe" "--desktop" "$INSTDIR\nexus-fabric.exe" 0 SW_SHOWNORMAL "" "NEXUS FABRIC (development build)"
  CreateShortCut "$SMPROGRAMS\NEXUS FABRIC\Uninstall NEXUS FABRIC.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "${APP_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "NEXUS FABRIC (development build)"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\nexus-fabric.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Desktop shortcut" SEC_DESKTOP
  SetShellVarContext current
  CreateShortCut "$DESKTOP\NEXUS FABRIC.lnk" "$INSTDIR\nexus-fabric.exe" "--desktop" "$INSTDIR\nexus-fabric.exe" 0 SW_SHOWNORMAL "" "NEXUS FABRIC (development build)"
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$INSTDIR\nexus-fabric.exe"
  IfFileExists "$INSTDIR\nexus-fabric.exe" 0 +3
    MessageBox MB_OK|MB_ICONEXCLAMATION "NEXUS FABRIC is still running. Stop it (Ctrl+C in its console window), then run the uninstaller again."
    Abort
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\NOTICE.txt"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\NEXUS FABRIC\NEXUS FABRIC.lnk"
  Delete "$SMPROGRAMS\NEXUS FABRIC\Uninstall NEXUS FABRIC.lnk"
  RMDir "$SMPROGRAMS\NEXUS FABRIC"
  Delete "$DESKTOP\NEXUS FABRIC.lnk"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "${APP_KEY}"

  ${If} $DeleteData == 1
    RMDir /r "$LOCALAPPDATA\NexusFabric\data"
    RMDir "$LOCALAPPDATA\NexusFabric"
  ${EndIf}
SectionEnd

Function un.DataPage
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${NSD_CreateLabel} 0 0 100% 48u "Your NEXUS FABRIC data is kept in $LOCALAPPDATA\NexusFabric\data. It is kept when you uninstall, so a later installation finds it again. Tick the box only if you want that data deleted as well. This cannot be undone."
  Pop $0
  ${NSD_CreateCheckbox} 0 56u 100% 12u "Also delete my data"
  Pop $DeleteCheckbox
  nsDialogs::Show
FunctionEnd

Function un.DataPageLeave
  ${NSD_GetState} $DeleteCheckbox $DeleteData
FunctionEnd
