$ErrorActionPreference = "Stop"

$host.UI.RawUI.WindowTitle = "OpenRhiza Serial Log"

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 4444)
$listener.Server.SetSocketOption([System.Net.Sockets.SocketOptionLevel]::Socket, [System.Net.Sockets.SocketOptionName]::ReuseAddress, $true)
$listener.Start()

Write-Host "[OpenRhiza Serial] Listening on 127.0.0.1:4444" -ForegroundColor Cyan

try {
    while ($true) {
        Write-Host "[OpenRhiza Serial] Waiting for QEMU..." -ForegroundColor DarkGray
        $client = $listener.AcceptTcpClient()
        Write-Host "[OpenRhiza Serial] Connected." -ForegroundColor Green

        try {
            $stream = $client.GetStream()
            $buffer = New-Object byte[] 4096
            $encoding = [System.Text.Encoding]::ASCII

            while ($client.Connected) {
                if (-not $stream.DataAvailable) {
                    Start-Sleep -Milliseconds 50
                    continue
                }

                $read = $stream.Read($buffer, 0, $buffer.Length)
                if ($read -le 0) {
                    break
                }

                $text = $encoding.GetString($buffer, 0, $read)
                Write-Host -NoNewline $text
            }
        } finally {
            $client.Dispose()
            Write-Host ""
            Write-Host "[OpenRhiza Serial] Disconnected." -ForegroundColor Yellow
        }
    }
} finally {
    $listener.Stop()
}
