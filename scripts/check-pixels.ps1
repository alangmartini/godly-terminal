param(
    [string]$Reference = (Join-Path $PSScriptRoot "..\docs\references\web-reference.png"),
    [string]$Actual = (Join-Path $PSScriptRoot "..\docs\references\current-godly-shell.png"),
    [string]$DiffOut,
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing
$drawingAssembly = [System.Drawing.Bitmap].Assembly.Location
Add-Type -ReferencedAssemblies $drawingAssembly -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;

public sealed class ImageDiffResult {
    public int Width;
    public int Height;
    public long TotalPixels;
    public long ChangedPixels;
    public long PixelsOver10;
    public int MaxChannelDiff;
    public double MeanAbsoluteError;
    public double RootMeanSquareError;
}

public static class ImageDiff {
    public static ImageDiffResult Compare(string referencePath, string actualPath, string diffPath) {
        using (Bitmap referenceRaw = new Bitmap(referencePath))
        using (Bitmap actualRaw = new Bitmap(actualPath))
        using (Bitmap reference = EnsureArgb(referenceRaw))
        using (Bitmap actual = EnsureArgb(actualRaw)) {
            if (reference.Width != actual.Width || reference.Height != actual.Height) {
                throw new InvalidOperationException(
                    string.Format(
                        "Image size mismatch: reference={0}x{1}, actual={2}x{3}",
                        reference.Width,
                        reference.Height,
                        actual.Width,
                        actual.Height
                    )
                );
            }

            Rectangle rect = new Rectangle(0, 0, reference.Width, reference.Height);
            BitmapData refData = reference.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            BitmapData actData = actual.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            Bitmap diff = new Bitmap(reference.Width, reference.Height, PixelFormat.Format32bppArgb);
            BitmapData diffData = diff.LockBits(rect, ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
            bool refLocked = true;
            bool actLocked = true;
            bool diffLocked = true;

            try {
                int bytes = Math.Abs(refData.Stride) * refData.Height;
                byte[] refBuffer = new byte[bytes];
                byte[] actBuffer = new byte[bytes];
                byte[] diffBuffer = new byte[bytes];
                Marshal.Copy(refData.Scan0, refBuffer, 0, bytes);
                Marshal.Copy(actData.Scan0, actBuffer, 0, bytes);

                long totalPixels = (long)reference.Width * reference.Height;
                long changedPixels = 0;
                long pixelsOver10 = 0;
                long absErrorSum = 0;
                double squaredErrorSum = 0.0;
                int maxChannelDiff = 0;

                for (int i = 0; i < bytes; i += 4) {
                    int db = Math.Abs(refBuffer[i + 0] - actBuffer[i + 0]);
                    int dg = Math.Abs(refBuffer[i + 1] - actBuffer[i + 1]);
                    int dr = Math.Abs(refBuffer[i + 2] - actBuffer[i + 2]);
                    int da = Math.Abs(refBuffer[i + 3] - actBuffer[i + 3]);
                    int maxRgb = Math.Max(dr, Math.Max(dg, db));

                    if (dr != 0 || dg != 0 || db != 0 || da != 0) {
                        changedPixels++;
                    }
                    if (maxRgb >= 10) {
                        pixelsOver10++;
                    }

                    absErrorSum += dr + dg + db;
                    squaredErrorSum += (dr * dr) + (dg * dg) + (db * db);
                    maxChannelDiff = Math.Max(maxChannelDiff, Math.Max(da, maxRgb));

                    diffBuffer[i + 0] = (byte)Math.Min(255, db * 4);
                    diffBuffer[i + 1] = (byte)Math.Min(255, dg * 4);
                    diffBuffer[i + 2] = (byte)Math.Min(255, dr * 4);
                    diffBuffer[i + 3] = 255;
                }

                Marshal.Copy(diffBuffer, 0, diffData.Scan0, bytes);
                reference.UnlockBits(refData);
                actual.UnlockBits(actData);
                diff.UnlockBits(diffData);
                refLocked = false;
                actLocked = false;
                diffLocked = false;

                string diffDir = Path.GetDirectoryName(diffPath);
                if (!string.IsNullOrEmpty(diffDir)) {
                    Directory.CreateDirectory(diffDir);
                }
                diff.Save(diffPath, ImageFormat.Png);
                diff.Dispose();

                return new ImageDiffResult {
                    Width = reference.Width,
                    Height = reference.Height,
                    TotalPixels = totalPixels,
                    ChangedPixels = changedPixels,
                    PixelsOver10 = pixelsOver10,
                    MaxChannelDiff = maxChannelDiff,
                    MeanAbsoluteError = absErrorSum / (double)(totalPixels * 3 * 255.0),
                    RootMeanSquareError = Math.Sqrt(squaredErrorSum / (double)(totalPixels * 3 * 255.0 * 255.0))
                };
            }
            catch {
                if (refLocked) reference.UnlockBits(refData);
                if (actLocked) actual.UnlockBits(actData);
                if (diffLocked) diff.UnlockBits(diffData);
                diff.Dispose();
                throw;
            }
        }
    }

    private static Bitmap EnsureArgb(Bitmap source) {
        Bitmap copy = new Bitmap(source.Width, source.Height, PixelFormat.Format32bppArgb);
        using (Graphics g = Graphics.FromImage(copy)) {
            g.DrawImage(source, 0, 0, source.Width, source.Height);
        }
        return copy;
    }
}
"@

function Resolve-ImagePath([string]$PathValue) {
    $resolved = Resolve-Path -LiteralPath $PathValue -ErrorAction Stop
    return $resolved.Path
}

$Reference = Resolve-ImagePath $Reference
$Actual = Resolve-ImagePath $Actual
if (-not $DiffOut) {
    $actualDir = Split-Path -Parent $Actual
    $actualBase = [System.IO.Path]::GetFileNameWithoutExtension($Actual)
    $DiffOut = Join-Path $actualDir "$actualBase.diff.png"
}

$result = [ImageDiff]::Compare($Reference, $Actual, $DiffOut)
$summary = [pscustomobject]@{
    reference = $Reference
    actual = $Actual
    diff = $DiffOut
    width = $result.Width
    height = $result.Height
    total_pixels = $result.TotalPixels
    changed_pixels = $result.ChangedPixels
    changed_pct = [math]::Round(($result.ChangedPixels / [double]$result.TotalPixels) * 100.0, 4)
    pixels_over_10 = $result.PixelsOver10
    pixels_over_10_pct = [math]::Round(($result.PixelsOver10 / [double]$result.TotalPixels) * 100.0, 4)
    max_channel_diff = $result.MaxChannelDiff
    mean_absolute_error = [math]::Round($result.MeanAbsoluteError, 6)
    root_mean_square_error = [math]::Round($result.RootMeanSquareError, 6)
}

if ($Json) {
    $summary | ConvertTo-Json -Depth 3
    return
}

Write-Host "Reference : $($summary.reference)"
Write-Host "Actual    : $($summary.actual)"
Write-Host "Diff image: $($summary.diff)"
Write-Host ("Size      : {0}x{1}" -f $summary.width, $summary.height)
Write-Host ("Changed   : {0} px ({1}%)" -f $summary.changed_pixels, $summary.changed_pct)
Write-Host ("Over-10   : {0} px ({1}%)" -f $summary.pixels_over_10, $summary.pixels_over_10_pct)
Write-Host ("Max diff  : {0}" -f $summary.max_channel_diff)
Write-Host ("MAE       : {0}" -f $summary.mean_absolute_error)
Write-Host ("RMSE      : {0}" -f $summary.root_mean_square_error)
