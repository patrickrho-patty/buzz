#!/bin/sh
set -eu

python3 /tests/verify.py \
  --evidence /logs/artifacts/crew-evidence.json \
  --reward /logs/verifier/reward.json \
  --details /logs/verifier/details.json
