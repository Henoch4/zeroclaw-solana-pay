use std::collections::HashMap;
use base64::Engine;

pub const SOLANA_PAY_PROTOCOL: &str = "solana:";
pub const SOL_DECIMALS: u8 = 9;
pub const USDC_DECIMALS: u8 = 6;
pub const USDC_MINT_MAINNET: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xr25wh9So8vYqKcFZ";
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
pub const NONCE_ACCOUNT_LENGTH: u64 = 80;
pub const RENT_EXEMPT_NONCE_LAMPORTS: u64 = 1_500_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolAddress(pub String);

#[derive(Debug, Clone)]
pub struct SolanaPayUrl {
    pub recipient: String,
    pub amount: u64,
    pub spl_token: Option<String>,
    pub reference: Option<String>,
    pub label: String,
    pub message: String,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentVerification {
    pub confirmed: bool,
    pub signature_count: usize,
    pub signatures: Vec<String>,
    pub slot: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub default_rpc_url: String,
    pub default_spl_token: String,
    pub default_token_decimals: u8,
    pub shop_label: String,
    pub shop_message: String,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            default_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            default_spl_token: USDC_MINT_MAINNET.to_string(),
            default_token_decimals: USDC_DECIMALS,
            shop_label: "Shop Payment".to_string(),
            shop_message: "Payment for order".to_string(),
        }
    }
}

impl PluginConfig {
    pub fn from_section(section: &HashMap<String, String>) -> Self {
        Self {
            default_rpc_url: section
                .get("default_rpc_url")
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string()),
            default_spl_token: section
                .get("default_spl_token")
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| USDC_MINT_MAINNET.to_string()),
            default_token_decimals: section
                .get("default_token_decimals")
                .and_then(|v| v.parse().ok())
                .unwrap_or(USDC_DECIMALS),
            shop_label: section
                .get("shop_label")
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| "Shop Payment".to_string()),
            shop_message: section
                .get("shop_message")
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| "Payment for order".to_string()),
        }
    }
}

pub fn validate_address(addr: &str) -> Result<SolAddress, String> {
    if addr.len() < 32 || addr.len() > 44 {
        return Err("Invalid address length".to_string());
    }
    let decoded = bs58::decode(addr)
        .into_vec()
        .map_err(|e| format!("Invalid base58: {e}"))?;
    if decoded.len() != 32 {
        return Err("Decoded address must be 32 bytes".to_string());
    }
    Ok(SolAddress(addr.to_string()))
}

pub fn normalize_address(addr: &str) -> Result<String, String> {
    let trimmed = addr.trim();
    let decoded = bs58::decode(trimmed)
        .into_vec()
        .map_err(|e| format!("Invalid base58 address: {e}"))?;
    Ok(bs58::encode(decoded).into_string())
}

pub fn amount_to_units(amount_str: &str, decimals: u8) -> Result<u64, String> {
    let trimmed = amount_str.trim();
    if trimmed.is_empty() {
        return Err("Amount string is empty".to_string());
    }
    if trimmed.starts_with('-') {
        return Err("Amount must be non-negative".to_string());
    }
    let multiplier = 10u64
        .checked_pow(decimals as u32)
        .ok_or_else(|| format!("Decimals value {decimals} is too large (max supported: 19)"))?;

    let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
    let integer_str = parts[0];
    let integer: u64 = integer_str
        .parse()
        .map_err(|e| format!("Invalid integer part '{integer_str}': {e}"))?;

    let raw_frac = if parts.len() > 1 { parts[1] } else { "" };
    if raw_frac.len() > decimals as usize {
        return Err(format!(
            "Amount '{amount_str}' has {} decimal places but token only supports {decimals}",
            raw_frac.len()
        ));
    }

    let integer_units = integer
        .checked_mul(multiplier)
        .ok_or_else(|| format!("Amount '{amount_str}' exceeds maximum representable value"))?;

    let fractional_units = if !raw_frac.is_empty() {
        let padded = format!("{}{}", raw_frac, "0".repeat(decimals as usize - raw_frac.len()));
        padded
            .parse::<u64>()
            .map_err(|e| format!("Invalid fractional part: {e}"))?
    } else {
        0
    };

    integer_units
        .checked_add(fractional_units)
        .ok_or_else(|| format!("Amount '{amount_str} exceeds maximum representable value"))
}

