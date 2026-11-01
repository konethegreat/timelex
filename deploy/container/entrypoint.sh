#!/bin/sh
# Written by Kone Tshivhinda & Claude
# Loads the locally generated development secrets, then runs the binary named by TIMELEX_BINARY.
set -eu
if [ -f /keys/env ]; then
    # `set -a` exports every assignment the file makes; a plain source would keep them shell-local.
    set -a
    . /keys/env
    set +a
fi
exec "$TIMELEX_BINARY" "$@"
