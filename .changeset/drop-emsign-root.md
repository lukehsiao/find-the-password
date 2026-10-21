---
"challenge": patch
---

**fix**: make the example solver reject certificates issued by the compromised eMudhra emSign G3 intermediates ([Mozilla bug 2083449](https://bugzilla.mozilla.org/show_bug.cgi?id=2083449)).

The private keys of eight eMudhra intermediates are now public, so anyone can mint certificates that chain to them. `rustls-webpki` 0.103.16 rejects any path through those keys ([RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382)), and `webpki-root-certs` 1.0.10 drops `emSign ECC Root CA - G3` from the `wasm32` bundle ([RUSTSEC-2026-0383](https://rustsec.org/advisories/RUSTSEC-2026-0383)). `rustls` 0.23.46 also fixes [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346) through [RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349). Only the `passwords` example makes outbound TLS connections; the server itself does not.

All other dependencies are also upgraded to their latest versions.
