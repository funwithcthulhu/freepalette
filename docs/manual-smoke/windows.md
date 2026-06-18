# Windows Manual Smoke

Run these commands from the repository root in PowerShell.

## Daemon IPC

```powershell
cargo run -p freepalette-daemon -- start
cargo run -p freepalette-cli -- daemon status
```

Expected result: the daemon starts, and status prints providers, clipboard item
count, recent action count, hotkey state, and local state path.

## Default Hotkey

```powershell
cargo run -p freepalette-daemon -- run
```

Expected result with the default config: the command prints that the global
hotkey is disabled and exits.

## Indexed App Launch

```powershell
cargo run -p freepalette-cli -- apps list
cargo run -p freepalette-cli -- run "notepad"
```

Expected result: `apps list` prints indexed app status, and the run command
launches Notepad or the configured Notepad entry.

## Clipboard Action

```powershell
function Invoke-FreePaletteIpc($Request) {
    $endpointPath = Join-Path $env:LOCALAPPDATA 'freepalette\freepalette\data\ipc.json'
    $endpoint = Get-Content -LiteralPath $endpointPath | ConvertFrom-Json
    $hostName, $port = $endpoint.address -split ':', 2
    $client = [Net.Sockets.TcpClient]::new($hostName, [int]$port)
    $stream = $client.GetStream()
    $writer = [IO.StreamWriter]::new($stream)
    $reader = [IO.StreamReader]::new($stream)
    $body = @{ token = $endpoint.token; request = $Request } | ConvertTo-Json -Depth 8 -Compress
    $writer.WriteLine($body)
    $writer.Flush()
    $reader.ReadLine()
    $client.Close()
}

Invoke-FreePaletteIpc @{ type = 'set-clipboard-capture'; enabled = $true }
Invoke-FreePaletteIpc @{ type = 'record-clipboard-text'; text = 'freepalette smoke clipboard' }
cargo run -p freepalette-cli -- daemon search "freepalette smoke clipboard"
```

Expected result: the record request returns a stored clipboard outcome, and the
daemon search returns a clipboard result without printing the stored text in
status output.

## Stop Daemon

```powershell
cargo run -p freepalette-cli -- daemon stop
```
