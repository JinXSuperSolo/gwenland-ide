Unicode true
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
  SetCompress off
!else
  SetCompressor /SOLID "{{compression}}"
!endif

!include MUI2.nsh
!include FileFunc.nsh
!include LogicLib.nsh

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define RELEASE_API "https://api.github.com/repos/JinXSuperSolo/gwenland-ide/releases/latest"
!define RELEASE_PAGE "https://github.com/JinXSuperSolo/gwenland-ide/releases/latest"
!define RELEASE_BINARY "https://github.com/JinXSuperSolo/gwenland-ide/releases/latest/download/GwenLand-IDE.exe"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"

Name "${PRODUCTNAME}"
BrandingText "${COPYRIGHT}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\${PRODUCTNAME}"
RequestExecutionLevel user

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinary
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetShellVarContext current
  ReadRegStr $0 HKCU "${MANUPRODUCTKEY}" ""
  ${If} $0 != ""
    StrCpy $INSTDIR $0
  ${EndIf}
FunctionEnd

Function un.onInit
  SetShellVarContext current
FunctionEnd

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  CreateDirectory "$INSTDIR"

  Delete "$TEMP\gwenland-release.json"
  Delete "$TEMP\gwenland-release.html"

  DetailPrint "Querying GwenLand IDE release metadata"
  NSISdl::download /TIMEOUT=30000 "${RELEASE_API}" "$TEMP\gwenland-release.json"
  Pop $0
  ${If} $0 != "success"
    DetailPrint "GitHub API query failed: $0"
    DetailPrint "Querying the GitHub Releases page"
    NSISdl::download /TIMEOUT=30000 "${RELEASE_PAGE}" "$TEMP\gwenland-release.html"
    Pop $0
    ${If} $0 != "success"
      Abort "Could not query the latest GwenLand IDE release."
    ${EndIf}
  ${EndIf}

  Delete "$TEMP\${MAINBINARYNAME}.download.exe"
  DetailPrint "Downloading latest GwenLand IDE binary"
  NSISdl::download /TIMEOUT=30000 "${RELEASE_BINARY}" "$TEMP\${MAINBINARYNAME}.download.exe"
  Pop $0
  ${If} $0 != "success"
    Delete "$TEMP\${MAINBINARYNAME}.download.exe"
    Abort "Could not download the latest GwenLand IDE binary."
  ${EndIf}

  CopyFiles /SILENT "$TEMP\${MAINBINARYNAME}.download.exe" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${If} ${Errors}
    Delete "$TEMP\${MAINBINARYNAME}.download.exe"
    Abort "Could not install GwenLand IDE into the selected folder."
  ${EndIf}
  Delete "$TEMP\${MAINBINARYNAME}.download.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "${MANUPRODUCTKEY}" "" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" "1"
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" "1"

  ${GetSize} "$INSTDIR" "/M=uninstall.exe /S=0K /G=0" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" "$0"

  !if "${HOMEPAGE}" != ""
    WriteRegStr HKCU "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
    WriteRegStr HKCU "${UNINSTKEY}" "URLUpdateInfo" "${HOMEPAGE}"
  !endif

  CreateDirectory "$SMPROGRAMS\${PRODUCTNAME}"
  CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
SectionEnd

Function RunMainBinary
  ExecShell "open" "$INSTDIR\${MAINBINARYNAME}.exe"
FunctionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$INSTDIR\${MAINBINARYNAME}.exe"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\${PRODUCTNAME}\${PRODUCTNAME}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCTNAME}"
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
  DeleteRegKey /ifempty HKCU "${MANUKEY}"
  RMDir "$INSTDIR"
SectionEnd
