# Build script for solana-wallet plugin
# Run from the project root: .\scripts\build.ps1

$ErrorActionPreference = "Stop"

Write-Host "=== Solana Wallet Plugin Build ===" -ForegroundColor Cyan

# 1. Check wasm32-wasip2 target
$targets = rustup target list --installed
if ($targets -notcontains "wasm32-wasip2") {
    Write-Host "Adding wasm32-wasip2 target..." -ForegroundColor Yellow
    rustup target add wasm32-wasip2
}

# 2. Run host tests
Write-Host "`n=== Running host tests ===" -ForegroundColor Cyan
Push-Location plugins\solana-wallet
cargo test
if ($LASTEXITCODE -ne 0) {
    Write-Host "Tests failed!" -ForegroundColor Red
    exit 1
}
Write-Host "All tests passed!" -ForegroundColor Green

# 3. Build WASM component
Write-Host "`n=== Building WASM component ===" -ForegroundColor Cyan
cargo build --target wasm32-wasip2 --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "WASM build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "WASM build successful!" -ForegroundColor Green

# 4. Show output
$wasmPath = "target\wasm32-wasip2\release\solana_wallet.wasm"
if (Test-Path $wasmPath) {
    $size = (Get-Item $wasmPath).Length
    Write-Host "`n=== Output ===" -ForegroundColor Cyan
    Write-Host "Plugin: $wasmPath"
    Write-Host "Size: $($size / 1KB) KB"
}

Pop-Location
Write-Host "`n=== Build complete ===" -ForegroundColor Green