pub fn units_to_amount(units: u64, decimals: u8) -> String {
    let multiplier = 10u64.pow(decimals as u32);
    let integer = units / multiplier;
    let fraction = units % multiplier;
    if fraction == 0 {
        integer.to_string()
    } else {
        let frac_str = format!("{fraction:0>width$}", width = decimals as usize);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{integer}.{trimmed}")
    }
}

pub fn build_transfer_url(params: &SolanaPayUrl) -> Result<String, String> {
    validate_address(&params.recipient)?;
    let mut url = format!("{}{}", SOLANA_PAY_PROTOCOL, params.recipient);
    url.push_str(&format!("?amount={}", params.amount));
    if let Some(ref token) = params.spl_token {
        if !token.is_empty() {
            validate_address(token)?;
            url.push_str(&format!("&spl-token={token}"));
        }
    }
    if let Some(ref ref_key) = params.reference {
        if !ref_key.is_empty() {
            validate_address(ref_key)?;
            url.push_str(&format!("&reference={ref_key}"));
        }
    }
    if !params.label.is_empty() {
        url.push_str(&format!("&label={}", urlencoding(&params.label)));
    }
    if !params.message.is_empty() {
        url.push_str(&format!("&message={}", urlencoding(&params.message)));
    }
    if let Some(ref memo) = params.memo {
        if !memo.is_empty() {
            url.push_str(&format!("&memo={}", urlencoding(memo)));
        }
    }
    Ok(url)
}

pub fn build_transfer_url_with_reference(
    recipient: &str,
    amount_units: u64,
    spl_token: Option<&str>,
    reference: &str,
    label: &str,
    message: &str,
    memo: Option<&str>,
) -> Result<String, String> {
    let params = SolanaPayUrl {
        recipient: recipient.to_string(),
        amount: amount_units,
        spl_token: spl_token.map(|s| s.to_string()),
        reference: Some(reference.to_string()),
        label: label.to_string(),
        message: message.to_string(),
        memo: memo.map(|s| s.to_string()),
    };
    build_transfer_url(&params)
}

pub fn verify_payment_from_rpc(
    reference_key: &str,
    rpc_response_json: &str,
    min_confirmations: u64,
) -> Result<PaymentVerification, String> {
    validate_address(reference_key)?;
    let parsed: serde_json::Value = serde_json::from_str(rpc_response_json)
        .map_err(|e| format!("Invalid RPC response JSON: {e}"))?;
    let result_val = if let Some(r) = parsed.get("result") {
        r.clone()
    } else if let Some(arr) = parsed.as_array() {
        serde_json::Value::Array(arr.clone())
    } else {
        return Err("No 'result' field in RPC response".to_string());
    };
    let signatures_arr = match result_val {
        serde_json::Value::Array(ref arr) => arr,
        _ => return Err("RPC result is not an array".to_string()),
    };
    let mut confirmed_count = 0usize;
    let mut sigs = Vec::new();
    let mut highest_slot: Option<u64> = None;
    for entry in signatures_arr {
        let sig = entry
            .get("signature")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let slot = entry.get("slot").and_then(|s| s.as_u64());
        let confirmations = entry
            .get("confirmations")
            .and_then(|c| {
                if c.is_null() { Some(u64::MAX) } else { c.as_u64() }
            })
            .unwrap_or(0);
        if sig.is_empty() { continue; }
        sigs.push(sig.to_string());
        if confirmations >= min_confirmations || confirmations == u64::MAX {
            confirmed_count += 1;
        }
        if let Some(s) = slot {
            match highest_slot {
                None => highest_slot = Some(s),
                Some(h) if s > h => highest_slot = Some(s),
                _ => {}
            }
        }
    }
    Ok(PaymentVerification {
        confirmed: confirmed_count > 0,
        signature_count: sigs.len(),
        signatures: sigs,
        slot: highest_slot,
        error: None,
    })
}

