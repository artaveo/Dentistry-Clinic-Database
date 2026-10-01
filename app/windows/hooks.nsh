; Machine-wide data folder shared by every Windows account (Config::from_env,
; ADR-04 machine-scope DPAPI). Users get Modify so staff accounts can work;
; uninstall never deletes clinic data (roadmap 11.1 Data Preservation).
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$COMMONPROGRAMDATA\ArtaveoDental"
  nsExec::ExecToLog 'icacls "$COMMONPROGRAMDATA\ArtaveoDental" /grant *S-1-5-32-545:(OI)(CI)M /T /Q'
!macroend
