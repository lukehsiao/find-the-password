---
"challenge": minor
---

**refactor**: the server now runs on [Topcoat](https://github.com/tokio-rs/topcoat) instead of Leptos. Pages are plain server-rendered HTML with no WebAssembly bundle, and the join and confirmation forms are ordinary form posts: success redirects with 303, and a mistake re-renders the page with its message and a semantic status (409, 422, or 429 with `Retry-After`). The check URL, `passwords.txt`, `/up`, and `/robots.txt` answer exactly as before, except that 404 bodies now read `not found`. The binary is the whole app, with the stylesheet and favicon compiled in, and it listens on `HOST`/`PORT` (default `127.0.0.1:3000`) instead of `LEPTOS_SITE_ADDR`.
