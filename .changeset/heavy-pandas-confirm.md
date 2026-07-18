---
"challenge": minor
---

**Feature**: Finding the password no longer auto-solves the challenge. The check URL still answers `true`/`false`, but the solve is only recorded once the player returns to their user page and confirms the password there. Every evaluated guess counts as an attempt, whether it goes through the check URL or the confirmation box, and confirmations are throttled to one every 10 seconds so the form itself can't be brute-forced. Server function errors now also carry semantically correct HTTP statuses (404, 409, 422, and 429 with `Retry-After`) instead of a blanket 500.
