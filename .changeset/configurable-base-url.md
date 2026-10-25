---
"challenge": minor
---

**feat**: the how-to-play instructions now show URLs under the `BASE_URL` environment variable, which defaults to `http://localhost:$PORT`, instead of a hardcoded `https://challenge.hsiao.dev`. Self-hosted servers now show players their own address. An invalid `BASE_URL`, or one with a path, stops the server at startup.
