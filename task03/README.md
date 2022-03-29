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

Create a new username `new_user`.
```
$ curl -X POST https://challenge.hsiao.dev/03/u/new_user
```

Reset `new_user`, removing them from winners and taking away their hits from `total_hits`.
```
$ curl -X PATCH https://challenge.hsiao.dev/03/u/new_user
```

Delete `new_user`, removing them from winners and taking away their hits from `total_hits`.
```
$ curl -X DELETE https://challenge.hsiao.dev/03/u/new_user
```

Obviously, these are not secured in any way. However, given the audience for this project will just
be issuing GET requests (most likely?), we should be relatively safe from accidental state changes.


# Finding the password

I have a text file with 20,000 passwords. I seem to have lost my password in this file! Can you help
me find it?

Of course, there will be prizes:

- 1st Place: \$30
- 2nd Place \$15
- 3rd Place: \$10

## How to Play

The list of passwords is available at:

    https://challenge.hsiao.dev/03/<name>/passwords.txt

Where `<name>` is your username (e.g., `alexh`).

You can check if a password is the one I lost by checking the website with it in the URL following this template:

    https://challenge.hsiao.dev/03/<name>/check/<password>

For example, if I wanted to test the password: `testpass`, I would visit

    https://challenge.hsiao.dev/03/luke/check/testpass

And I’d see the response:

    False

If I get the right password, I’d see:

    True

You can check some stats about everyone's attempts by visiting

    https://challenge.hsiao.dev/03/status

## Rules

- No sharing a solution with each other, everyone has to do their own work, but you’re free to collaborate!
- If you can solve it, you have to share with me what you did!
- Parents are not allowed to help much. I’ll leave it to parents' judgement on what “much” is. When in doubt, feel free to send them to me!
- Only use the url with your own name in it, don’t impersonate others!
- There is no limit to how many times you can try!
- I will update this email thread as prizes are claimed!

## Some solutions

Brute force is the only answer.

- Ideally, this is a trivial for loop over the passwords and making a web requests, checking for
  "Yes" in the response.
- Turns out you can also use a spreadsheet, leveraging something like `=WEBSERVICE` to make the web
  requests for you. Turns out Google Sheet's `IMPORTDATA` only allows 50 per sheet, so no go there.
- Some kids actually did brute force, turns out 10k wasn't crazy enough! Their approach was to use a
  multiple tab opener and literally open hundreds of Chrome tabs, closing them quickly as they kept
  their eyes trained on where "No" and "Yes" were displayed.

## Running the Parallelized Rust Example Solution

```
$ curl -L https://challenge.hsiao.dev/03/u/luke/passwords.txt | cargo run --release --example=solution --
```

_This challenge was inspired by Marc Scott's blog post: [Kids can't use computers... and this is why
it should worry you](http://www.coding2learn.org/blog/2013/07/29/kids-cant-use-computers/)._

> When we teach kids to ride a bike, at some point we have to take the training wheels off. Here's
> an idea. When they hit eleven, give them a plaintext file with ten-thousand WPA2 keys and tell
> them that the real one is in there somewhere. See how quickly they discover Python or Bash then.

