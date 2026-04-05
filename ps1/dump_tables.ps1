# Reads all known configuration tables from the keyboard and saves
# non-zero lines to tables_dump.txt.

$commands = @(
    @{ cmd = "11"; chunks = 10; name = "DeviceConfig" },
    @{ cmd = "12"; chunks = 20; name = "KeyMapping" },
    @{ cmd = "14"; chunks = 20; name = "PerKeyRGB" },
    @{ cmd = "15"; chunks = 20; name = "LEDAnimation" },
    @{ cmd = "16"; chunks = 20; name = "Table16" },
    @{ cmd = "17"; chunks = 20; name = "PressActuation" },
    @{ cmd = "18"; chunks = 20; name = "ReleaseActuation" }
)

$outFile = "tables_dump.txt"
"" | Set-Content $outFile

foreach ($c in $commands) {
    $header = "=== CMD 0x$($c.cmd) ($($c.name)) ==="
    Write-Host $header -ForegroundColor Cyan
    $header | Add-Content $outFile

    $output = cargo run --release --bin ak680-probe -- read $c.cmd 00 $c.chunks 2>&1 |
        Select-String "Combined data" -Context 0,9999 |
        ForEach-Object { $_.Context.PostContext } |
        Where-Object { $_ -and $_ -notmatch "^\s*$" }

    $nonZero = $output | Where-Object {
        $_ -match "^\s+[0-9A-F]{4}:" -and $_ -notmatch "00 00 00 00 00 00 00 00\s+00 00 00 00 00 00 00 00"
    }

    if ($nonZero) {
        $nonZero | ForEach-Object {
            Write-Host $_
            $_ | Add-Content $outFile
        }
    } else {
        $msg = "  (all zeros)"
        Write-Host $msg -ForegroundColor DarkGray
        $msg | Add-Content $outFile
    }

    "" | Add-Content $outFile
    Write-Host ""
}

Write-Host "Saved to $outFile" -ForegroundColor Green