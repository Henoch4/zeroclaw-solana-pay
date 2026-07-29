use solana_wallet::core::*;

#[test]
fn test_full_payment_flow() {
    let recipient = "4Q6ivcJN9LGTBryNUF65mEycqG5F3PMK2NkKjSJSkWUb";
    let reference = "11111111111111111111111111111111";

    let url = build_transfer_url_with_reference(
        recipient,
        25_000_000,
        Some(USDC_MINT_MAINNET),
        reference,
        "Shop",
        "Invoice #412",
        None,
    )
    .unwrap();

    assert!(url.starts_with("solana:"));
    assert!(url.contains("amount=25000000"));
    assert!(url.contains(&format!("spl-token={USDC_MINT_MAINNET}")));
    assert!(url.contains(&format!("reference={reference}")));

    let rpc = format!(
        r#"{{"result": [{{"signature": "sig1", "slot": 100, "confirmations": 64}}]}}"#
    );
    let verification = verify_payment_from_rpc(reference, &rpc, 32).unwrap();
    assert!(verification.confirmed);
    assert_eq!(verification.signature_count, 1);
}

#[test]
fn test_amount_conversion_roundtrip() {
    let original = "42.50";
    let units = amount_to_units(original, 6).unwrap();
    let back = units_to_amount(units, 6);
    assert_eq!(back, "42.5");
}

#[test]
fn test_ata_derivation_consistency() {
    let wallet = "11111111111111111111111111111111";
    let ata1 = derive_associated_token_account(wallet, USDC_MINT_MAINNET).unwrap();
    let ata2 = derive_associated_token_account(wallet, USDC_MINT_MAINNET).unwrap();
    assert_eq!(ata1, ata2);
}

#[test]
fn test_validate_solana_pay_url() {
    let url = build_transfer_url_with_reference(
        "11111111111111111111111111111111",
        100_000_000,
        None,
        "11111111111111111111111111111111",
        "Label",
        "Msg",
        None,
    )
    .unwrap();
    assert!(url.starts_with("solana:11111111111111111111111111111111?amount=100000000"));
    assert!(url.contains("label=Label"));
    assert!(url.contains("message=Msg"));
}

#[test]
fn test_payment_not_confirmed() {
    let rpc = r#"{"result": [{"signature": "sig1", "slot": 1, "confirmations": 0}]}"#;
    let result = verify_payment_from_rpc(
        "11111111111111111111111111111111",
        rpc,
        32,
    )
    .unwrap();
    assert!(!result.confirmed);
}
