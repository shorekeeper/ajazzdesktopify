# Reads the per-key RGB table (CMD 0x14) and displays colors
# for all configured keys.

Write-Host "Reading per-key RGB table (CMD 0x14)..." -ForegroundColor Cyan
Write-Host ""

$output = cargo run --release --bin ak680-probe -- read 14 00 10 2>&1

$combined = $output | Select-String "Combined data" -Context 0,9999 |
    ForEach-Object { $_.Context.PostContext } |
    Where-Object { $_ -and $_ -notmatch "^\s*$" }

# Parse hex dump into byte array.
$allBytes = @()
foreach ($line in $combined) {
    if ($line -match "^\s+[0-9A-F]{4}:\s+(.+?)\s{2}") {
        $hexPart = $Matches[1]
        $hexBytes = $hexPart -split '\s+' | Where-Object { $_ -match '^[0-9A-Fa-f]{2}$' }
        $allBytes += $hexBytes | ForEach-Object { [Convert]::ToByte($_, 16) }
    }
}

if ($allBytes.Length -eq 0) {
    Write-Host "No data received." -ForegroundColor Red
    exit 1
}

Write-Host "Parsed $($allBytes.Length) bytes ($([math]::Floor($allBytes.Length / 4)) keys)" -ForegroundColor DarkGray
Write-Host ""

# Key name lookup (matching ak680max key list).
$keyNames = @{}
$keyNames[0] = "Escape"; $keyNames[17] = "1"; $keyNames[18] = "2"
$keyNames[19] = "3"; $keyNames[20] = "4"; $keyNames[21] = "5"
$keyNames[22] = "6"; $keyNames[23] = "7"; $keyNames[24] = "8"
$keyNames[25] = "9"; $keyNames[26] = "0"; $keyNames[27] = "Minus"
$keyNames[28] = "Equal"; $keyNames[32] = "Tab"; $keyNames[33] = "Q"
$keyNames[34] = "W"; $keyNames[35] = "E"; $keyNames[36] = "R"
$keyNames[37] = "T"; $keyNames[38] = "Y"; $keyNames[39] = "U"
$keyNames[40] = "I"; $keyNames[41] = "O"; $keyNames[42] = "P"
$keyNames[43] = "["; $keyNames[44] = "]"; $keyNames[48] = "Caps"
$keyNames[49] = "A"; $keyNames[50] = "S"; $keyNames[51] = "D"
$keyNames[52] = "F"; $keyNames[53] = "G"; $keyNames[54] = "H"
$keyNames[55] = "J"; $keyNames[56] = "K"; $keyNames[57] = "L"
$keyNames[58] = ";"; $keyNames[59] = "'"; $keyNames[60] = "\"
$keyNames[64] = "LShift"; $keyNames[65] = "Z"; $keyNames[66] = "X"
$keyNames[67] = "C"; $keyNames[68] = "V"; $keyNames[69] = "B"
$keyNames[70] = "N"; $keyNames[71] = "M"; $keyNames[72] = ","
$keyNames[73] = "."; $keyNames[74] = "/"; $keyNames[75] = "RShift"
$keyNames[76] = "Enter"; $keyNames[80] = "LCtrl"; $keyNames[81] = "Win"
$keyNames[82] = "LAlt"; $keyNames[83] = "Space"; $keyNames[84] = "RAlt"
$keyNames[85] = "Fn"; $keyNames[87] = "RCtrl"; $keyNames[88] = "Left"
$keyNames[89] = "Down"; $keyNames[90] = "Up"; $keyNames[91] = "Right"
$keyNames[92] = "Bksp"; $keyNames[104] = "Home"; $keyNames[105] = "PgUp"
$keyNames[106] = "Del"; $keyNames[108] = "PgDn"

$colored = 0
$total = 0

Write-Host ("{0,-6} {1,-8} {2,3} {3,3} {4,3}  {5}" -f "Code", "Key", "R", "G", "B", "Hex")
Write-Host ("-" * 45)

for ($key = 0; $key -lt [math]::Floor($allBytes.Length / 4); $key++) {
    $idx = $allBytes[$key * 4]
    $r = $allBytes[$key * 4 + 1]
    $g = $allBytes[$key * 4 + 2]
    $b = $allBytes[$key * 4 + 3]

    $name = if ($keyNames.ContainsKey($key)) { $keyNames[$key] } else { "" }
    if (-not $name) { continue }
    $total++

    if ($r -ne 0 -or $g -ne 0 -or $b -ne 0) {
        $hex = "#{0:X2}{1:X2}{2:X2}" -f $r, $g, $b
        Write-Host ("{0,-6} {1,-8} {2,3} {3,3} {4,3}  {5}" -f $key, $name, $r, $g, $b, $hex)
        $colored++
    }
}

Write-Host ""
Write-Host "$colored of $total keys have custom colors" -ForegroundColor Green