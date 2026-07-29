# SOP: Daily Revenue Report
# Runs at end of day to summarize payment activity

trigger:
  type: cron
  schedule: "0 23 * * *"  # 11 PM daily
  timezone: "America/Sao_Paulo"

steps:
  - id: get_completed_ids
    description: Load list of completed invoice IDs
    action: memory.read
    params:
      key: "completed_payments_list"
    output: completed_ids
    on_missing: "respond: No completed payments today."

  - id: load_invoices
    description: Load each invoice detail
    foreach: invoice_id in completed_ids
    do:
      - id: get_invoice
        action: memory.read
        params:
          key: "completed_payments.{{invoice_id}}"
        output: invoice_{{invoice_id}}

  - id: reconcile
    description: Check on-chain balances to verify
    action: tools.http_request
    params:
      method: POST
      url: "{{config.rpc_url}}"
      headers:
        Content-Type: application/json
      body:
        jsonrpc: "2.0"
        id: 1
        method: "getTokenAccountBalance"
        params:
          - "{{shop_usdc_token_account}}"

  - id: build_report
    description: Generate daily summary
    action: evaluate
    input: "{{load_invoices}}"
    logic: |
      total_orders = count
      total_volume = sum of amounts
      unique_customers = distinct customer_ids

  - id: send_report
    description: Send report to owner
    action: channel.send
    params:
      channel: "{{config.owner_channel}}"
      message: |
        Daily Revenue Report
        {{now|date:"YYYY-MM-DD"}}

        Orders completed: {{build_report.total_orders}}
        Total volume: {{build_report.total_volume}} USDC
        Unique customers: {{build_report.unique_customers}}

        On-chain balance: {{reconcile.result.value.uiAmountString}} USDC
