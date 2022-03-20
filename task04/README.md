# Running

This challenge simply stores state in-memory, and does not persist it to disk. Consequently, you
need to keep it running for the duration of the challenge. The recommended way to do this is using
tmux or screen.

Assuming Caddy and Rust are already installed, and this repo is already cloned, start a new tmux
session and run

```
$ cargo run --release 2>&1 | tee winner.log
```

The redirection and logging to a file is helpful if you need to reference what happened later.

While the server is running, you can then create, reset, and delete users. Assuming the server is
public at https://challenge.hsiao.dev

# Obstacle Course

- Find instructions in source file

- Download archive from hidden URL

- Recompress and upload to HTTP endpoint smaller than orig

- Informs user files are base64 encoded

- How many times does the word "excellent" appear in all of the files? 8

- Email me the number at task04@luke.hsiao.dev
