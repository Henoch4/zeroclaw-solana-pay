pub mod core;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use std::collections::HashMap;

    use crate::core::{
        amount_to_units, build_advance_nonce_and_transfer, build_create_and_init_nonce,
        build_transfer_url, build_transfer_url_with_reference, build_unsigned_advance_nonce,
        build_unsigned_sol_transfer, derive_associated_token_account, normalize_address,
        units_to_amount, validate_address, verify_payment_from_rpc, verify_transfer_amount,
        PluginConfig, SolanaPayUrl,
    };
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };

    struct SolanaWallet;

    const PLUGIN_NAME: &str = "solana-wallet";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
    const TOOL_NAME: &str = "solana_wallet";

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct ExecuteArgs {
        action: String,
        recipient: Option<String>,
        sender: Option<String>,
        authority: Option<String>,
        amount: Option<String>,
        amount_units: Option<u64>,
        spl_token: Option<String>,
        mint: Option<String>,
        reference: Option<String>,
        label: Option<String>,
        message: Option<String>,
        memo: Option<String>,
        blockhash: Option<String>,
        rpc_response: Option<String>,
        min_confirmations: Option<u64>,
        wallet: Option<String>,
        nonce_address: Option<String>,
        payer: Option<String>,
        lamports: Option<u64>,
        decimals: Option<u8>,
        from: Option<String>,
        to: Option<String>,
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    impl PluginInfo for SolanaWallet {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }
        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for SolanaWallet {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Solana wallet toolkit: validate addresses, convert amounts, build Solana Pay \
             transfer URLs, verify payments by reference key, build unsigned SOL transfer \
             transactions, derive Associated Token Accounts, and manage durable nonces. \
             The agent provides RPC transport via http_request; this plugin handles all \
             Solana-specific construction and parsing."
                .to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Action to perform",
                        "enum": [
                            "validate_address",
                            "normalize_address",
                            "amount_to_units",
                            "units_to_amount",
                            "build_transfer_url",
                            "verify_payment",
                            "verify_transfer_amount",
                            "build_unsigned_sol_transfer",
                            "build_unsigned_advance_nonce",
                            "build_advance_nonce_and_transfer",
                            "build_create_and_init_nonce",
                            "derive_ata",
                            "get_config"
                        ]
                    },
                    "recipient": { "type": "string", "description": "Recipient address" },
                    "sender": { "type": "string", "description": "Sender address" },
                    "amount": { "type": "string", "description": "Human-readable amount (e.g. '1.5')" },
                    "amount_units": { "type": "integer", "description": "Amount in base units (lamports/token units)" },
                    "spl_token": { "type": "string", "description": "SPL token mint address" },
                    "mint": { "type": "string", "description": "Token mint address" },
                    "reference": { "type": "string", "description": "Reference key for payment tracking" },
                    "label": { "type": "string", "description": "Label for Solana Pay URL" },
                    "message": { "type": "string", "description": "Message for Solana Pay URL" },
                    "memo": { "type": "string", "description": "Optional memo" },
                    "blockhash": { "type": "string", "description": "Recent blockhash (base58)" },
                    "rpc_response": { "type": "string", "description": "Raw JSON RPC response from getSignaturesForAddress" },
                    "min_confirmations": { "type": "integer", "description": "Minimum confirmations required (default: 32)" },
                    "decimals": { "type": "integer", "description": "Token decimals" },
                    "wallet": { "type": "string", "description": "Wallet address for ATA derivation" },
                    "from": { "type": "string", "description": "Source address for transfer" },
                    "to": { "type": "string", "description": "Destination address for transfer" }
                },
                "required": ["action"]
            })
            .to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = match serde_json::from_str(&args) {
                Ok(a) => a,
                Err(e) => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, "invalid arguments", None);
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Invalid arguments: {e}")),
                    });
                }
            };

            let cfg = PluginConfig::from_section(&parsed.config);
            let result = dispatch_action(&parsed, &cfg);

            match &result {
                Ok(output) => {
                    emit(PluginAction::Complete, PluginOutcome::Success, "action completed", None);
                    Ok(ToolResult {
                        success: true,
                        output: output.clone(),
                        error: None,
                    })
                }
                Err(e) => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, e, None);
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(e.clone()),
                    })
                }
            }
        }
    }

    fn dispatch_action(args: &ExecuteArgs, cfg: &PluginConfig) -> Result<String, String> {
        match args.action.as_str() {
            "validate_address" => {
                let addr = args.recipient.as_ref()
                    .or(args.sender.as_ref())
                    .or(args.wallet.as_ref())
                    .ok_or_else(|| "Missing address (recipient/sender/wallet)".to_string())?;
                validate_address(addr)?;
                Ok(serde_json::json!({"valid": true, "address": addr}).to_string())
            }
            "normalize_address" => {
                let addr = args.recipient.as_ref()
                    .or(args.sender.as_ref())
                    .or(args.wallet.as_ref())
                    .ok_or_else(|| "Missing address".to_string())?;
                let normalized = normalize_address(addr)?;
                Ok(serde_json::json!({"normalized": normalized}).to_string())
            }
            "amount_to_units" => {
                let amount = args.amount.as_ref()
                    .ok_or_else(|| "Missing amount".to_string())?;
                let decimals = args.decimals.unwrap_or(cfg.default_token_decimals);
                let units = amount_to_units(amount, decimals)?;
                Ok(serde_json::json!({"units": units, "decimals": decimals}).to_string())
            }
            "units_to_amount" => {
                let units = args.amount_units
                    .ok_or_else(|| "Missing amount_units".to_string())?;
                let decimals = args.decimals.unwrap_or(cfg.default_token_decimals);
                let amount = units_to_amount(units, decimals);
                Ok(serde_json::json!({"amount": amount, "decimals": decimals}).to_string())
            }
            "build_transfer_url" => {
                let recipient = args.recipient.as_ref()
                    .ok_or_else(|| "Missing recipient".to_string())?;
                let amount_units = args.amount_units
                    .ok_or_else(|| "Missing amount_units".to_string())?;
                let label = args.label.as_deref().unwrap_or(&cfg.shop_label);
                let message = args.message.as_deref().unwrap_or(&cfg.shop_message);

                let url = if let Some(ref_key) = &args.reference {
                    build_transfer_url_with_reference(
                        recipient,
                        amount_units,
                        args.spl_token.as_deref(),
                        ref_key,
                        label,
                        message,
                        args.memo.as_deref(),
                    )?
                } else {
                    let params = SolanaPayUrl {
                        recipient: recipient.to_string(),
                        amount: amount_units,
                        spl_token: args.spl_token.clone(),
                        reference: args.reference.clone(),
                        label: label.to_string(),
                        message: message.to_string(),
                        memo: args.memo.clone(),
                    };
                    build_transfer_url(&params)?
                };
                Ok(serde_json::json!({"url": url, "protocol": "solana"}).to_string())
            }
            "verify_payment" => {
                let reference = args.reference.as_ref()
                    .ok_or_else(|| "Missing reference".to_string())?;
                let rpc_response = args.rpc_response.as_ref()
                    .ok_or_else(|| "Missing rpc_response".to_string())?;
                let min_conf = args.min_confirmations.unwrap_or(32);
                let verification = verify_payment_from_rpc(reference, rpc_response, min_conf)?;
                Ok(serde_json::to_string(&serde_json::json!({
                    "confirmed": verification.confirmed,
                    "signature_count": verification.signature_count,
                    "signatures": verification.signatures,
                    "slot": verification.slot,
                })).map_err(|e| e.to_string())?)
            }
            "verify_transfer_amount" => {
                let transaction = args.rpc_response.as_ref()
                    .ok_or_else(|| "Missing transaction JSON (use rpc_response field)".to_string())?;
                let verification = verify_transfer_amount(
                    transaction,
                    args.amount_units,
                    args.to.as_ref().or(args.recipient.as_ref()).map(|s| s.as_str()),
                    args.amount_units,
                    args.spl_token.as_ref().or(args.mint.as_ref()).map(|s| s.as_str()),
                )?;
                Ok(serde_json::to_string(&serde_json::json!({
                    "amount_correct": verification.amount_correct,
                    "recipient_correct": verification.recipient_correct,
                    "mint_correct": verification.mint_correct,
                    "actual_lamports": verification.actual_lamports,
                    "actual_token_amount": verification.actual_token_amount,
                    "actual_recipient": verification.actual_recipient,
                    "actual_mint": verification.actual_mint,
                })).map_err(|e| e.to_string())?)
            }
            "build_unsigned_sol_transfer" => {
                let from = args.from.as_ref()
                    .or(args.sender.as_ref())
                    .ok_or_else(|| "Missing from/sender".to_string())?;
                let to = args.to.as_ref()
                    .or(args.recipient.as_ref())
                    .ok_or_else(|| "Missing to/recipient".to_string())?;
                let lamports = args.lamports.or(args.amount_units)
                    .ok_or_else(|| "Missing lamports/amount_units".to_string())?;
                let blockhash = args.blockhash.as_ref()
                    .ok_or_else(|| "Missing blockhash".to_string())?;
                let b64 = build_unsigned_sol_transfer(from, to, lamports, blockhash)?;
                Ok(serde_json::json!({
                    "unsigned_tx_base64": b64,
                    "kind": "sol_transfer",
                    "from": from,
                    "to": to,
                }).to_string())
            }
            "build_unsigned_advance_nonce" => {
                let nonce = args.nonce_address.as_ref()
                    .ok_or_else(|| "Missing nonce_address".to_string())?;
                let auth = args.authority.as_ref()
                    .ok_or_else(|| "Missing authority".to_string())?;
                let blockhash = args.blockhash.as_ref()
                    .ok_or_else(|| "Missing blockhash".to_string())?;
                let b64 = build_unsigned_advance_nonce(nonce, auth, blockhash)?;
                Ok(serde_json::json!({
                    "unsigned_tx_base64": b64,
                    "kind": "advance_nonce",
                }).to_string())
            }
            "build_advance_nonce_and_transfer" => {
                let nonce = args.nonce_address.as_ref()
                    .ok_or_else(|| "Missing nonce_address".to_string())?;
                let auth = args.authority.as_ref()
                    .ok_or_else(|| "Missing authority".to_string())?;
                let from = args.from.as_ref()
                    .ok_or_else(|| "Missing from".to_string())?;
                let to = args.to.as_ref()
                    .or(args.recipient.as_ref())
                    .ok_or_else(|| "Missing to/recipient".to_string())?;
                let lamports = args.lamports.or(args.amount_units)
                    .ok_or_else(|| "Missing lamports/amount_units".to_string())?;
                let blockhash = args.blockhash.as_ref()
                    .ok_or_else(|| "Missing blockhash".to_string())?;
                let b64 = build_advance_nonce_and_transfer(nonce, auth, from, to, lamports, blockhash)?;
                Ok(serde_json::json!({
                    "unsigned_tx_base64": b64,
                    "kind": "advance_nonce_and_transfer",
                    "nonce_address": nonce,
                    "from": from,
                    "to": to,
                    "amount_lamports": lamports,
                }).to_string())
            }
            "build_create_and_init_nonce" => {
                let nonce = args.nonce_address.as_ref()
                    .ok_or_else(|| "Missing nonce_address".to_string())?;
                let auth = args.authority.as_ref()
                    .ok_or_else(|| "Missing authority".to_string())?;
                let payer = args.payer.as_ref()
                    .or(args.from.as_ref())
                    .ok_or_else(|| "Missing payer/from".to_string())?;
                let lamports = args.lamports.unwrap_or(crate::core::RENT_EXEMPT_NONCE_LAMPORTS);
                let blockhash = args.blockhash.as_ref()
                    .ok_or_else(|| "Missing blockhash".to_string())?;
                let b64 = build_create_and_init_nonce(nonce, auth, payer, lamports, blockhash)?;
                Ok(serde_json::json!({
                    "unsigned_tx_base64": b64,
                    "kind": "create_nonce",
                }).to_string())
            }
            "derive_ata" => {
                let wallet = args.wallet.as_ref()
                    .ok_or_else(|| "Missing wallet".to_string())?;
                let mint = args.mint.as_ref()
                    .or(args.spl_token.as_ref())
                    .ok_or_else(|| "Missing mint/spl_token".to_string())?;
                let ata = derive_associated_token_account(wallet, mint)?;
                Ok(serde_json::json!({"ata": ata}).to_string())
            }
            "get_config" => {
                Ok(serde_json::json!({
                    "default_rpc_url": cfg.default_rpc_url,
                    "default_spl_token": cfg.default_spl_token,
                    "default_token_decimals": cfg.default_token_decimals,
                    "shop_label": cfg.shop_label,
                    "shop_message": cfg.shop_message,
                }).to_string())
            }
            _ => Err(format!("Unknown action: {}", args.action)),
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str, _count: Option<usize>) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "solana_wallet::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(SolanaWallet);
}
