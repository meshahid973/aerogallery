$ErrorActionPreference = 'Stop'
$extensions = @('jpg','jpeg','png','webp','gif','bmp','tif','tiff','ico','tga','qoi','pnm','ppm','pgm','pbm')
$classes = 'HKCU:\Software\Classes'
$registered = 'HKCU:\Software\RegisteredApplications'
$capabilities = 'HKCU:\Software\AeroGallery\Capabilities\FileAssociations'
$installer = Get-ChildItem 'src-tauri/target/release/bundle/nsis/*-setup.exe' | Select-Object -First 1
if (!$installer) { throw 'NSIS installer is missing.' }
$installDir = Join-Path $env:RUNNER_TEMP 'AeroGalleryInstallerTest'
function Value($path, $name) {
    if (Test-Path $path) { return (Get-Item $path).GetValue($name, $null) }
    return $null
}
$original = @{}
foreach ($ext in $extensions) {
    $original[$ext] = @( (Value "$classes\.$ext" ''), (Value "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.$ext\UserChoice" 'ProgId') )
}
try {
    $process = Start-Process -FilePath $installer.FullName -ArgumentList '/S', "/D=$installDir" -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
    if (!(Test-Path "$installDir\aerogallery.exe")) { throw 'Installed executable missing.' }
    if ((Value $registered 'AeroGallery') -ne 'Software\AeroGallery\Capabilities') { throw 'Default Apps registration missing.' }
    $command = Value "$classes\AeroGallery.Picture\shell\open\command" ''
    if ($command -ne ('"{0}\aerogallery.exe" "%1"' -f $installDir)) { throw "Invalid shell command: $command" }
    foreach ($ext in $extensions) {
        if ((Value $capabilities ".$ext") -ne 'AeroGallery.Picture') { throw "Capability missing: $ext" }
        if ((Value "$classes\.$ext\OpenWithProgids" 'AeroGallery.Picture') -ne '') { throw "Open with missing: $ext" }
        if ((Value "$classes\.$ext" '') -ne $original[$ext][0]) { throw "Default changed: $ext" }
        if ((Value "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.$ext\UserChoice" 'ProgId') -ne $original[$ext][1]) { throw "UserChoice changed: $ext" }
    }
} finally {
    if (Test-Path "$installDir\uninstall.exe") {
        $process = Start-Process -FilePath "$installDir\uninstall.exe" -ArgumentList '/S' -PassThru -Wait
        if ($process.ExitCode -ne 0) { throw "Uninstall failed: $($process.ExitCode)" }
    }
}
if (Test-Path "$classes\AeroGallery.Picture") { throw 'ProgID remains after uninstall.' }
if ($null -ne (Value $registered 'AeroGallery')) { throw 'Default Apps registration remains.' }
foreach ($ext in $extensions) {
    if ($null -ne (Value "$classes\.$ext\OpenWithProgids" 'AeroGallery.Picture')) { throw "Open with remains: $ext" }
    if ((Value "$classes\.$ext" '') -ne $original[$ext][0]) { throw "Default changed after uninstall: $ext" }
    if ((Value "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.$ext\UserChoice" 'ProgId') -ne $original[$ext][1]) { throw "UserChoice changed after uninstall: $ext" }
}
Write-Host 'Installer and uninstaller registration checks passed.'
