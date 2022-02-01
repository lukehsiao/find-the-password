# Code Challenges for Youth

A collection of coding challenges for youth.

## Running

Typically run on a \$5/month DigitalOcean droplet running Ubuntu. This runs Rocket.rs, but I put
that behind a [Caddy](https://caddyserver.com/) server with the included Caddyfile for easy HTTPS.
All run in a tmux session for the duration of the challenge.

### Starting on a clean DigitalOcean Ubuntu droplet
1. Install Rust - https://www.rust-lang.org/tools/install
2. In a separate tmux/screen session, from taskN dir, run `cargo run`
    * If this fails with an error like `error: linker `cc` not found`, then run `sudo apt install build-essential`
3. Switch to the nightly Rust build (required for Rocket) - `rustup default nightly`
4. In a persistent tmux/screen session, run `cargo run`
5. In a separate tmux/screen session, install Caddy - https://caddyserver.com/docs/install#debian-ubuntu-raspbian
6. Create a Caddyfile from Caddyfile.example for your domain.
7. Run `sudo caddy run`
    * You may be able to run without `sudo`, but I got permission denied binding to port 443.
## Ideas

Some helpful links with other ideas

- [Ask HN: Please share your experience teaching your kids to program](https://news.ycombinator.com/item?id=25650224)
- [CS Unplugged](https://classic.csunplugged.org/wp-content/uploads/2015/03/CSUnplugged_OS_2015_v3.1.pdf)
- [The Hacker Way: How I taught my nephew to program](https://stopa.io/post/246)
