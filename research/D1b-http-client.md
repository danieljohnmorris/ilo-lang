# D1b HTTP Client Research

Choosing an HTTP client for the `get`/`$` builtin. Requirements: simple GET returning body or error as text. No POST, headers, or cookies needed initially. Project is fully synchronous (no async runtime).

## Options Compared

| Library | Transitive Deps | TLS | Redirects | Timeouts | API |
|---------|-----------------|-----|-----------|----------|-----|
| **minreq** | 0 (plain HTTP) | opt-in (`rustls`/`native-tls` features) | opt-in (`redirects` feature) | built-in | `minreq::get(url).send()?.as_str()?` |
| **ureq 3.x** | ~5 (base64, log, utf8-zero, etc.) | rustls by default | yes | yes | `ureq::get(url).call()?.body_mut().read_to_string()?` |
| **attohttpc** | ~4 (base64, http, log, url) | opt-in | yes | yes | `attohttpc::get(url).send()?.text()?` |
| **reqwest** | 50+ (tokio, hyper, etc.) | yes | yes | yes | `reqwest::blocking::get(url)?.text()?` |
| **std::net** | 0 | no | no | manual | ~40 lines of manual HTTP/1.1 |

## Analysis

**minreq** — Zero mandatory deps with `default-features = false`. ~214 GitHub stars, actively maintained (v2.14, 2025). TLS and redirects available as opt-in features. Clean API. Only limitation: no redirect following by default (fine for a builtin that targets APIs returning JSON).

**ureq** — Popular (~2k stars), good API, but ureq 3.x pulled in several deps even at baseline. More than we need for a simple GET.

**attohttpc** — Decent middle ground but fewer users (~285 stars) and slightly more deps than minreq.

**reqwest** — Massively over-specified. 50+ transitive deps, pulls in tokio even in blocking mode. Reserved for D1d tool provider infrastructure.

**std::net** — Zero deps but no TLS at all, and ~40 lines of manual HTTP parsing. Fragile.

## Recommendation: minreq

```toml
[dependencies]
minreq = { version = "2.14", default-features = false, optional = true }

[features]
default = ["cranelift", "http"]
http = ["dep:minreq"]
http-tls = ["http", "minreq/https-rustls"]
```

- `http` feature: plain HTTP only, zero transitive deps
- `http-tls` feature: adds rustls for HTTPS support
- Both opt-in, so `--no-default-features` keeps ilo dependency-free

Start with `http` in default features. Add `http-tls` to default once we confirm rustls compile times are acceptable.

### Trade-offs accepted
- No redirect following (can add `minreq/redirects` later)
- No HTTPS without `http-tls` feature (acceptable for initial implementation, most dev/testing uses HTTP)
- Small library (~214 stars) but stable API, no unsafe, well-tested
