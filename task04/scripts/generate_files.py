#!/usr/bin/env python
import base64
import random

lines = []
NUM_FILES = 10_000
LINES_PER_FILE = 10

# Inject the source text file, base64 encode
with open("../data/bookofmormon.txt") as f:
    for line in f:
        lines.append(base64.b64encode(line.encode()))


# Generate NUM_FILES files, with LINES_PER_FILE random lines from the source text
for i in range(NUM_FILES):
    with open(f"task04_{i:05}", "wb") as f:
        sample = random.sample(lines, k=LINES_PER_FILE)
        for line in sample:
            f.write(line)

