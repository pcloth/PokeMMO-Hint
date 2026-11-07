# PowerShell 5.1; fixed upstreams only. Never execute downloaded scripts.
param([Parameter(Mandatory=$true)][string]$CacheDir,[ValidateSet('mirror','official')][string]$Source='mirror',[string]$ProgressPath)
$ErrorActionPreference='Stop'
[Net.ServicePointManager]::SecurityProtocol=[Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
Add-Type -AssemblyName System.Net.Http
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Progress-Text([string]$Text) { if($ProgressPath){[IO.File]::WriteAllText($ProgressPath,$Text,[Text.UTF8Encoding]::new($false))} }
function Download-File([string[]]$Urls,[string]$Destination,[string]$Label) {
    $lastError='Download failed'
    foreach($url in $Urls) {
        $client=[Net.Http.HttpClient]::new();$client.Timeout=[TimeSpan]::FromMinutes(8)
        $response=$null;$inputStream=$null;$outputStream=$null
        try {
            Progress-Text "$Label connecting"
            $response=$client.GetAsync($url,[Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
            $response.EnsureSuccessStatusCode() | Out-Null
            $total=$response.Content.Headers.ContentLength
            $inputStream=$response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
            $outputStream=[IO.File]::Create($Destination)
            $buffer=New-Object byte[] 65536;$received=0L;$lastTick=[DateTime]::UtcNow
            while(($count=$inputStream.Read($buffer,0,$buffer.Length)) -gt 0){
                $outputStream.Write($buffer,0,$count);$received+=$count
                if(([DateTime]::UtcNow-$lastTick).TotalMilliseconds -ge 200){
                    $size=('{0:N1} MB' -f ($received/1MB))
                    if($total){$size+=(' / {0:N1} MB' -f ($total/1MB))}
                    Progress-Text "$Label $size";$lastTick=[DateTime]::UtcNow
                }
            }
            return
        } catch { $lastError=$_.Exception.Message }
        finally {if($outputStream){$outputStream.Dispose()};if($inputStream){$inputStream.Dispose()};if($response){$response.Dispose()};$client.Dispose()}
    }
    throw $lastError
}
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
$hash='9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6'
$model=Join-Path $CacheDir 'inference.onnx'
$workRoot=$CacheDir
if($ProgressPath){$workRoot=Split-Path -Parent $ProgressPath}
$work=Join-Path $workRoot ('.download-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    if(-not (Test-Path -LiteralPath $model) -or (Get-FileHash -LiteralPath $model -Algorithm SHA256).Hash -ne $hash){
        Remove-Item -LiteralPath (Join-Path $CacheDir 'ready.txt') -ErrorAction SilentlyContinue
        $relative='PaddlePaddle/PP-OCRv6_tiny_rec_onnx/resolve/main/inference.onnx'
        $urls=@("https://hf-mirror.com/$relative","https://huggingface.co/$relative")
        if($Source -eq 'official'){$urls=@($urls[1],$urls[0])}
        $temp=Join-Path $work 'model.part'
        Download-File $urls $temp 'MODEL'
        if((Get-FileHash -LiteralPath $temp -Algorithm SHA256).Hash -ne $hash){throw 'MODEL_SHA256_MISMATCH'}
        Move-Item -LiteralPath $temp -Destination $model -Force
    } else { Progress-Text 'CACHE model verified' }
    $runtime=Join-Path $CacheDir 'onnxruntime.dll'
    $manifest=Join-Path $CacheDir 'runtime-files.json'
    $valid=Test-Path -LiteralPath $manifest
    if($valid){
        try {foreach($record in @(Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json)){
            if($record.name -notmatch '^[a-zA-Z0-9_.-]+\.dll$'){throw 'Invalid runtime manifest'}
            $file=Join-Path $CacheDir $record.name
            if(-not(Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $record.sha256){$valid=$false}
        }}catch{$valid=$false}
    }
    if(-not $valid -or -not(Test-Path -LiteralPath $runtime)){
        Remove-Item -LiteralPath (Join-Path $CacheDir 'ready.txt') -ErrorAction SilentlyContinue
        $package=Join-Path $work 'runtime.zip'
        Download-File @('https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/1.22.0/microsoft.ml.onnxruntime.directml.1.22.0.nupkg','https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.22.0') $package 'RUNTIME'
        Progress-Text 'RUNTIME extracting'
        $archive=[IO.Compression.ZipFile]::OpenRead($package)
        try {
            $records=@()
            foreach($entry in $archive.Entries){
                if($entry.FullName -match '^runtimes/win-x64/native/([a-zA-Z0-9_.-]+\.dll)$'){
                    $name=$Matches[1];$target=Join-Path $CacheDir $name
                    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$target,$true)
                    $records+=@{name=$name;sha256=(Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash}
                }elseif($entry.FullName -match '(^|/)(LICENSE|ThirdPartyNotices)(\.txt)?$'){
                    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,(Join-Path $CacheDir $entry.Name),$true)
                }
            }
            if(-not(Test-Path -LiteralPath $runtime)){throw 'Native x64 runtime missing in package'}
            [IO.File]::WriteAllText($manifest,(ConvertTo-Json -InputObject @($records)),[Text.UTF8Encoding]::new($false))
        }finally{$archive.Dispose()}
    } else { Progress-Text 'CACHE runtime verified' }
    Progress-Text 'CHECK loading model'
    [Console]::WriteLine('DOWNLOAD_READY')
} catch { [Console]::Error.WriteLine($_.Exception.Message);exit 1 }
finally {if(Test-Path -LiteralPath $work){Remove-Item -LiteralPath $work -Recurse -Force}}