pub fn build_unsigned_sol_transfer(
    from: &str,
    to: &str,
    lamports: u64,
    blockhash: &str,
) -> Result<String, String> {
    let from_pk = parse_pubkey(from)?;
    let to_pk = parse_pubkey(to)?;
    let hash = parse_hash(blockhash)?;
    let system_id = parse_pubkey(SYSTEM_PROGRAM_ID)?;
    let ix = solana_instruction::Instruction {
        program_id: system_id,
        accounts: vec![
            solana_instruction::AccountMeta::new(from_pk, true),
            solana_instruction::AccountMeta::new(to_pk, false),
        ],
        data: encode_system_transfer_data(lamports),
    };
    let mut message = solana_message::Message::new(&[ix], Some(&from_pk));
    message.recent_blockhash = hash;
    let msg_bytes = bincode::serialize(&message)
        .map_err(|e| format!("Message serialization failed: {e}"))?;
    Ok(base64_engine().encode(msg_bytes))
}

pub fn build_unsigned_advance_nonce(
    nonce_address: &str,
    authority: &str,
    blockhash: &str,
) -> Result<String, String> {
    let nonce_pk = parse_pubkey(nonce_address)?;
    let auth_pk = parse_pubkey(authority)?;
    let hash = parse_hash(blockhash)?;
    let system_id = parse_pubkey(SYSTEM_PROGRAM_ID)?;
    let recent_blockhashes_id = parse_pubkey("SysvarRecentB1ockHashes11111111111111111111")?;

    let ix = solana_instruction::Instruction {
        program_id: system_id,
        accounts: vec![
            solana_instruction::AccountMeta::new(nonce_pk, false),
            solana_instruction::AccountMeta::new_readonly(auth_pk, true),
            solana_instruction::AccountMeta::new_readonly(recent_blockhashes_id, false),
        ],
        data: encode_advance_nonce_data(),
    };
    let mut message = solana_message::Message::new(&[ix], Some(&auth_pk));
    message.recent_blockhash = hash;
    let msg_bytes = bincode::serialize(&message)
        .map_err(|e| format!("Message serialization failed: {e}"))?;
    Ok(base64_engine().encode(msg_bytes))
}

pub fn build_advance_nonce_and_transfer(
    nonce_address: &str,
    authority: &str,
    from: &str,
    to: &str,
    lamports: u64,
    nonce_blockhash: &str,
) -> Result<String, String> {
    let nonce_pk = parse_pubkey(nonce_address)?;
    let auth_pk = parse_pubkey(authority)?;
    let from_pk = parse_pubkey(from)?;
    let to_pk = parse_pubkey(to)?;
    let hash = parse_hash(nonce_blockhash)?;
    let system_id = parse_pubkey(SYSTEM_PROGRAM_ID)?;
    let recent_blockhashes_id = parse_pubkey("SysvarRecentB1ockHashes11111111111111111111")?;
    let advance_ix = solana_instruction::Instruction {
        program_id: system_id,
        accounts: vec![
            solana_instruction::AccountMeta::new(nonce_pk, false),
            solana_instruction::AccountMeta::new_readonly(auth_pk, true),
            solana_instruction::AccountMeta::new_readonly(recent_blockhashes_id, false),
        ],
        data: encode_advance_nonce_data(),
    };
    let transfer_ix = solana_instruction::Instruction {
        program_id: system_id,
        accounts: vec![
            solana_instruction::AccountMeta::new(from_pk, true),
            solana_instruction::AccountMeta::new(to_pk, false),
        ],
        data: encode_system_transfer_data(lamports),
    };
    let mut message = solana_message::Message::new(&[advance_ix, transfer_ix], Some(&auth_pk));
    message.recent_blockhash = hash;
    let msg_bytes = bincode::serialize(&message)
        .map_err(|e| format!("Message serialization failed: {e}"))?;
    Ok(base64_engine().encode(msg_bytes))
}

