# PE import inspection for portable x64 OCR packaging. PowerShell 5.1+.
function Get-PeImports {
    param([Parameter(Mandatory = $true)][string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 77 -or $bytes[1] -ne 90) { throw "Not a PE file: $Path" }
    $pe = [BitConverter]::ToInt32($bytes, 60)
    if ($pe -lt 0 -or $pe + 24 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550 -or [BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664) {
        throw "Expected an x64 PE file: $Path"
    }
    $count = [BitConverter]::ToUInt16($bytes, $pe + 6)
    $optionalSize = [BitConverter]::ToUInt16($bytes, $pe + 20)
    $optional = $pe + 24
    if ($optional + $optionalSize -gt $bytes.Length -or $optionalSize -lt 224 -or [BitConverter]::ToUInt16($bytes, $optional) -ne 0x20b) {
        throw "Invalid PE32+ header: $Path"
    }
    $headersSize = [BitConverter]::ToUInt32($bytes, $optional + 60)
    $sections = @()
    for ($i = 0; $i -lt $count; $i++) {
        $section = $optional + $optionalSize + $i * 40
        if ($section + 40 -gt $bytes.Length) { throw "Invalid PE section: $Path" }
        $sections += [pscustomobject]@{
            Rva = [BitConverter]::ToUInt32($bytes, $section + 12)
            Size = [Math]::Max([BitConverter]::ToUInt32($bytes, $section + 8), [BitConverter]::ToUInt32($bytes, $section + 16))
            Offset = [BitConverter]::ToUInt32($bytes, $section + 20)
        }
    }
    function Convert-Rva {
        param([long]$Rva)
        if ($Rva -lt $headersSize -and $Rva -lt $bytes.Length) { return [int]$Rva }
        foreach ($s in $sections) {
            if ($Rva -ge $s.Rva -and $Rva -lt ([long]$s.Rva + $s.Size)) {
                $offset = [long]$s.Offset + $Rva - $s.Rva
                if ($offset -ge 0 -and $offset -lt $bytes.Length) { return [int]$offset }
            }
        }
        throw "Invalid PE RVA in $Path"
    }
    # Import directory (1) and delay-load directory (13); both descriptors use name RVAs.
    foreach ($directory in @(1, 13)) {
        $rva = [BitConverter]::ToUInt32($bytes, $optional + 112 + 8 * $directory)
        $size = [BitConverter]::ToUInt32($bytes, $optional + 116 + 8 * $directory)
        if ($rva -eq 0 -or $size -eq 0) { continue }
        $cursor = Convert-Rva $rva
        $stride = if ($directory -eq 1) { 20 } else { 32 }
        $limit = [Math]::Min([long]$bytes.Length, [long]$cursor + $size)
        while ($cursor + $stride -le $limit) {
            $nameOffset = if ($directory -eq 1) { 12 } else { 4 }
            $nameRva = [BitConverter]::ToUInt32($bytes, $cursor + $nameOffset)
            if ($nameRva -eq 0) { break }
            if ($directory -eq 13 -and ([BitConverter]::ToUInt32($bytes, $cursor) -band 1) -eq 0) {
                throw "Unsupported non-RVA delay imports in $Path"
            }
            $start = Convert-Rva $nameRva
            $end = $start
            while ($end -lt $bytes.Length -and $bytes[$end] -ne 0 -and $end - $start -le 260) { $end++ }
            if ($end -ge $bytes.Length -or $end - $start -gt 260) { throw "Invalid PE import name: $Path" }
            $name = [Text.Encoding]::ASCII.GetString($bytes, $start, $end - $start)
            if ($name -notmatch '^[A-Za-z0-9_+.-]+\.(dll|drv)$') { throw "Unexpected DLL import: $name" }
            $name
            $cursor += $stride
        }
    }
}

function Get-OcrRuntime {
    param([Parameter(Mandatory = $true)][string]$Directory)
    $root = (Resolve-Path -LiteralPath $Directory).Path
    $system = @('advapi32.dll','bcrypt.dll','bcryptprimitives.dll','cabinet.dll','comctl32.dll','comdlg32.dll','crypt32.dll','dbghelp.dll','dnsapi.dll','gdi32.dll','imm32.dll','iphlpapi.dll','kernel32.dll','msvcrt.dll','ncrypt.dll','netapi32.dll','normaliz.dll','ntdll.dll','ole32.dll','oleaut32.dll','powrprof.dll','psapi.dll','rpcrt4.dll','secur32.dll','setupapi.dll','shell32.dll','shlwapi.dll','ucrtbase.dll','user32.dll','userenv.dll','usp10.dll','version.dll','winhttp.dll','wininet.dll','winmm.dll','winspool.drv','wldap32.dll','ws2_32.dll','wtsapi32.dll')
    $queue = New-Object 'System.Collections.Generic.Queue[string]'
    $queue.Enqueue('tesseract.exe')
    $seen = @{}
    $files = @()
    $external = @{}
    while ($queue.Count -gt 0) {
        $name = $queue.Dequeue()
        if ($seen.ContainsKey($name)) { continue }
        $seen[$name] = $true
        $path = Join-Path $root $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "OCR runtime dependency missing: $path" }
        $files += (Get-Item -LiteralPath $path).FullName
        foreach ($import in @(Get-PeImports -Path $path)) {
            $dependency = Join-Path $root $import
            if (Test-Path -LiteralPath $dependency -PathType Leaf) { $queue.Enqueue($import) }
            elseif ($system -contains $import -or $import -match '^(api-ms-win-|ext-ms-win-)') { $external[$import] = $true }
            else { throw "Missing non-system DLL '$import' required by '$name'. Supply a complete x64 Tesseract runtime directory." }
        }
    }
    [pscustomobject]@{ Files = @($files | Sort-Object); SystemLibraries = @($external.Keys | Sort-Object) }
}
