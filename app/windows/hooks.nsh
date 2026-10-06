; Machine-wide data folder shared by every Windows account (Config::from_env,
; ADR-04 machine-scope DPAPI). Users get Modify so staff accounts can work;
; uninstall never deletes clinic data (roadmap 11.1 Data Preservation).
;
; OF-026: after an upgrade Windows kept the old logo on the desktop icon for up to two days,
; because its icon cache still pointed at the previous build. The shortcuts are therefore
; rewritten with an explicit icon path, and the icon cache is refreshed at once.
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$COMMONPROGRAMDATA\ArtaveoDental"
  nsExec::ExecToLog 'icacls "$COMMONPROGRAMDATA\ArtaveoDental" /grant *S-1-5-32-545:(OI)(CI)M /T /Q'

  IfFileExists "$DESKTOP\Artaveo Dental.lnk" 0 +2
  CreateShortCut "$DESKTOP\Artaveo Dental.lnk" "$INSTDIR\artaveo-dental.exe" "" "$INSTDIR\artaveo-dental.exe" 0
  IfFileExists "$SMPROGRAMS\Artaveo Dental.lnk" 0 +2
  CreateShortCut "$SMPROGRAMS\Artaveo Dental.lnk" "$INSTDIR\artaveo-dental.exe" "" "$INSTDIR\artaveo-dental.exe" 0
  nsExec::ExecToLog 'ie4uinit.exe -show'
!macroend
