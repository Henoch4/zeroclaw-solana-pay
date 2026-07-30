cd $PSScriptRoot\..
$msg = 'Use the network tool to make an HTTP POST to https://api.devnet.solana.com with Content-Type application/json and body: {"jsonrpc":"2.0","id":1,"method":"getLatestBlockhash"}'
.\bin\zeroclaw.exe --config-dir agent agent -a solana-payments -m $msg 2>&1
