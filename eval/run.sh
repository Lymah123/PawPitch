#!/usr/bin/env bash
# Runs the sample creator against every fictional brand in examples/brands and reports
# whether a draft passed the fact check, and on which attempt. Raw output goes to eval/out/.
cd "$(dirname "$0")/.." || exit 1
mkdir -p eval/out
pass=0; total=0
for f in examples/brands/*.txt; do
  name=$(basename "$f" .txt | tr '-' ' ')
  total=$((total+1))
  out=$(cargo run -q -- pitch --profile examples/sample-profile.json --brand "$name" --brand-file "$f" --tries 5 2>&1)
  echo "$out" > "eval/out/$(basename "$f")"
  if echo "$out" | grep -q "=== DRAFT"; then
    n=$(echo "$out" | grep -o 'attempt [0-9]*/' | tail -n 1 | grep -o '[0-9]*')
    pass=$((pass+1)); echo "PASS  $name (attempt $n)"
  else
    echo "FAIL  $name"
  fi
done
echo "$pass/$total passed"
