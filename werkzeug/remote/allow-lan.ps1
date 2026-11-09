# Run in an elevated Windows PowerShell after starting the remote listener.
param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][System.Net.IPAddress]$LocalAddress,
    [ValidateRange(1, 65535)][int]$Port = 7337,
    [switch]$Remove
)
$ErrorActionPreference = 'Stop'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Windows requires an Administrator PowerShell to configure its firewall.'
}
$binary = (Resolve-Path -LiteralPath $Executable).ProviderPath
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'Executable must be a file.' }
$ruleName = "OrchesterRemote-$LocalAddress-$Port"
$existing = Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue
if ($Remove) {
    if ($existing) { $existing | Remove-NetFirewallRule }
    return
}
if ($existing) { throw "Rule $ruleName already exists. Inspect it or remove it with -Remove before replacing it." }
# Restrict inbound traffic to this executable, port, local IP and local subnet.
# Public profiles are common on home Wi-Fi too; never change the network profile.
New-NetFirewallRule -Name $ruleName -DisplayName "Orchester Remote ($LocalAddress : $Port)" `
    -Direction Inbound -Action Allow -Enabled True -Protocol TCP -Profile Any `
    -Program $binary -LocalAddress $LocalAddress.IPAddressToString -LocalPort $Port `
    -RemoteAddress LocalSubnet