pub fn build_create_and_init_nonce(
    nonce_address: &str,
    authority: &str,
    payer: &str,
    lamports: u64,
    blockhash: &str,
) -> Result<String, String> {
    let nonce_pk = parse_pubkey(nonce_address)?;
    let auth_pk = parse_pubkey(authority)?;
    let payer_pk = parse_pubkey(payer)?;
    let hash = parse_hash(blockhash)?;
    let system_id = parse_pubkey(SYSTEM_PROGRAM_ID)?;

    let create_ix = solana_instruction::Instruction {
        program_id: system_id,
        accounts: vec![
            solana_instruction::AccountMeta::new(payer_pk, true),
            solana_instruction::AccountMeta::new(nonce_pk, false),
        ],
        data: encode_create_account_data(lamports, NONCE_ACCOUNT_LENGTH, system_id),
    };
    let init_ix = solana_instruction::Instruction {
        program_id: system_id,
        accounts: vec![
            solana_instruction::AccountMeta::new(nonce_pk, false),
            solana_instruction::AccountMeta::new_readonly(auth_pk, true),
            solana_instruction::AccountMeta::new_readonly(
                parse_pubkey("SysvarRent111111111111111111111111111111111")?,
                false,
            ),
        ],
        data: encode_initialize_nonce_data(auth_pk),
    };
    let mut message = solana_message::Message::new(&[create_ix, init_ix], Some(&payer_pk));
    message.recent_blockhash = hash;
    let msg_bytes = bincode::serialize(&message)
        .map_err(|e| format!("Message serialization failed: {e}"))?;
    Ok(base64_engine().encode(msg_bytes))
}

fn encode_system_transfer_data(lamports: u64) -> Vec<u8> {
    let mut data = vec![2u8, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    data
}

fn encode_create_account_data(lamports: u64, space: u64, owner: solana_pubkey::Pubkey) -> Vec<u8> {
    let mut data = vec![0u8, 0, 0, 0];
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());
    data.extend_from_slice(&owner.to_bytes());
    data
}

fn encode_initialize_nonce_data(authority: solana_pubkey::Pubkey) -> Vec<u8> {
    let mut data = vec![6u8, 0, 0, 0];
    data.extend_from_slice(&authority.to_bytes());
    data
}

fn encode_advance_nonce_data() -> Vec<u8> {
    vec![7u8, 0, 0, 0]
}

pub fn derive_associated_token_account(wallet: &str, mint: &str) -> Result<String, String> {
    let wallet_pk = parse_pubkey(wallet)?;
    let mint_pk = parse_pubkey(mint)?;
    let token_prog_id = parse_pubkey(TOKEN_PROGRAM_ID)?;
    let ata_prog_id = parse_pubkey(ASSOCIATED_TOKEN_PROGRAM_ID)?;

    let seeds: &[&[u8]] = &[
        wallet_pk.as_ref(),
        token_prog_id.as_ref(),
        mint_pk.as_ref(),
    ];
    let (pda, _bump) = solana_pubkey::Pubkey::find_program_address(seeds, &ata_prog_id);
    Ok(pda.to_string())
}

fn parse_pubkey(s: &str) -> Result<solana_pubkey::Pubkey, String> {
    let trimmed = s.trim();
    let bytes = bs58::decode(trimmed)
        .into_vec()
        .map_err(|e| format!("Invalid base58 pubkey '{s}': {e}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "Pubkey '{s}' decoded to {} bytes, expected 32",
            bytes.len()
        ));
    }
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "Pubkey conversion failed".to_string())?;
    Ok(solana_pubkey::Pubkey::new_from_array(arr))
}

