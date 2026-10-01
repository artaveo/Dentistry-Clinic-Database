; Phase 0 LAN spike: open the LAN port for the app only, on Private networks,
; so clinic staff never touch Windows Firewall (roadmap: "no IT needed").
!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental LAN"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Artaveo Dental LAN" dir=in action=allow program="$INSTDIR\artaveo-spike.exe" enable=yes profile=private,domain protocol=TCP localport=47800'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Artaveo Dental mDNS" dir=in action=allow program="$INSTDIR\artaveo-spike.exe" enable=yes profile=private,domain protocol=UDP localport=5353'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental LAN"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Artaveo Dental mDNS"'
!macroend
