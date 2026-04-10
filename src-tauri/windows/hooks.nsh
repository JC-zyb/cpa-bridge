!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$APPDATA\cpa-bridge"
!macroend
