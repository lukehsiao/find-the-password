---
"challenge": patch
---

**fix**: a throttled confirmation's `Retry-After` header now gives the time actually left in the cooldown, rounded up to whole seconds, instead of always the full 10 seconds.
