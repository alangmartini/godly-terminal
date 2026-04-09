Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$b = New-Object System.Drawing.Bitmap($s.Width, $s.Height)
$g = [System.Drawing.Graphics]::FromImage($b)
$g.CopyFromScreen($s.Location, [System.Drawing.Point]::Empty, $s.Size)
$b.Save('C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.png', [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$b.Dispose()
