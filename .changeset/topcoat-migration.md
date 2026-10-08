---
"challenge": minor
---

**refactor**: the server now runs on [Topcoat](https://github.com/tokio-rs/topcoat) instead of Leptos.

Pages are plain server-rendered HTML with no WebAssembly bundle, and the join and confirmation forms are ordinary form posts: success redirects with 303, and a mistake re-renders the page with its message and a semantic status (409, 422, or 429 with `Retry-After`). The check URL, `passwords.txt`, `/up`, and `/robots.txt` answer exactly as before, except that 404 bodies now read `not found`. The binary is the whole app, with the stylesheet and favicon compiled in, and it listens on `HOST`/`PORT` (default `127.0.0.1:3000`) instead of `LEPTOS_SITE_ADDR`.

Performance, measured against the previous release on the same machine (`oha`, median of 3 alternating runs): the home page serves about 15x the requests per second (9.7k to 153k, p99 5.8 ms to 0.6 ms), and gzip downloads of `passwords.txt` about 4x (130 to 544 per second, p99 99 ms to 22 ms) because responses now compress at the fastest level. Browser downloads of `passwords.txt` are unchanged: brotli now stands in for zstd, which Topcoat does not offer. The check URL is 6-8% slower (about 363k requests per second with keep-alive, down from 396k), the cost of Topcoat's per-request routing work over axum's.
