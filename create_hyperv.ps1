param(
    [Parameter(Mandatory=$true)]
    [string]$IsoPath
)

if (-not (Test-Path $IsoPath)) {
    Write-Error "ISO file not found: $IsoPath"
    exit 1
}

# Remove the duplicate switch we accidentally created
Get-VMSwitch -Name "Default Switch" -SwitchType Internal -ErrorAction SilentlyContinue | Remove-VMSwitch -Force

# Create the VHD directory
New-Item -ItemType Directory -Path "C:\HyperV\GodlyDev" -Force

# Create the VM using the built-in Default Switch
New-VM -Name "GodlyDev" -MemoryStartupBytes 8GB -Generation 2 -NewVHDPath "C:\HyperV\GodlyDev\GodlyDev.vhdx" -NewVHDSizeBytes 100GB -SwitchName "Default Switch"

# Configure it
Set-VM -Name "GodlyDev" -ProcessorCount 4 -CheckpointType Standard -AutomaticCheckpointsEnabled $false
Set-VMFirmware -VMName "GodlyDev" -EnableSecureBoot Off

# Mount your ISO
Add-VMDvdDrive -VMName "GodlyDev" -Path $IsoPath
# Boot from DVD first
$dvd = Get-VMDvdDrive -VMName "GodlyDev"
Set-VMFirmware -VMName "GodlyDev" -FirstBootDevice $dvd

# TPM for Win11
Set-VMKeyProtector -VMName "GodlyDev" -NewLocalKeyProtector
Enable-VMTPM -VMName "GodlyDev"