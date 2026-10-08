<h1 align="center">
    🔍<br>
    Challenge: Find the Password
</h1>
<div align="center">
    <strong>A brute-force challenge to introduce computer automation to youth.</strong>
</div>
<br>
<div align="center">
  <a href="https://github.com/lukehsiao/find-the-password/actions/workflows/general.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/lukehsiao/find-the-password/general.yml" alt="Build Status">
  </a>
  <a href="https://github.com/lukehsiao/fine-the-password/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-BlueOak--1.0.0-whitesmoke" alt="License">
  </a>
</div>
<br>

## Introduction

Back in 2013, Marc Scott wrote a great blog post: [Kids can't use computers...and this is why it should worry you](http://coding2learn.org/blog/2013/07/29/kids-cant-use-computers/).
If you haven't read it, I highly recommend it!
In my opinion, the take was true in 2013, and even more true now, over a decade later.

An arguably-too-brief summary of the post is: despite the widespread use of technology, many people, including children and adults, lack true technical literacy.
Scott shares personal anecdotes to illustrate how even basic tasks on computers often baffle users.

Why does this matter? Scott concludes:

> I want the people who will help shape our society in the future to understand the technology that will help shape our society in the future.
> If this is going to happen, then we need to reverse the trend that is seeing digital illiteracy exponentially increase.
> We need to act together, as parents, as teachers, as policy makers.
> Let's build a generation of hackers. Who's with me?

I'm with him.

Software continues to [eat the world](https://a16z.com/why-software-is-eating-the-world/), and no matter what profession you ultimately work in, the probability that you are affected by and/or dependent on computer systems is high.
My blog post sets more context around the design of this server and some anecdotal stories from giving this challenge over the years.

<div align="center">

**<https://luke.hsiao.dev/blog/find-the-password/>**

</div>

## Running via Docker

We have a docker image of the latest commit on main.

To run, you can do something like:

```
docker run -d -p 8080:8080 --name find-the-password ghcr.io/lukehsiao/find-the-password:latest
```

## Building and Running

### Prerequisites

The server is a plain Rust binary built on [Topcoat](https://github.com/tokio-rs/topcoat), so a Rust 1.98+ toolchain is all it needs.
For live reload while editing and for formatting `view!` macros, also install the Topcoat CLI:

```
cargo install topcoat-cli --version 0.10.0 --locked
```

### Building

This project also uses [`just`](https://just.systems/man/en/chapter_4.html) as a command runner.
Install `just`.

Then, you can simply run

```
just build
```

### Running

While we could add a Dockerfile or similar to this project, I typically just run it directly on a server.
The stylesheet and favicon are compiled in, so the binary is the whole app.
It listens on `HOST`:`PORT` (default `127.0.0.1:3000`).
To make this easier, see

```
just run          # the release binary from `just build`, on 0.0.0.0:3000
just dev          # topcoat dev: rebuilds and reloads the page on save
```

## Example Solution

An example, fairly optimal but succinct solution can be found in the `examples/` directory.
It uses `tokio` and `reqwest` to run up to 256 requests concurrently (via `buffer_unordered`), and streams the password list from stdin line by line, so only the in-flight passwords are held in memory no matter how large the file is.

Pass the username and the challenge's hostname, then pipe in a password list:

```
cargo run --release --example=passwords -- -u <username> -H http://localhost:3000 < /path/to/your/passwords.txt
```

## Benchmarks

Just as a ballpark benchmark for actual password checking, I ran a test with [`oha`](https://github.com/hatoo/oha) against a release-built version of the server running on the same machine.
This shows throughput of **just over 107k requests/second** due to the full in-memory implementation.
This is run on a PC with 64 GB of DDR5 RAM and a Ryzen 7 7800X3D (8-core, 16-thread).

```
❯ oha -n 800000 -c 25 --disable-keepalive http://localhost:3000/u/bench/check/asdf
Summary:
  Success rate: 100.00%
  Total:        7446.2643 ms
  Slowest:      10.1764 ms
  Fastest:      0.0579 ms
  Average:      0.2314 ms
  Requests/sec: 107436.4228

  Total data:   3.81 MiB
  Size/request: 5 B
  Size/sec:     524.59 KiB

Response time histogram:
   0.058 ms [1]      |
   1.070 ms [799885] |■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■
   2.082 ms [106]    |
   3.093 ms [2]      |
   4.105 ms [0]      |
   5.117 ms [0]      |
   6.129 ms [0]      |
   7.141 ms [0]      |
   8.153 ms [0]      |
   9.165 ms [0]      |
  10.176 ms [6]      |

Response time distribution:
  10.00% in 0.1672 ms
  25.00% in 0.1846 ms
  50.00% in 0.2129 ms
  75.00% in 0.2629 ms
  90.00% in 0.3170 ms
  95.00% in 0.3596 ms
  99.00% in 0.4675 ms
  99.90% in 0.7444 ms
  99.99% in 1.1355 ms


Details (average, fastest, slowest):
  DNS+dialup:   0.0630 ms, 0.0213 ms, 10.0012 ms
  DNS-lookup:   0.0020 ms, 0.0007 ms, 0.4483 ms

Status code distribution:
  [200] 800000 responses
```

So, as long as there isn't a huge group of kids trying at a given time, it is likely a single server running locally can handle the load.

## Testing

Domain logic and the store have [property-based tests](https://hegel.dev/) (`just test`), and the route and page tests drive the real router in-process to lock the HTTP contract that solver scripts rely on.

```
just test       # cargo nextest run
just coverage   # cargo llvm-cov nextest
```

End-to-end tests live in `end2end/` and run against a live server via Playwright (`just e2e`), which builds and starts the app itself.
Playwright has no official Arch build, so locally the tests use the system Chromium instead of Playwright's fragile fallback download.
With [`mise`](https://mise.jdx.dev/) and `chromium` installed, the whole thing is one command:

```
sudo pacman -S chromium   # once
mise run e2e              # pnpm install, then just e2e
```

`mise.toml` sets `PLAYWRIGHT_CHROMIUM_PATH=/usr/bin/chromium` and skips the browser download.
CI runs on Ubuntu, where Playwright's own browser works, so it ignores this and installs the bundled Chromium normally.
