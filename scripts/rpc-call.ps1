param([string]$method='getLatestBlockhash', [string]$params='null')
$body = '{"jsonrpc":"2.0","id":1,"method":"' + $method + '","params":' + $params + '}'
$r = Invoke-RestMethod -Uri 'https://api.devnet.solana.com' -Method Post -Body $body -ContentType 'application/json'
$r | ConvertTo-Json -Compress
