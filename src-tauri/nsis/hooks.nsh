!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\data\*.*" 0 done
    MessageBox MB_YESNO|MB_ICONQUESTION "是否删除应用数据目录？$\r$\n$\r$\n$INSTDIR\data$\r$\n$\r$\n选择“是”将删除设置、日志、缓存和配音素材；选择“否”将保留这些数据。" IDNO done
    RMDir /r "$INSTDIR\data"
  done:
!macroend
