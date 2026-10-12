# Fix and start cloudflared Windows service (locally-managed tunnel tabletop-web)
# Run as ADMINISTRATOR:
#   powershell -ExecutionPolicy Bypass -File "D:\code\tabletop_game\deploy\cloudflared\install-service.ps1"

$svcDir = "C:\ProgramData\cloudflared"
$credName = "562666ab-7166-4b20-aef8-7a3e757ae226.json"
$credSrc = "C:\Users\jiayan\.cloudflared\$credName"

# 1. copy tunnel credentials
New-Item -ItemType Directory -Force -Path $svcDir | Out-Null
Copy-Item $credSrc $svcDir -Force
Write-Output "[1/4] credentials copied to $svcDir"

# 2. write service config.yml
$config = @(
  "tunnel: tabletop-web"
  "credentials-file: ${svcDir}\${credName}"
  ""
  "ingress:"
  "  - hostname: tabletopgame.20221219.xyz"
  "    service: http://127.0.0.1:18080"
  "  - service: http_status:404"
)
$config | Set-Content -Path "${svcDir}\config.yml" -Encoding ASCII
Write-Output "[2/4] config.yml written"

# 3. stop the (broken) service and wait until it is actually stopped
sc.exe stop cloudflared 2>$null
for ($i = 0; $i -lt 10; $i++) {
  $state = (sc.exe query cloudflared 2>$null | Select-String 'STATE').Line
  if ($state -match 'STOPPED') { break }
  Start-Sleep -Seconds 1
}
Write-Output "[3/5] service stopped"

# 4. fix the service command line via registry ImagePath
$imagePath = '"C:\Program Files (x86)\cloudflared\cloudflared.exe" tunnel --config C:\ProgramData\cloudflared\config.yml run'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\cloudflared' -Name ImagePath -Value $imagePath
Write-Output "[4/5] ImagePath fixed"

# 5. start
sc.exe start cloudflared
Start-Sleep -Seconds 3
Write-Output "[5/5] started"
Write-Output ""
Write-Output "=== service status ==="
sc.exe query cloudflared | Select-String "STATE"
