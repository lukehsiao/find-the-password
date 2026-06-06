---
"challenge": patch
---

**perf**: stop passwords.txt downloads from stalling password checks, and trim the check hot path.

The password file was generated while holding a dashmap shard guard, so each download stalled every check on that shard for several milliseconds (check p99.9 under concurrent downloads: 6.5ms -> 0.32ms). Generation also fills a single flat buffer now instead of allocating 60,000 intermediate Strings, with a property test pinning the output byte for byte. Beyond that, the check route skips the compression layer, the server binary picks up mimalloc and thin LTO, wrong guesses no longer read the clock, and the check handler borrows its path params instead of allocating them. All of this pushes our throughput up from like 77krps to 108krps.
