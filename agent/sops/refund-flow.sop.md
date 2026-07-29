# SOP: Process Refund with Approval Checkpoint
# This SOP requires human approval before executing a refund

trigger:
  type: channel
  channel: cli
  intent: "refund"

steps:
  - id: verify_original
    description: Verify the original payment exists
    action: memory.read
    params:
      key: "completed_payments.{{invoice_id}}"
    output: original_payment
    on_missing: "respond: Original payment not found. Cannot process refund."

  - id: validate_amount
    description: Ensure refund amount does not exceed original
    action: evaluate
    input:
      refund_amount: "{{refund_amount_units}}"
      original_amount: "{{original_payment.amount_units}}"
    logic: |
      if refund_amount > original_amount:
        fail("Refund amount exceeds original payment")
      if refund_amount <= 0:
        fail("Refund amount must be positive")

  - id: get_tx_signatures
    description: Get on-chain signatures for the original reference key
    action: tools.http_request
    params:
      method: POST
      url: "{{config.rpc_url}}"
      headers:
        Content-Type: application/json
      body:
        jsonrpc: "2.0"
        id: 1
        method: "getSignaturesForAddress"
        params:
          - "{{original_payment.reference_key}}"
          - limit: 1

  - id: get_original_tx_details
    description: Fetch the full transaction to extract the sender address
    condition: get_tx_signatures.result.result is not empty
    action: tools.http_request
    params:
      method: POST
      url: "{{config.rpc_url}}"
      headers:
        Content-Type: application/json
      body:
        jsonrpc: "2.0"
        id: 1
        method: "getTransaction"
        params:
          - "{{get_tx_signatures.result.result[0].signature}}"
          - encoding: "jsonParsed"

  - id: extract_sender
    description: Extract the sender address from the original transaction
    action: evaluate
    input:
      tx_data: "{{get_original_tx_details.result}}"
    logic: |
      fee_payer = tx_data.result.transaction.message.accountKeys[0].pubkey
      # The fee payer is the customer who paid. Verify refund goes back to them.

  - id: check_fraud
    description: Verify refund destination matches original sender
    action: evaluate
    input:
      original_sender: "{{extract_sender.fee_payer}}"
      requested_destination: "{{refund_destination}}"
    logic: |
      if requested_destination != original_sender:
        fail("Refund destination " + requested_destination + " does not match original payer " + original_sender + ". Flagged for manual review.")
        flag_for_review = true

  - id: human_approval
    description: >
      ⚠️ APPROVAL CHECKPOINT — Refund requires owner approval
      This step PAUSES execution until a human approves or rejects.
    action: checkpoint.approve
    params:
      channel: "{{config.owner_channel}}"
      message: |
        🔔 Refund request requires approval

        Original invoice: {{invoice_id}}
        Amount: {{refund_amount_formatted}} USDC
        Reason: {{refund_reason}}
        Customer: {{customer_id}}
        Original payer: {{extract_sender.fee_payer}}

        Reply "approve" to authorize or "reject" to decline.
      timeout: 3600  # 1 hour
      on_timeout: "reject"

  - id: execute_refund
    description: Generate Solana Pay refund URL for shop owner to scan
    condition: human_approval.result == "approved"
    steps:
      - id: create_refund_url
        description: Build Solana Pay URL that sends to the customer
        action: evaluate
        logic: |
          url = "solana:" + refund_destination
          url += "?amount=" + refund_amount_units
          url += "&spl-token=" + default_token_mint
          url += "&reference=" + new_guid()
          url += "&label=Shop Refund"
          url += "&message=Refund for " + invoice_id

      - id: notify_owner_scan
        description: Send the refund URL to the shop owner to scan and sign
        action: channel.send
        params:
          channel: "{{config.owner_channel}}"
          message: |
            ✅ Refund approved — scan this URL with your wallet to send
            Invoice: {{invoice_id}}
            Amount: {{refund_amount_formatted}} USDC
            Refund URL: {{create_refund_url.url}}
            Scan with Phantom/Backpack/Solflare to sign and send.

  - id: notify_rejected
    description: If human rejected
    condition: human_approval.result == "rejected"
    steps:
      - id: inform_customer
        action: channel.send
        params:
          channel: cli
          message: "Your refund request for {{invoice_id}} was declined by the shop owner. Please contact them directly for more information."
