# Find the Password

A brute-force challenge that teaches youth to automate with code: each player hunts for one secret among thousands of candidate passwords.

## Language

### People

**Player**:
A person who joined the challenge under a username.
_Avoid_: User, account

**Join**:
Becoming a player by choosing a username.
_Avoid_: register, sign up

### The hunt

**Password file**:
A player's own list of candidate passwords, served as `passwords.txt`.
_Avoid_: password list, wordlist

**Secret**:
The one correct password hidden in a player's password file.
_Avoid_: answer, solution

**Password**:
Any line of a password file, or any guess a player sends.

**Check**:
A guess sent through the check URL. It answers only true or false and never solves.
_Avoid_: hit, try

**Confirm**:
Submitting a password on the player page. Only a confirm can solve.
_Avoid_: submit, verify

**Attempt**:
A guess, by check or by confirm, that the server evaluates before the player has solved.
_Avoid_: hit

### Outcomes

**Solve**:
A player's first correct confirm.
_Avoid_: completion, finish

**Time to solve**:
The time from a player joining to their solve.

**Leaderboard**:
The players who have solved, in solve order.
_Avoid_: leaders

**Roster**:
Every player, ordered by attempts.
_Avoid_: all players

**Standings**:
The leaderboard and the roster shown together.
