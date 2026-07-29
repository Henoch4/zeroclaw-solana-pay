# SOP: Verify Pending Payments
# Runs every 15 seconds to check for payment confirmations
# This is the watch-loop for the payment terminal

trigger:
  type: cron
  schedule: "*/15 * * * * *"
  max_runs: 240  # 60 minutes max monitoring window
  timeout_behavior: "skip"  # skip on rate-limit, don't mark failed
  retry_policy:
    max_retries: 3
    backoff: "exponential"
    status_codes: [429, 503]

state:
  consecutive_errors: 0
  max_consecutive_errors: 5

steps:
  - id: get_pending
    description: Load pending payments from memory
    action: memory.read
    params:
      key: "pending_payments"
    output: pending_list

  - id: check_if_empty
    description: Skip if no pending payments
    condition: pending_list is empty or not defined
    action: return
    output: "No pending payments to verify"

  - id: check_error_state
    description: Back off if too many consecutive RPC errors
    condition: "{{consecutive_errors}} >= {{max_consecutive_errors}}"
    action: return
    output: "Skipping run due to {{consecutive_errors}} consecutive RPC errors"

  - id: verify_payments
    description: Check each pending payment against the RPC
    foreach: payment in pending_list
    do:
      - id: query_rpc
        description: Query getSignaturesForAddress with reference key
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
              - "{{payment.reference_key}}"
              - limit: 10
        on_http_error:
          - action: increment
            target: "consecutive_errors"
          - action: return

      - id: reset_errors
        description: Reset error counter on successful RPC call
        action: evaluate
        logic: |
          consecutive_errors = 0

      - id: parse_result
        description: Check if payment is confirmed (32+ confirmations or finalized)
        action: evaluate
        input: "{{verify_payments.query_rpc.result}}"
        logic: |
          For each signature in result:
            If confirmations is null or confirmations >= 32:
              Mark payment as confirmed
            If any signature exists but none confirmed:
              Keep as pending
          If result has no signatures:
            Keep as pending

      - id: handle_confirmed
        description: Notify owner and clean up when payment is confirmed
        condition: parse_result has confirmed payments
        steps:
          - id: notify_owner
            action: channel.send
            params:
              channel: "{{config.owner_channel}}"
              message: |
                ✓ Payment confirmed — {{payment.label}}
                Amount: {{payment.amount_formatted}} {{payment.token}}
                Invoice: {{payment.invoice_id}}
                Reference: {{payment.reference_key_short}}
                Signature: {{parse_result.first_signature_short}}

          - id: notify_customer
            action: channel.send
            params:
              channel: "{{payment.customer_channel}}"
              message: |
                ✓ Your payment of {{payment.amount_formatted}} {{payment.token}} is confirmed!
                Thank you for your purchase.

          - id: store_record
            action: memory.write
            params:
              key: "completed_payments.{{payment.invoice_id}}"
              value:
                invoice_id: "{{payment.invoice_id}}"
                amount: "{{payment.amount_formatted}}"
                amount_units: "{{payment.amount_units}}"
                token: "{{payment.token}}"
                reference_key: "{{payment.reference_key}}"
                signature: "{{parse_result.first_signature}}"
                confirmed_at: "{{now}}"
                customer_id: "{{payment.customer_id}}"

          - id: append_to_list
            description: Append invoice_id to the completed list for daily reporting
            action: memory.list_append
            params:
              key: "completed_payments_list"
              value: "{{payment.invoice_id}}"

          - id: remove_pending
            action: memory.delete
            params:
              key: "pending_payments.{{payment.invoice_id}}"

      - id: handle_timeout
        description: If payment has been pending > 60 minutes, mark expired
        condition: "{{payment.created_at is defined and payment.created_at + 60 min < now}}"
        steps:
          - id: notify_timeout
            action: channel.send
            params:
              channel: "{{payment.customer_channel}}"
              message: "Payment request for {{payment.invoice_id}} has expired. Please request a new payment link."

          - id: remove_expired
            action: memory.delete
            params:
              key: "pending_payments.{{payment.invoice_id}}"

      - id: gc_stale_entries
        description: Remove pending entries missing created_at (data integrity sweep)
        condition: "{{payment.created_at is not defined}}"
        steps:
          - id: remove_stale
            action: memory.delete
            params:
              key: "pending_payments.{{payment.invoice_id}}"
