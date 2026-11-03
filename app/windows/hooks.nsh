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

  ; Phase 7 (LAN mode, ADR-18): the clinic server must be reachable without anybody touching the
  ; Windows Firewall. The rules cover only this program, only on Private/Domain networks (never a
  ; café's Public network): TCP for the LAN server and UDP 5353 for the mDNS announcement.
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental LAN"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental LAN discovery"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Artaveo Dental LAN" dir=in action=allow program="$INSTDIR\artaveo-dental.exe" enable=yes profile=private,domain protocol=tcp'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Artaveo Dental LAN discovery" dir=in action=allow program="$INSTDIR\artaveo-dental.exe" enable=yes profile=private,domain protocol=udp localport=5353'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental LAN"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental LAN discovery"'
!macroend
