---
"challenge": patch
---

**refactor**: the server now runs on [Topcoat](https://github.com/tokio-rs/topcoat) 0.12 and needs Rust 1.99 or later to build. Each page's `<head>` now also carries `og:type`, `og:title`, and `twitter:card` tags, so shared links preview with the page title. Every route and status, and every plain-text response body, is unchanged.
