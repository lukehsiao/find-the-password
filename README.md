# Code Challenges for Youth

A collection of coding challenges for youth.

## Running

Typically run on a \$5/month DigitalOcean droplet running Ubuntu using
[Caddy](https://caddyserver.com/) as a reverse proxy with easy HTTPS.
All run in a tmux session for the duration of the challenge. See the individual tasks for additional
details.

## Setting Up Caddy

Install Caddy: https://caddyserver.com/docs/install

Recommend installing via `apt` so it is already set up as a systemd service.

Then, configure the Caddyfile for your domain.

```
$ sudoedit /etc/caddy/Caddyfile
```

Once configuration is done, reload caddy.

```
$ sudo systemctl reload caddy
```

You can view the caddy logs using journalctl.

```
$ sudo journalctl -b 0 -u caddy.service
```

An example Caddyfile is provided in Caddy.example.

## TODO Ideas

Some helpful links with other ideas

- [Ask HN: Please share your experience teaching your kids to program](https://news.ycombinator.com/item?id=25650224)
- [CS Unplugged](https://classic.csunplugged.org/wp-content/uploads/2015/03/CSUnplugged_OS_2015_v3.1.pdf)
- [The Hacker Way: How I taught my nephew to program](https://stopa.io/post/246)
