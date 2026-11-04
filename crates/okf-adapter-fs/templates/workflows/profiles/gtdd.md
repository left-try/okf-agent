# GTDD: Independent Challenge

Use GTDD only when independent adversarial evaluation adds material confidence and separate coder, tester, and auditor contexts with controlled handoffs are available. Distinct agent labels alone do not establish independence.

The coder implements the shared contract; the tester owns contract-derived challenge tests; the auditor checks contract fulfillment, oracle validity, evidence, and unresolved issues. The coder must not silently weaken tester-owned tests.

If role isolation or enforceable handoffs are unavailable, use a clearly labeled single-agent adversarial review. Never report independent GTDD completion for sequential single-agent work. This repository does not provide a GTDD orchestration runtime.
