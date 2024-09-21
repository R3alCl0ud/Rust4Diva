; The name of the installer
Name "Rust4Diva"

; To change from default installer icon:
Icon "..\assets\rust4diva.ico"

!define VERSION "/Git_Commit_Hash/g"

; The setup filename
OutFile "output\Rust4Diva ${VERSION} installer.exe"

; The default installation directory
InstallDir $PROGRAMFILES64\Rust4Diva

; Registry key to check for directory (so if you install again, it will 
; overwrite the old one automatically)
InstallDirRegKey HKLM "Software\Rust4Diva" "Install_Dir"

RequestExecutionLevel admin

;--------------------------------

; Pages

Page components
Page directory
Page instfiles

UninstPage uninstConfirm
UninstPage instfiles

;--------------------------------

; The stuff to install
Section "Rust4Diva"

  SectionIn RO
  
  ; Set output path to the installation directory.
  SetOutPath $INSTDIR
  
  ; Put file there (you can add more File lines too)
  File "..\target\debug\rust4diva.exe"
  File "libarchive.dll"
  File "..\assets\rust4diva.png"
  ; Wildcards are allowed:
  ; File *.dll
  ; To add a folder named MYFOLDER and all files in it recursively, use this EXACT syntax:
  ; File /r MYFOLDER\*.*
  ; See: https://nsis.sourceforge.io/Reference/File
  ; MAKE SURE YOU PUT ALL THE FILES HERE IN THE UNINSTALLER TOO
  
  ; Write the installation path into the registry
  WriteRegStr HKLM SOFTWARE\Rust4Diva "Install_Dir" "$INSTDIR"
  WriteRegStr HKCR divamodmanager "URL Protocol" ""
  
  ; Write the uninstall keys for Windows
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rust4Diva" "DisplayName" "Rust4Diva"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rust4Diva" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rust4Diva" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rust4Diva" "NoRepair" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
  
SectionEnd

; Optional section (can be disabled by the user)
Section "Start Menu Shortcuts (required)"
  SectionIn RO

  CreateDirectory "$SMPROGRAMS\Rust4Diva"
  CreateShortcut "$SMPROGRAMS\Rust4Diva\Uninstall.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
  CreateShortcut "$SMPROGRAMS\Rust4Diva\rust4diva.lnk" "$INSTDIR\rust4diva.exe" "" "$INSTDIR\rust4diva.exe" 0
  
SectionEnd

;--------------------------------

; Uninstaller

Section "Uninstall"
  
  ; Remove registry keys
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rust4Diva"
  DeleteRegKey HKLM SOFTWARE\Rust4Diva

  ; Remove files and uninstaller
  ; MAKE SURE NOT TO USE A WILDCARD. IF A
  ; USER CHOOSES A STUPID INSTALL DIRECTORY,
  ; YOU'LL WIPE OUT OTHER FILES TOO
  Delete $INSTDIR\Rust4Diva.exe
  Delete $INSTDIR\uninstall.exe

  ; Remove shortcuts, if any
  Delete "$SMPROGRAMS\Rust4Diva\*.*"

  ; Remove directories used (only deletes empty dirs)
  RMDir "$SMPROGRAMS\Rust4Diva"
  RMDir "$INSTDIR"

SectionEnd