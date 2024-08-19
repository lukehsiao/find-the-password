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
This shows throughput of **just under 60k requests/second** due to the full in-memory implementation.
This is run on a PC with 64 GB of DDR5 RAM and a Ryzen 7 7800X3D (8-core, 16-thread).

```
❯ oha -n 500000 -c 10 --disable-keepalive http://localhost:3000/u/luke/check/asdf
Summary:
  Success rate:     100.00%
  Total:        8.5331 secs
  Slowest:     0.0011 secs
  Fastest:     0.0001 secs
  Average:     0.0002 secs
  Requests/sec: 58595.5887

  Total data:   2.38 MiB
  Size/request: 5 B
  Size/sec:     286.11 KiB

Response time histogram:
  0.000 [1]      |
  0.000 [335249] |■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■
  0.000 [136740] |■■■■■■■■■■■■■
  0.000 [16186]  |■
  0.000 [8683]   |
  0.001 [1556]   |
  0.001 [810]    |
  0.001 [445]    |
  0.001 [184]    |
  0.001 [119]    |
  0.001 [27]     |

Response time distribution:
  10.00% in 0.0001 secs
  25.00% in 0.0001 secs
  50.00% in 0.0002 secs
  75.00% in 0.0002 secs
  90.00% in 0.0002 secs
  95.00% in 0.0003 secs
  99.00% in 0.0005 secs
  99.90% in 0.0008 secs
  99.99% in 0.0010 secs


Details (average, fastest, slowest):
  DNS+dialup:   0.0001 secs, 0.0000 secs, 0.0008 secs
  DNS-lookup:   0.0000 secs, 0.0000 secs, 0.0005 secs

Status code distribution:
  [200] 500000 responses
```

So, as long as there isn't a huge group of kids trying at a given time, it is likely a single server running locally can handle the load.