fn parse_hash(s: &str) -> Result<solana_hash::Hash, String> {
    let trimmed = s.trim();
    let bytes = bs58::decode(trimmed)
        .into_vec()
        .map_err(|e| format!("Invalid base58 hash '{s}': {e}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "Hash '{s}' decoded to {} bytes, expected 32",
            bytes.len()
        ));
    }
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "Hash conversion failed".to_string())?;
    Ok(solana_hash::Hash::new_from_array(arr))
}

fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}

fn base64_engine() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address_valid() {
        assert!(validate_address("11111111111111111111111111111111").is_ok());
        assert!(validate_address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").is_ok());
    }

    #[test]
    fn test_validate_address_invalid() {
        assert!(validate_address("short").is_err());
    }

    #[test]
    fn test_amount_to_units_sol() {
        assert_eq!(amount_to_units("1.5", 9).unwrap(), 1_500_000_000);
    }

    #[test]
    fn test_amount_to_units_usdc() {
        assert_eq!(amount_to_units("25.00", 6).unwrap(), 25_000_000);
    }

    #[test]
    fn test_amount_to_units_zero() {
        assert_eq!(amount_to_units("0", 9).unwrap(), 0);
    }

    #[test]
    fn test_units_to_amount() {
        assert_eq!(units_to_amount(1_500_000_000, 9), "1.5");
        assert_eq!(units_to_amount(25_000_000, 6), "25");
        assert_eq!(units_to_amount(1, 6), "0.000001");
    }

    #[test]
    fn test_build_transfer_url_sol() {
        let url = build_transfer_url(&SolanaPayUrl {
            recipient: "11111111111111111111111111111111".to_string(),
            amount: 1_500_000_000,
            spl_token: None,
            reference: None,
            label: "Test".to_string(),
            message: "Test payment".to_string(),
            memo: None,
        }).unwrap();
        assert!(url.starts_with("solana:11111111111111111111111111111111"));
        assert!(url.contains("amount=1500000000"));
        assert!(url.contains("label=Test"));
        assert!(url.contains("message=Test%20payment"));
    }

    #[test]
    fn test_build_transfer_url_spl() {
        let url = build_transfer_url(&SolanaPayUrl {
            recipient: "11111111111111111111111111111111".to_string(),
            amount: 25_000_000,
            spl_token: Some(USDC_MINT_MAINNET.to_string()),
            reference: Some("8m5J9KNFE1sCjYxJmYxJrNkQF7P7T7hLhL7pL7pL7pL".to_string()),
            label: "Shop".to_string(),
            message: "Invoice #412".to_string(),
            memo: Some("order-412".to_string()),
        }).unwrap();
        assert!(url.contains(&format!("spl-token={USDC_MINT_MAINNET}")));
        assert!(url.contains("reference=8m5J9KNFE1sCjYxJmYxJrNkQF7P7T7hLhL7pL7pL7pL"));
    }

    #[test]
    fn test_verify_payment_found() {
        let rpc = r#"{
            "result": [
                {"signature": "sig1", "slot": 100, "confirmations": 64},
                {"signature": "sig2", "slot": 99, "confirmations": 1}
            ]
        }"#;
        let result = verify_payment_from_rpc(
            "11111111111111111111111111111111", rpc, 32,
        ).unwrap();
        assert!(result.confirmed);
        assert_eq!(result.signature_count, 2);
    }

    #[test]
    fn test_verify_payment_empty() {
        let rpc = r#"{"result": []}"#;
        let result = verify_payment_from_rpc(
            "11111111111111111111111111111111", rpc, 1,
        ).unwrap();
        assert!(!result.confirmed);
        assert_eq!(result.signature_count, 0);
    }

    #[test]
    fn test_derive_ata() {
        let result = derive_associated_token_account(
            "11111111111111111111111111111111",
            USDC_MINT_MAINNET,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 44);
    }

    #[test]
    fn test_build_unsigned_sol_transfer() {
        let blockhash_str = "Cy6SH8KjK1S1YNsjyfcLNLFxqQ18aDjcHDSbCpiMfRPb";
        let result = build_unsigned_sol_transfer(
            "11111111111111111111111111111111",
            "4Q6ivcJN9LGTBryNUF65mEycqG5F3PMK2NkKjSJSkWUb",
            1_000_000,
            blockhash_str,
        );
        assert!(result.is_ok());
        let b64 = result.unwrap();
        assert!(!b64.is_empty());
        let bytes = base64_engine().decode(&b64).expect("valid base64");
        // Deserialize and verify recent_blockhash is set correctly
        let msg: solana_message::Message = bincode::deserialize(&bytes).expect("valid Message");
        assert_eq!(
            msg.recent_blockhash,
            parse_hash(blockhash_str).unwrap(),
            "recent_blockhash must match input blockhash"
        );
    }

    #[test]
    fn test_plugin_config_defaults() {
        let cfg = PluginConfig::from_section(&HashMap::new());
        assert_eq!(cfg.default_rpc_url, "https://api.mainnet-beta.solana.com");
    }

    #[test]
    fn test_plugin_config_custom() {
        let mut map = HashMap::new();
        map.insert("default_rpc_url".to_string(), "https://rpc.ankr.com/solana".to_string());
        map.insert("shop_label".to_string(), "My Shop".to_string());
        let cfg = PluginConfig::from_section(&map);
        assert_eq!(cfg.default_rpc_url, "https://rpc.ankr.com/solana");
        assert_eq!(cfg.shop_label, "My Shop");
    }

    #[test]
    fn test_amount_negative_fails() {
        assert!(amount_to_units("-5", 9).is_err());
    }

    #[test]
    fn test_normalize_address() {
        let n = normalize_address("11111111111111111111111111111111").unwrap();
        assert_eq!(n.len(), 32);
    }

    #[test]
    fn test_validate_address_wrong_length() {
        assert!(validate_address("abc").is_err());
    }

    #[test]
    fn test_build_transfer_url_with_reference_helper() {
        let url = build_transfer_url_with_reference(
            "11111111111111111111111111111111",
            25_000_000,
            Some(USDC_MINT_MAINNET),
            "11111111111111111111111111111111",
            "Shop", "Invoice #412",
            Some("order-412"),
        ).unwrap();
        assert!(url.contains("reference=11111111111111111111111111111111"));
    }

    #[test]
    fn test_build_create_and_init_nonce_success() {
        let blockhash_str = "Cy6SH8KjK1S1YNsjyfcLNLFxqQ18aDjcHDSbCpiMfRPb";
        let result = build_create_and_init_nonce(
            "8m5J9KNFE1sCjYxJmYxJrNkQF7P7T7hLhL7pL7pL7pL",
            "11111111111111111111111111111111",
            "11111111111111111111111111111111",
            1_500_000,
            blockhash_str,
        );
        assert!(result.is_ok());
        let b64 = result.unwrap();
        assert!(!b64.is_empty());
        let bytes = base64_engine().decode(&b64).expect("valid base64");
        let msg: solana_message::Message = bincode::deserialize(&bytes).expect("valid Message");
        assert_eq!(
            msg.recent_blockhash,
            parse_hash(blockhash_str).unwrap(),
            "recent_blockhash must match input blockhash"
        );
    }

    #[test]
    fn test_build_create_and_init_nonce_invalid_address() {
        let result = build_create_and_init_nonce(
            "short",
            "11111111111111111111111111111111",
            "11111111111111111111111111111111",
            1_500_000,
            "Cy6SH8KjK1S1YNsjyfcLNLFxqQ18aDjcHDSbCpiMfRPb",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_build_unsigned_advance_nonce_blockhash() {
        let blockhash_str = "Cy6SH8KjK1S1YNsjyfcLNLFxqQ18aDjcHDSbCpiMfRPb";
        let result = build_unsigned_advance_nonce(
            "8m5J9KNFE1sCjYxJmYxJrNkQF7P7T7hLhL7pL7pL7pL",
            "11111111111111111111111111111111",
            blockhash_str,
        );
        assert!(result.is_ok());
        let b64 = result.unwrap();
        assert!(!b64.is_empty());
        let bytes = base64_engine().decode(&b64).expect("valid base64");
        let msg: solana_message::Message = bincode::deserialize(&bytes).expect("valid Message");
        assert_eq!(
             msg.recent_blockhash,
             parse_hash(blockhash_str).unwrap(),
             "recent_blockhash must match input blockhash"
         );
     }

     #[test]
     fn test_build_advance_nonce_and_transfer() {
         let blockhash_str = "Cy6SH8KjK1S1YNsjyfcLNLFxqQ18aDjcHDSbCpiMfRPb";
         let result = build_advance_nonce_and_transfer(
             "8m5J9KNFE1sCjYxJmYxJrNkQF7P7T7hLhL7pL7pL7pL",
             "11111111111111111111111111111111",
             "11111111111111111111111111111111",
             "4Q6ivcJN9LGTBryNUF65mEycqG5F3PMK2NkKjSJSkWUb",
             1_000_000,
             blockhash_str,
         );
         assert!(result.is_ok());
         let b64 = result.unwrap();
         assert!(!b64.is_empty());
         let bytes = base64_engine().decode(&b64).expect("valid base64");
         let msg: solana_message::Message = bincode::deserialize(&bytes).expect("valid Message");
         assert_eq!(msg.recent_blockhash, parse_hash(blockhash_str).unwrap());
         assert_eq!(msg.instructions.len(), 2);
     }

     #[test]
     fn test_amount_to_units_precision() {
        assert_eq!(amount_to_units("0.1", 6).unwrap(), 100_000);
        assert_eq!(amount_to_units("0.000001", 6).unwrap(), 1);
        assert_eq!(amount_to_units("100.999999", 6).unwrap(), 100_999_999);
    }

    #[test]
    fn test_amount_to_units_no_decimal_point() {
        assert_eq!(amount_to_units("100", 6).unwrap(), 100_000_000);
    }

    #[test]
    fn test_amount_to_units_too_many_decimals() {
        assert!(amount_to_units("1.1234567", 6).is_err());
    }

    #[test]
    fn test_amount_to_units_empty_fails() {
        assert!(amount_to_units("", 6).is_err());
    }

    #[test]
    fn test_amount_to_units_overflow_fails() {
        assert!(amount_to_units("99999999999999999999", 0).is_err());
    }

    #[test]
    fn test_amount_to_units_large_decimals_fails() {
        assert!(amount_to_units("1", 255).is_err());
    }

    #[test]
    fn test_encode_advance_nonce_data_stable() {
        let data = encode_advance_nonce_data();
        assert_eq!(data, vec![7u8, 0, 0, 0]);
    }

    #[test]
    fn test_verify_payment_null_confirmations() {
        let rpc = r#"{"result": [{"signature": "sig1", "slot": 100, "confirmations": null}]}"#;
        let result = verify_payment_from_rpc(
            "11111111111111111111111111111111", rpc, 1,
        ).unwrap();
        assert!(result.confirmed);
    }

    #[test]
    fn test_verify_payment_malformed_entry_skipped() {
        let rpc = r#"{"result": [
            {"slot": 100, "confirmations": 64},
            {"signature": "valid_sig", "slot": 101, "confirmations": 64}
        ]}"#;
        let result = verify_payment_from_rpc(
            "11111111111111111111111111111111", rpc, 32,
        ).unwrap();
        assert!(result.confirmed);
        assert_eq!(result.signature_count, 1);
    }

    #[test]
    fn test_validate_address_solana_pay_32_char_max() {
        assert!(validate_address("1").is_err());
        let long = "a".repeat(45);
        assert!(validate_address(&long).is_err());
    }

    #[test]
    fn test_normalize_address_trim_and_roundtrip() {
        let raw = " 11111111111111111111111111111111 ";
        let n = normalize_address(raw).unwrap();
        assert_eq!(n.len(), 32);
        let n2 = normalize_address(&n).unwrap();
        assert_eq!(n, n2);
    }
}
