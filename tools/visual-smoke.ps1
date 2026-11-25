# Repeatable native-window visual smoke for Nexora.
#
#   powershell -ExecutionPolicy Bypass -File tools\visual-smoke.ps1
#   powershell -ExecutionPolicy Bypass -File tools\visual-smoke.ps1 -Inspection
#
# Captures a real screenshot plus the UI Automation control list into
# artifacts\visual\<timestamp>\. With -Inspection the app is launched with
# EGUI_INSPECTION=1 so the egui-inspection MCP server can attach on loopback.
# No browser is involved.

param(
    [switch]$Inspection,
    [string]$VaultName = "visual-smoke-vault"
)

$ErrorActionPreference = "Stop"
$project = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $project "target\debug\nexora-app.exe"
if (-not (Test-Path $exe)) {
    throw "Build first: cargo build -p nexora-app"
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$out = Join-Path $project "artifacts\visual\$stamp"
New-Item -ItemType Directory -Force -Path $out | Out-Null

# Isolated vault: never touch the real Documents\Nexora vault.
$tempRoot = Join-Path $env:TEMP "nexora-$VaultName"
$vault = Join-Path $tempRoot "Documents\Nexora\my-first-vault"
New-Item -ItemType Directory -Force -Path $vault | Out-Null
$env:USERPROFILE = $tempRoot
$env:APPDATA = Join-Path $tempRoot "AppData\Roaming"
if ($Inspection) { $env:EGUI_INSPECTION = "1" } else { Remove-Item Env:\EGUI_INSPECTION -ErrorAction SilentlyContinue }

Get-Process nexora-app -ErrorAction SilentlyContinue | Stop-Process -Force
$proc = Start-Process -FilePath $exe -PassThru -RedirectStandardError (Join-Path $out "stderr.txt")
Start-Sleep -Seconds 3

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$window = [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle)
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
$bmp.Save((Join-Path $out "window.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

$controls = @()
$all = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
foreach ($item in $all) {
    $controls += [pscustomobject]@{
        role  = $item.Current.ControlType.ProgrammaticName
        name  = $item.Current.Name
        value = $item.Current.HelpText
    }
}
$controls | Export-Csv -NoTypeInformation -Encoding UTF8 (Join-Path $out "uia-controls.csv")

"Artifacts: $out"
"Screenshot: $(Join-Path $out 'window.png')"
"UIA controls: $($controls.Count)"
"Inspection port: $(if ($Inspection) { '127.0.0.1:5719' } else { 'disabled' })"
