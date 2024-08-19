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

In fact, this project is specifically a real implementation of a little toy challenge he proposes in the blog post.
In his words (emphasis mine):

> Stop fixing things for your kids.
> You spend hours of your time potty-training them when they're in their infancy, because being able to use the toilet is pretty much an essential skill in modern society.
> You need to do the same with technology.
> Buy them a computer by all means, but if it goes wrong, get them to fix it.
> Buy them a smartphone, give them £10 of app store credit a year and let them learn why in-app-purchases are a bad idea.
> When we teach kids to ride a bike, at some point we have to take the training wheels off.
> Here's an idea.
> **When they hit eleven, give them a plaintext file with ten-thousand WPA2 keys and tell them that the real one is in there somewhere.
> See how quickly they discover Python or Bash then.**

This project is a simple-to-run web app which generates a list of 60k passwords, and provides a simple URL scheme for checking passwords to know if they are right or wrong.

Typically, the challenge is presented with some rewards for the earliest solvers (and it shows a leaderboard for that), but the real goal is to tap into some natural curiosity and educate.

Note that there are still several ways you can take the challenge for different knowledge levels of youth.

1. Simple, brute-force serial approach.
   I don't have a level less than this, because I think the struggle and learning to get to this point is the crux of the challenge.
   For those with less experience, pointers on what to search, or even setting up a working Python environment with an example file that makes 1 web request might help.
   Note that there are other ways people might solve this (e.g., utilizing the web request functions of Excel), so really any solution works, but this is an opportunity to teach about programming.
2. More optimal, concurrent approach.
   E.g., if you have someone somewhat familiar with programming, have them implement a checker that brute forces all the passwords as fast as they can!
3. Design the challenge.
   If the concurrent brute force is too easy, it's time to talk system design.
   For these youth, pose the question "How would you implement this challenge?"
   This presents an opportunity to talk about tradeoffs in the design space.
   For example, the tradeoffs of the in-memory approach discussed below.

## Design

This challenge is essentially inviting youth to DDoS your server.
In addition, this is NOT intended to be a long-running application.

With these two things in mind, the design of this application takes the following approach:

- Simplify user creation.
  No authentication, etc.
  Youth can add themselves to reduce admin overhead, and we don't worry about extra persistence or auth or anything because the server will be killed after a short time anyway.
- All in-memory.
  We want **high throughput**.
  A previous iteration of this server was implemented with a sqlite database, but would cap at around 1k requests per second.
  By instead just using shared state entirely in memory, this server does closer to 60k rps.
  Eliminating backing services also simplifies things.
- But not too much memory.
  Since we want to use memory only, we don't want to use too much.
  For example, when generating a 60k list of passwords (~2MB), we could store that for each user after we first generate it.
  But, instead, to save memory, we just _regenerate_ it from a random seed that we save each time the password file is requests.
  This optimization makes sense because getting the password file should be rare compared to actually checking the passwords.
  We also do save the correct password directly so that that check is wicked fast.

So, what this looks like is essentially just 3 endpoints:

1. A homepage with instructions, a leaderboard, and a way for a youth to join the challenge.
2. A user-specific page that allows them to download a password file.
3. A password-checking API.

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

## Stories So Far

I've given this to a handful of youth, ranging from ages 10 to 18.

Here are some stories worth highlighting.

### Never underestimate the grit of a teenager

One of the first times I gave this challenge, I offered a monetary reward for the first person to find the password.
At the time, I was generating a list of 10k password as the blog post suggested.

One particularly determined young man had a high tolerance for manual repetition.
He had cleverly figured out how to generate the list of URLs from the passwords by pasting in the newline-separated `passwords.txt` into a spreadsheet, and then use spreadsheet functions to construct the URLs for each.

Then, he discovered there are websites ([example](https://www.websiteplanet.com/webtools/multiple-url/)) in which you can paste 1 URL per line, and it will open 1 tab per URL (the limit being the limits of the computer you're running it on).
In his case, he found that was about 500.
This young man then proceeded to open batches of 500 URLs at a time, gluing his eyes to the top left of the screen to look at the `false` or `true` printed there, positioned his mouse accordingly, and just closed tabs fast, waiting to see if a `true` flashed.
In his manual, brute force approach, he solved the puzzle in just a few hours.

Now, the list is 60k passwords.

### This is an opportunity to teach a growth mindset

The flip side to the thrill of seeing youth dive in with enthusiasm, is seeing a youth react very quickly with defeat.
This is not unusual if giving the challenge to a general group of youth.
Some will have a reaction of: trying several passwords by hand, realizing this is an untenable approach, and then quickly decided "they don't know" and quitting.
It feels even more heart-wrenching when you here defeated statements erroneously tied to identity, like "I'm not smart enough", or "I'm just bad at computers".

I hope if you give the challenge, you see these as opportunities.
Here is a moment in time that you might uniquely change someone's mind about what defines them, and what they can do.
My goal in these scenarios is to do my best to make this experience a memorable counter-example for that individual---yes, you can do it!

## Benchmarks

Just as a ballpark benchmark for actual password checking, I ran a test with [`oha`](https://github.com/hatoo/oha) against a release-built version of the server running on the same machine.
This shows throughput of **just under 60k requests/second** due to the full in-memory implementation.

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
