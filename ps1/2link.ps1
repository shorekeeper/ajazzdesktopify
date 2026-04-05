$cmd10 = [System.IO.File]::ReadAllBytes("../dump_cmd10.bin")
$cmd17 = [System.IO.File]::ReadAllBytes("../firmware.bin")

# cmd10[0..0x25FF] = flash 0x9000..0xB5FF (9728 bytes)
# cmd17 = flash 0xB600..0x1B5FF (65536 bytes)

$prefixLen = 0x2600
$prefix = New-Object byte[] $prefixLen
[Array]::Copy($cmd10, 0, $prefix, 0, $prefixLen)

$combined = New-Object byte[] ($prefixLen + $cmd17.Length)
[Array]::Copy($prefix, 0, $combined, 0, $prefixLen)
[Array]::Copy($cmd17, 0, $combined, $prefixLen, $cmd17.Length)

[System.IO.File]::WriteAllBytes("flash_full.bin", $combined)
Write-Host "Size: $($combined.Length) bytes covering flash 0x9000-0x1B5FF"