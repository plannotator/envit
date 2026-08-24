# envit installer (Windows). Usage: irm https://envit.dev/install.ps1 | iex
#
# Windows binaries are not published yet: envit's linking layer ships
# symlinks/junctions for Windows in a later milestone. Until then, use
# envit inside WSL:
#
#   wsl curl -fsSL https://envit.dev/install.sh | sh
#
Write-Host "envit: Windows binaries are not published yet."
Write-Host "Use WSL: wsl curl -fsSL https://envit.dev/install.sh | sh"
Write-Host "Track progress: https://github.com/plannotator/envit"
exit 1
