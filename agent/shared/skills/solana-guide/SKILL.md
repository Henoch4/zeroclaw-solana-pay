# Solana Integration Guide

This skill provides the agent with the knowledge to interact with Solana
using only the built-in `http_request` tool (Tier 1 - stock release).
No plugins or custom code required.

## Solana RPC Endpoints

### Devnet RPC (testing)
```
POST https://api.devnet.solana.com
```

On Windows, always use `curl.exe` (full path: C:\Windows\System32\curl.exe). Do NOT use `curl` — that's an alias for Invoke-WebRequest in PowerShell and will fail.

### Key Methods for Payment Terminal

**getLatestBlockhash** — Get recent blockhash:
```shell
curl.exe -X POST https://api.devnet.solana.com -H "Content-Type: application/json" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getLatestBlockhash\"}"
```
Result blockhash is in the `result.value.blockhash` field of the JSON response.

**getSignaturesForAddress** — Check payment status by reference key:
```shell
curl.exe -X POST https://api.devnet.solana.com -H "Content-Type: application/json" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getSignaturesForAddress\",\"params\":[\"REFERENCE_KEY\",{\"limit\":5}]}"
```

Response contains signatures with confirmation counts.
- `confirmations: null` means finalized (max confirmations)
- `confirmations: 0` means just processed
- Wait for at least 32 confirmations or null

**getBalance** — Check wallet balance:
```shell
curl.exe -X POST https://api.devnet.solana.com -H "Content-Type: application/json" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getBalance\",\"params\":[\"WALLET_ADDRESS\"]}"
```
Returns balance in lamports. Divide by 10^9 for SOL.

**getTokenAccountBalance** — Check USDC balance:
```shell
curl.exe -X POST https://api.devnet.solana.com -H "Content-Type: application/json" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTokenAccountBalance\",\"params\":[\"TOKEN_ACCOUNT\"]}"
```

## Solana Pay Protocol

A Solana Pay transfer-request URL has this format:
```
solana:<recipient>?amount=<units>&spl-token=<mint>&reference=<ref>&label=<label>&message=<msg>&memo=<memo>
```

### Fields
- `recipient`: base58 wallet address receiving funds
- `amount`: amount in smallest units (USDC: 6 decimals, SOL: 9 decimals)
- `spl-token`: mint address for SPL tokens (omit for SOL)
- `reference`: unique reference key for payment verification
- `label`: merchant name displayed in wallet
- `message`: invoice description
- `memo`: optional on-chain memo

### Reference Keys
Always generate a unique reference key per payment. Use this scheme:
1. Take the invoice ID + random salt
2. Hash with SHA-256
3. Encode as base58 (first 32 characters)

The reference key is NOT a private key - it's a public identifier used
to track which transactions are associated with this payment.

### Payment Flow
1. Generate Solana Pay URL with reference key
2. Customer scans/pastes in their wallet
3. Wallet shows preview, customer signs
4. Transaction lands on-chain
5. Poll `getSignaturesForAddress` with reference key
6. Check confirmations >= 32

## USDC on Solana

USDC mint address (devnet):
`4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`

USDC has 6 decimal places.
- 1 USDC = 1,000,000 units
- 0.01 USDC = 10,000 units
- 25 USDC = 25,000,000 units

## Relevant Token Addresses

- Shop USDC wallet (store in memory as `shop_wallet`)
- Shop USDC token account (derive from wallet + USDC mint)
- Customer addresses (store per session)

## Error Handling

- Blockhash expires after ~90 seconds. Use `getLatestBlockhash` (not the deprecated `getRecentBlockhash`). For pending payments, the Solana Pay URL is valid until the customer uses it - the wallet handles blockhash refresh.
- RPC rate limits: devnet public endpoint is rate-limited. Use a paid
  endpoint for mainnet production (Helius, Triton, QuickNode).
- getSignaturesForAddress can return empty array if no payment yet.
  Keep polling on cron every 10 seconds until confirmed or timeout.
