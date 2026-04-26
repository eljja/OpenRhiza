$ErrorActionPreference = "Stop"

$host.UI.RawUI.WindowTitle = "OpenRhiza Serial Log"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class OpenRhizaConsoleFont {
    [StructLayout(LayoutKind.Sequential)]
    public struct COORD {
        public short X;
        public short Y;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct CONSOLE_FONT_INFO_EX {
        public uint cbSize;
        public uint nFont;
        public COORD dwFontSize;
        public int FontFamily;
        public int FontWeight;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string FaceName;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern IntPtr GetStdHandle(int nStdHandle);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetCurrentConsoleFontEx(
        IntPtr consoleOutput,
        bool maximumWindow,
        ref CONSOLE_FONT_INFO_EX consoleCurrentFontEx
    );
}
"@

function Set-OpenRhizaConsoleFont {
    param(
        [int]$Width = 4,
        [int]$Height = 8,
        [string]$FaceName = "Consolas"
    )

    try {
        $handle = [OpenRhizaConsoleFont]::GetStdHandle(-11)
        if ($handle -eq [IntPtr]::Zero -or $handle -eq [IntPtr]::new(-1)) {
            return
        }

        $font = New-Object OpenRhizaConsoleFont+CONSOLE_FONT_INFO_EX
        $font.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf([OpenRhizaConsoleFont+CONSOLE_FONT_INFO_EX])
        $font.nFont = 0
        $font.dwFontSize = New-Object OpenRhizaConsoleFont+COORD
        $font.dwFontSize.X = [int16]$Width
        $font.dwFontSize.Y = [int16]$Height
        $font.FontFamily = 54
        $font.FontWeight = 400
        $font.FaceName = $FaceName
        [void][OpenRhizaConsoleFont]::SetCurrentConsoleFontEx($handle, $false, [ref]$font)
    } catch {
    }
}

function Write-OpenRhizaLogLine {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Line
    )

    if ([string]::IsNullOrEmpty($Line)) {
        Write-Host ""
        return
    }

    if ($Line.StartsWith("[OpenRhiza Serial]")) {
        $color = if ($Line -like "*Connected*") {
            "Green"
        } elseif ($Line -like "*Disconnected*") {
            "Yellow"
        } elseif ($Line -like "*Waiting*") {
            "DarkGray"
        } else {
            "Cyan"
        }
        Write-Host $Line -ForegroundColor $color
        return
    }

    if ($Line.StartsWith("QEMU_LOG:") -or $Line.StartsWith("[TLS]") -or $Line.StartsWith("[HTTPS") -or $Line.StartsWith("[HTTP]")) {
        Write-Host $Line -ForegroundColor DarkGray
        return
    }

    Write-Host $Line -ForegroundColor Yellow
}

Set-OpenRhizaConsoleFont

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 4444)
$listener.Server.SetSocketOption([System.Net.Sockets.SocketOptionLevel]::Socket, [System.Net.Sockets.SocketOptionName]::ReuseAddress, $true)
$listener.Start()

Write-OpenRhizaLogLine "[OpenRhiza Serial] Listening on 127.0.0.1:4444"

try {
    while ($true) {
        Write-OpenRhizaLogLine "[OpenRhiza Serial] Waiting for QEMU..."
        $client = $listener.AcceptTcpClient()
        Write-OpenRhizaLogLine "[OpenRhiza Serial] Connected."

        try {
            $stream = $client.GetStream()
            $buffer = New-Object byte[] 4096
            $encoding = [System.Text.Encoding]::ASCII
            $pending = ""

            while ($client.Connected) {
                if (-not $stream.DataAvailable) {
                    Start-Sleep -Milliseconds 50
                    continue
                }

                $read = $stream.Read($buffer, 0, $buffer.Length)
                if ($read -le 0) {
                    break
                }

                $pending += $encoding.GetString($buffer, 0, $read)
                $pending = $pending -replace "`r", ""

                while ($true) {
                    $newlineIndex = $pending.IndexOf("`n")
                    if ($newlineIndex -lt 0) {
                        break
                    }

                    $line = $pending.Substring(0, $newlineIndex)
                    Write-OpenRhizaLogLine $line
                    if ($newlineIndex + 1 -ge $pending.Length) {
                        $pending = ""
                    } else {
                        $pending = $pending.Substring($newlineIndex + 1)
                    }
                }
            }

            if (-not [string]::IsNullOrEmpty($pending)) {
                Write-OpenRhizaLogLine $pending
            }
        } finally {
            $client.Dispose()
            Write-OpenRhizaLogLine ""
            Write-OpenRhizaLogLine "[OpenRhiza Serial] Disconnected."
        }
    }
} finally {
    $listener.Stop()
}
