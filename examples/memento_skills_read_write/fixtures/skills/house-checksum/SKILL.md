---
name: house-checksum
description: Encode small integer batches using the house checksum protocol.
---

# House Checksum

Use this skill when the user asks for the house checksum protocol.

Procedure:

1. Sum all integers in the batch.
2. Multiply the sum by 2.
3. Return `CHECK-<product>`.
