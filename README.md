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

This project assumes you have all the tooling for Leptos installed.
Specifically, make sure you have the [tooling specified here](https://book.leptos.dev/ssr/21_cargo_leptos.html?highlight=cargo-lept#introducing-cargo-leptos).

```
cargo install cargo-leptos
rustup target add wasm32-unknown-unknown
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
To make this easier, see

```
just run
```

## Example Solution

An example, fairly optimal but succinct solution can be found in the `examples/` directory.
I say fairly optimal because it uses full parallelism with `rayon`, and likely the more efficient, but just as fast option would be to just use concurrency with `tokio`.

Modify the source code as needed (e.g., to change the username or hostname for the URLs), then you can run it with

```
cargo run --release --example=passwords < /path/to/your/passwords.txt
```

## Benchmarks

Just as a ballpark benchmark for actual password checking, I ran a test with [`oha`](https://github.com/hatoo/oha) against a release-built version of the server running on the same machine.
This shows throughput of **just over 108k requests/second** due to the full in-memory implementation.
This is run on a PC with 64 GB of DDR5 RAM and a Ryzen 7 7800X3D (8-core, 16-thread).

```
❯ oha -n 800000 -c 25 --disable-keepalive http://localhost:3000/u/bench/check/asdf
Summary:
  Success rate: 100.00%
  Total:        7389.0231 ms
  Slowest:      3.1833 ms
  Fastest:      0.0623 ms
  Average:      0.2282 ms
  Requests/sec: 108268.7097

  Total data:   3.81 MiB
  Size/request: 5 B
  Size/sec:     528.66 KiB

Response time histogram:
  0.062 ms [1]      |
  0.374 ms [775171] |■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■
  0.687 ms [22717]  |
  0.999 ms [978]    |
  1.311 ms [355]    |
  1.623 ms [749]    |
  1.935 ms [2]      |
  2.247 ms [23]     |
  2.559 ms [0]      |
  2.871 ms [0]      |
  3.183 ms [4]      |

Response time distribution:
  10.00% in 0.1539 ms
  25.00% in 0.1820 ms
  50.00% in 0.2180 ms
  75.00% in 0.2584 ms
  90.00% in 0.3033 ms
  95.00% in 0.3415 ms
  99.00% in 0.4659 ms
  99.90% in 1.3047 ms
  99.99% in 1.4563 ms


Details (average, fastest, slowest):
  DNS+dialup:   0.0904 ms, 0.0203 ms, 1.3269 ms
  DNS-lookup:   0.0028 ms, 0.0009 ms, 0.7391 ms

Status code distribution:
  [200] 800000 responses
```

So, as long as there isn't a huge group of kids trying at a given time, it is likely a single server running locally can handle the load.

## Testing

Domain logic and the store have [property-based tests](https://hegel.dev/) (`just test`), and `tests/http.rs` drives the real router to lock the HTTP contract that solver scripts rely on.

```
just test       # cargo nextest run
just coverage   # cargo llvm-cov nextest
```

End-to-end tests live in `end2end/` and run against a live server via Playwright (`cargo leptos end-to-end`).
Playwright has no official Arch build, so locally the tests use the system Chromium instead of Playwright's fragile fallback download.
With [`mise`](https://mise.jdx.dev/) and `chromium` installed, the whole thing is one command:

```
sudo pacman -S chromium   # once
mise run e2e              # npm ci, then cargo leptos end-to-end
```

`mise.toml` sets `PLAYWRIGHT_CHROMIUM_PATH=/usr/bin/chromium` and skips the browser download.
CI runs on Ubuntu, where Playwright's own browser works, so it ignores this and installs the bundled Chromium normally.
