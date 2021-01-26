# Finding the password

I have a text file with 10,000 passwords. I seem to have lost my password in this file! Can you help
me find it?

Of course, there will be prizes:

- 1st Place: \$30
- 2nd Place \$15
- 3rd Place: \$10

## How to Play

The list of passwords is available at:

    https://challenge.hsiao.dev/passwords.txt

You can check if a password is the one I lost by checking the website with it in the URL following this template:

    https://challenge.hsiao.dev/01/<name>/<password>

Where `<name>` can be `myriam`, `alex`, `lily`, `dean`, `brian`, `timmy`, `lise`, and yes, even
`lio`; and `<password>` is the password you’re testing.

For example, if I wanted to test the password: `testpass`, I would visit

    https://challenge.hsiao.dev/01/luke/testpass

And I’d see the response:

    No

If I get the right password, I’d see:

    Yes

You can check your attempts by visiting

    https://challenge.hsiao.dev/status/<name>

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

*This challenge was inspired by Marc Scott's blog post: [Kids can't use computers... and this is why
it should worry you](http://www.coding2learn.org/blog/2013/07/29/kids-cant-use-computers/).*
> When we teach kids to ride a bike, at some point we have to take the training wheels off. Here's
> an idea. When they hit eleven, give them a plaintext file with ten-thousand WPA2 keys and tell
> them that the real one is in there somewhere. See how quickly they discover Python or Bash then.
