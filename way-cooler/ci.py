#!/usr/bin/env python2

import sys
import os
import re
import subprocess

from docopt import docopt

VERSION_REGEX = '(\\d+\\.\\d+\\.\\d+)'
BRANCH_REGEX = '$release*' + VERSION_REGEX + '^'
# If we grab the first 'version=' line in the Cargo files we'll be fine
CARGO_VERSION_LINE = '$version = "' + VERSION_REGEX + '"^'

FILE_REGEX_MAP = {
    "Cargo.toml": CARGO_VERSION_LINE,
    "Cargo.lock": CARGO_VERSION_LINE,
}

DOCOPT_USAGE = """way-cooler CI integration.

Usage:
  ci.py travis-check
  ci.py prepare-deploy
  ci.py bump <old version> <new version> [-v]
  ci.py (-h | --help)
  ci.py --version

Options:
  -h --help    Show this menu
  -v           Be verbose, print actions taken
  --version    Show version information
"""

def check_file_version(file_name, regex, expected):
    reg = re.compile(regex)
    with open(file_name) as f:
        for line in f.readlines():
            match = reg.match(line)
            if not match:
                continue
            elif match == expected:
                print('\t' + file_name + " updated.")
                return True
            else:
                print('\t' + file_name + ": expected " + expected + ", got " + match)
                return False
        print('\t' + file_name + ": did not find any version match!")
        return False

def check_release_branch(version):
    all_clear = True
    for (file_name, file_regex) in FILE_REGEX_MAP.items():
        print("Checking " + file_name)
        if not check_file_version(file_name, file_regex, version):
            all_clear = False
    return all_clear

if __name__ == "__main__":
    print("Running way-cooler ci script...")
    args = docopt(DOCOPT_USAGE, version="ci.py v1.0")
    if args["travis-check"]:
        print("Running travis-check...")
        travis_pr_branch = os.environ["TRAVIS_PULL_REQUEST_BRANCH"]
        if not travis_pr_branch:
            print("Not running in a PR.")
            sys.exit(0)
        print("PR " + travis_pr_branch + " detected, checking for versions.")
        version_match = re.match(BRANCH_REGEX, travis_pr_branch)
        if not version_match:
            print("Not in a release branch PR.")
            sys.exit(0)
        print("Checking versions in branch " + travis_pr_branch)
        if not check_release_branch(version_match):
            sys.stderr.write("Not all files matched!\n")
            sys.exit(2)
        print("All version checks passed.")


    elif args["bump"]:
        sys.stderr.write("Not supported yet")
        sys.exit(1)

    elif args["prepare-deploy"]:
        print("Not compiling for multiple targets yet :(")
        print("cargo build --release --verbose")
        retcode = subprocess.call(["cargo", "build", "--release", "--verbose"])
        if retcode != 0:
            sys.stderr.write("Cargo build exited with {}\n".format(retcode))
            sys.exit(2)

        print("Moving way-cooler => way-cooler_linux_x86_64")
        build_dir = os.environ["TRAVIS_BUILD_DIR"]
        os.rename(build_dir + "/target/release/way-cooler",
                  build_dir + "/way-cooler_linux_x86_64")

    else:
        sys.stderr.write("Invalid arguments!\n")
        print(DOCOPT_USAGE)
        sys.exit(1)
