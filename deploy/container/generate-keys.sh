#!/bin/sh
# Written by Kone Tshivhinda & Claude
# Local development only. Generates the gateway key pair, an operator key and the vault master key
# into a volume so no credential ever lives in the repository or a compose file. Idempotent.
set -eu
dir=${1:-/keys}
if [ -s "$dir/env" ]; then
    echo "keys already present in $dir; leaving them alone"
    exit 0
fi
umask 077
mkdir -p "$dir"
keygen="$dir/keygen.tmp"
envfile="$dir/env.tmp"
trap 'rm -f "$keygen" "$envfile"' EXIT

"$TIMELEX_BINARY" keygen >"$keygen"
grep -v '^#' "$keygen" >"$envfile"

operator_key=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
master_key=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
operator_hash=$(printf %s "$operator_key" | sha256sum | cut -d' ' -f1)

printf 'TIMELEX_OPERATOR_KEY_SHA256=%s\n' "$operator_hash" >>"$envfile"
printf 'TIMELEX_VAULT_MASTER_KEY=%s\n' "$master_key" >>"$envfile"
mv "$envfile" "$dir/env"
printf '%s\n' "$operator_key" >"$dir/operator-key"
chmod 0444 "$dir/env" "$dir/operator-key"
echo "wrote $dir/env and $dir/operator-key"
