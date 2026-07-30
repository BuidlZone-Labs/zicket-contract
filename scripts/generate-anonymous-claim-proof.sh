#!/usr/bin/env sh
set -eu

NARGO_VERSION="1.0.0-beta.9"
BB_VERSION="0.87.0"
CIRCUIT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../circuits/anonymous-ticket-claim" && pwd)"

extract_version() {
  "$1" --version | awk '{
    for (i = 1; i <= NF; i++) {
      version = $i
      sub(/^v/, "", version)
      if (version ~ /^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$/) {
        print version
        exit
      }
    }
  }'
}

if [ "$(extract_version nargo)" != "$NARGO_VERSION" ]; then
  echo "expected nargo version = $NARGO_VERSION" >&2
  exit 1
fi

if [ "$(extract_version bb)" != "$BB_VERSION" ]; then
  echo "expected bb v$BB_VERSION" >&2
  exit 1
fi

cd "$CIRCUIT_DIR"
nargo execute
bb prove \
  -b target/anonymous_ticket_claim.json \
  -w target/anonymous_ticket_claim.gz \
  -o target/zk \
  --scheme ultra_honk \
  --oracle_hash keccak \
  --zk \
  --write_vk
