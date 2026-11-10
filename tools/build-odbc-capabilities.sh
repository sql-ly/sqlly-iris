#!/bin/sh
set -eu

output="${1:-target/odbc-capabilities}"
mkdir -p "$(dirname "$output")"
cc $(odbc_config --cflags) -std=c11 -Wall -Wextra -Werror tools/odbc-capabilities.c -o "$output" $(odbc_config --libs)
