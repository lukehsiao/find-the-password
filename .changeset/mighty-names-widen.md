---
"challenge": patch
---

**refactor**: usernames may now include hyphens, periods, and underscores alongside ASCII letters and digits. All three are URL-unreserved characters, and the 3-character minimum already rules out the special `.` and `..` path segments, so the new names drop into `/u/{username}` URLs verbatim.
