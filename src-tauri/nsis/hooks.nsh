!macro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  !searchreplace LINK_MAIN_BINARY_DIR "${MAINBINARYSRCPATH}" "\${MAINBINARYNAME}.exe" ""
  SetOutPath "$INSTDIR"
  File "${LINK_MAIN_BINARY_DIR}\c10.dll"
  File /nonfatal "${LINK_MAIN_BINARY_DIR}\cupti64_*.dll"
  File "${LINK_MAIN_BINARY_DIR}\libiomp5md.dll"
  File "${LINK_MAIN_BINARY_DIR}\torch_cpu.dll"
  !undef LINK_MAIN_BINARY_DIR
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  Delete "$INSTDIR\c10.dll"
  Delete "$INSTDIR\cupti64_*.dll"
  Delete "$INSTDIR\libiomp5md.dll"
  Delete "$INSTDIR\torch_cpu.dll"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; Remove installer/app registry state that can outlive the standard uninstall cleanup.
    DeleteRegValue HKCU "${MANUPRODUCTKEY}" "Installer Language"
    DeleteRegKey SHCTX "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty SHCTX "${MANUKEY}"
    DeleteRegKey HKCU "${UNINSTKEY}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  ${EndIf}

  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RMDir /r "$INSTDIR\data"
    RMDir "$INSTDIR"
  ${EndIf}
!macroend
