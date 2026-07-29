## Steps

1. **Verify original payment** — Load the completed payment record from memory.
   - tools: memory
   - input: {"type":"object","required":["invoice_id"],"properties":{"invoice_id":{"type":"string"}}}
   - output: {"type":"object","required":["original_payment"],"properties":{"original_payment":{"type":"object"}}}
   - next: 2

2. **Validate refund amount** — Ensure refund does not exceed original and is positive.
   - tools: none
   - input: {"type":"object","required":["refund_amount_units","original_amount"],"properties":{"refund_amount_units":{"type":"string"},"original_amount":{"type":"string"}}}
   - on_failure: fail
   - next: 3

3. **Check on-chain sender** — Fetch original transaction to extract the fee payer.
   - tools: http_request
   - output: {"type":"object","required":["fee_payer"],"properties":{"fee_payer":{"type":"string"}}}
   - next: 4

4. **Fraud check** — Verify refund destination matches original sender.
   - tools: none
   - input: {"type":"object","required":["refund_destination","original_sender"],"properties":{"refund_destination":{"type":"string"},"original_sender":{"type":"string"}}}
   - on_failure: fail
   - next: 5

5. **Owner approval** — Require human approval before executing refund.
   - kind: checkpoint
   - requires_confirmation: true
   - next: 6

6. **Execute refund** — Generate Solana Pay URL for owner to scan and sign.
   - tools: none
   - when: $.steps.5.result == "approved"
   - next: 7

7. **Notify rejected** — Inform customer their refund was declined.
   - tools: http_request
   - when: $.steps.5.result != "approved"
   - next: null
