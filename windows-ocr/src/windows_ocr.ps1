# Optional Windows 10/11 on-device Chinese OCR. No language installation or networking.
param([string]$ImagePath='', [string]$AlternativeImagePath='', [switch]$CheckOnly)
$ErrorActionPreference='Stop'
try {
    Add-Type -AssemblyName System.Runtime.WindowsRuntime
    $null=[Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime]
    $null=[Windows.Globalization.Language,Windows.Globalization,ContentType=WindowsRuntime]
    $null=[Windows.Storage.StorageFile,Windows.Storage,ContentType=WindowsRuntime]
    $null=[Windows.Storage.FileAccessMode,Windows.Storage,ContentType=WindowsRuntime]
    $null=[Windows.Storage.Streams.IRandomAccessStream,Windows.Storage.Streams,ContentType=WindowsRuntime]
    $null=[Windows.Graphics.Imaging.BitmapDecoder,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
    $null=[Windows.Graphics.Imaging.SoftwareBitmap,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
    $null=[Windows.Media.Ocr.OcrResult,Windows.Foundation,ContentType=WindowsRuntime]
    $asTask=[System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
        $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
    } | Select-Object -First 1
    function Await-Operation($operation,[Type]$type) {
        $task=$asTask.MakeGenericMethod($type).Invoke($null,@($operation))
        $task.GetAwaiter().GetResult()
    }
    $language=[Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages | Where-Object { $_.LanguageTag -match '^zh-(Hans|CN|SG)' } | Select-Object -First 1
    if (-not $language) { throw 'WINDOWS_CHINESE_OCR_UNAVAILABLE' }
    $engine=[Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($language)
    if (-not $engine) { throw 'WINDOWS_CHINESE_OCR_UNAVAILABLE' }
    if ($CheckOnly) { [Console]::WriteLine('WINDOWS_OCR_READY'); exit 0 }
    foreach ($path in @($ImagePath,$AlternativeImagePath) | Where-Object { $_ }) {
    $file=Await-Operation ([Windows.Storage.StorageFile]::GetFileFromPathAsync($path)) ([Windows.Storage.StorageFile])
    $stream=Await-Operation ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
    try {
        $decoder=Await-Operation ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
        $bitmap=Await-Operation ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
        try {
            if ($bitmap.PixelWidth -gt [Windows.Media.Ocr.OcrEngine]::MaxImageDimension -or $bitmap.PixelHeight -gt [Windows.Media.Ocr.OcrEngine]::MaxImageDimension) { throw 'WINDOWS_OCR_IMAGE_TOO_LARGE' }
            $result=Await-Operation ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
            [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
            [Console]::WriteLine($result.Text)
        } finally { if ($bitmap -is [IDisposable]) { $bitmap.Dispose() } }
    } finally { if ($stream -is [IDisposable]) { $stream.Dispose() } }
    }
    exit 0
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
