#!/bin/sh
set -eu

python3 /tests/verify.py \
  --evidence /logs/artifacts/crew-evidence.json \
  --skill-file /home/crew/.claude/skills/context-health-check/SKILL.md \
  --reward /logs/verifier/reward.json \
  --details /logs/verifier/details.json
