# just manual: https://github.com/casey/just

_default:
	@just --list

# Runs clippy on the sources
check:
	cargo clippy --all-features --all-targets --locked -- -W clippy::pedantic -D warnings

# Check links in markdown files
link-check:
	-lychee -E '**/*.md'

# Runs nextest
test:
	cargo nextest run

# Run the release binary
run addr="0.0.0.0:3000":
	LEPTOS_SITE_ADDR={{addr}} {{justfile_directory()}}/app/challenge

# Build the release binary
build:
	cargo leptos build --release
	mkdir -p {{justfile_directory()}}/app
	cp {{justfile_directory()}}/target/release/challenge {{justfile_directory()}}/app/
	cp -r {{justfile_directory()}}/target/site {{justfile_directory()}}/app/site
	cp {{justfile_directory()}}/Cargo.toml {{justfile_directory()}}/app/

# Format all sources, leptos-style
fmt:
	leptosfmt {{justfile_directory()}}

# Sets up a watcher that lints, tests, and builds
watch:
	bacon

# Update the changelog using git-cliff
_update_changelog version:
	#!/usr/bin/env bash
	set -euxo pipefail

	# Update changelog
	if ! command -v git-cliff &> /dev/null
	then
	    echo "Please install git-cliff: https://github.com/orhun/git-cliff#installation"
	    exit
	fi

	git-cliff --tag {{version}} --unreleased --prepend CHANGELOG.md
	${EDITOR:-vi} CHANGELOG.md
	git commit CHANGELOG.md -m "docs(CHANGELOG): add entry for {{version}}"

# Increment the version
_incr_version version: (_update_changelog version)
	#!/usr/bin/env bash
	set -euxo pipefail

	# Update version
	cargo set-version {{trim_start_match(version, "v")}}
	cargo build --release
	git commit Cargo.toml Cargo.lock -m "chore(release): bump version to {{version}}"

# Get the changelog and git stats for the release
_tlog describe version:
	# Format git-cliff output friendly for the tag
	@git cliff -c minimal --strip all --unreleased --tag {{version}} | sd "(^## .*\n\s+|^See the.*|^\[.*|^\s*$|^###\s)" ""
	@echo "$ git stats -r {{describe}}..{{version}}"
	@git stats -r {{describe}}..HEAD

# Target can be ["major", "minor", "patch", or a version]
release target: 
	#!/usr/bin/env python3
	# Inspired-by: https://git.sr.ht/~sircmpwn/dotfiles/tree/master/bin/semver
	import os
	import subprocess
	import sys
	import tempfile

	if subprocess.run(["git", "branch", "--show-current"], stdout=subprocess.PIPE
	        ).stdout.decode().strip() != "main":
	    print("WARNING! Not on the main branch.")

	subprocess.run(["git", "pull", "--rebase"])
	p = subprocess.run(["git", "describe", "--abbrev=0"], stdout=subprocess.PIPE)
	describe = p.stdout.decode().strip()
	old_version = describe[1:].split("-")[0].split(".")
	if len(old_version) == 2:
	    [major, minor] = old_version
	    [major, minor] = map(int, [major, minor])
	    patch = 0
	else:
	    [major, minor, patch] = old_version
	    [major, minor, patch] = map(int, [major, minor, patch])

	new_version = None

	if "{{target}}" == "patch":
	    patch += 1
	elif "{{target}}" == "minor":
	    minor += 1
	    patch = 0
	elif "{{target}}" == "major":
	    major += 1
	    minor = patch = 0
	else:
	    new_version = "{{target}}"

	if new_version is None:
	    if len(old_version) == 2 and patch == 0:
	        new_version = f"v{major}.{minor}"
	    else:
	        new_version = f"v{major}.{minor}.{patch}"

	p = subprocess.run(["just", "_tlog", describe, new_version],
	        stdout=subprocess.PIPE)
	shortlog = p.stdout.decode()

	p = subprocess.run(["just", "_incr_version", new_version])
	if p and p.returncode != 0:
	    print("Error: _incr_version returned nonzero exit code")
	    sys.exit(1)

	with tempfile.NamedTemporaryFile() as f:
	    basename = os.path.basename(os.getcwd())
	    f.write(f"{basename} {new_version}\n\n".encode())
	    f.write(shortlog.encode())
	    f.flush()
	    subprocess.run(["git", "tag", "-e", "-F", f.name, "-a", new_version])
	    print(new_version)
