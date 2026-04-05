# Analyzes a firmware dump file for RGB tables, LED mappings,
# and gamma LUT data.

param(
    [string]$Path = "firmware.bin"
)

if (-not (Test-Path $Path)) {
    Write-Host "File not found: $Path" -ForegroundColor Red
    exit 1
}

$bytes = [System.IO.File]::ReadAllBytes($Path)
Write-Host "Loaded $($bytes.Length) bytes from $Path" -ForegroundColor Green
Write-Host ""

# RGB color table: 12 rainbow colors at offset 0x7ED2, 3 bytes each.
Write-Host "=== RGB Animation Palette (12 colors at 0x7ED2) ===" -ForegroundColor Cyan
$offset = 0x7ED2
for ($key = 0; $key -lt 12; $key++) {
    $addr = $offset + $key * 3
    if ($addr + 3 -gt $bytes.Length) { break }
    $r = $bytes[$addr]
    $g = $bytes[$addr + 1]
    $b = $bytes[$addr + 2]
    Write-Host ("  Color {0,2}: R={1,3} G={2,3} B={3,3}  #{1:X2}{2:X2}{3:X2}" -f $key, $r, $g, $b)
}
Write-Host ""

# LED index mapping table at approximate offset 0x7F52.
Write-Host "=== LED-to-Key Mapping (at ~0x7F52) ===" -ForegroundColor Cyan
$mapOffset = 0x7F52
$entries = 0
for ($i = 0; $i -lt 128; $i++) {
    $addr = $mapOffset + $i
    if ($addr -ge $bytes.Length) { break }
    $val = $bytes[$addr]
    if ($val -ne 0 -and $val -ne 0x78) {
        Write-Host ("  LED [{0,3}] -> Key 0x{1:X2} ({1})" -f $i, $val)
        $entries++
    }
}
Write-Host "  $entries active mappings (0x78 = unused)" -ForegroundColor DarkGray
Write-Host ""

# Gamma / brightness LUT at offset 0x7FC2, 64 entries.
Write-Host "=== Gamma LUT (64 entries at 0x7FC2) ===" -ForegroundColor Cyan
$lutOffset = 0x7FC2
$lut = @()
for ($i = 0; $i -lt 64; $i++) {
    $addr = $lutOffset + $i
    if ($addr -ge $bytes.Length) { break }
    $lut += $bytes[$addr]
}
$line = ($lut | ForEach-Object { '{0,3}' -f $_ }) -join ' '
Write-Host "  $line"
Write-Host ""

# Flash address mapping reference.
Write-Host "=== Flash Address Mapping ===" -ForegroundColor Cyan
Write-Host "  CMD 0x10: flash 0x9000 - DeviceInfo"
Write-Host "  CMD 0x11: flash 0x9200 - Device Config"
Write-Host "  CMD 0x12: flash 0x9600 - Key Mapping"
Write-Host "  CMD 0x14: flash 0x9A00 - Per-key RGB"
Write-Host "  CMD 0x15: flash 0x9C00 - LED Animation"
Write-Host "  CMD 0x16: flash 0xB000 - Unknown"
Write-Host "  CMD 0x17: flash 0xB600 - Press Actuation"
Write-Host "  CMD 0x18: flash 0xB200 - Release Actuation"
Write-Host "  RGB palette:  flash 0x134D2"
Write-Host "  LED mapping:  flash 0x13552"
Write-Host "  Gamma LUT:    flash 0x135C2"
Write-Host ""
Write-Host "  Dump base (CMD 0x17 offset 0) = flash 0xB600"
Write-Host "  To convert: flash_addr = file_offset + 0xB600"