#!/usr/bin/env sh
set -eu

NARGO_VERSION="nargo version = 1.0.0-beta.9"
BB_VERSION="v0.87.0"
CIRCUIT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../circuits/anonymous-ticket-claim" && pwd)"

if [ "$(nargo --version | sed -n '1p')" != "$NARGO_VERSION" ]; then
  echo "expected $NARGO_VERSION" >&2
  exit 1
fi

if [ "$(bb --version)" != "$BB_VERSION" ]; then
  echo "expected bb $BB_VERSION" >&2
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
