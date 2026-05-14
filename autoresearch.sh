#!/bin/bash
set -euo pipefail

# Pre-check: does the project compile?
if ! cargo check 2>/dev/null; then
  echo "METRIC features_complete=0"
  echo "METRIC test_count=0"
  echo "FAIL: cargo check failed"
  exit 0
fi

# Run tests, capture output
TEST_OUTPUT=$(cargo test 2>&1 || true)

# Count total tests and passed tests
TOTAL=$(echo "$TEST_OUTPUT" | grep -oP '\d+ passed' | grep -oP '\d+' || echo "0")
FAILED=$(echo "$TEST_OUTPUT" | grep -oP '\d+ failed' | grep -oP '\d+' || echo "0")
PASSED=$((TOTAL))

# If cargo test itself failed to run, report 0
if echo "$TEST_OUTPUT" | grep -q "error\["; then
  PASSED=0
  TOTAL=0
fi

echo "METRIC features_complete=$PASSED"
echo "METRIC test_count=$TOTAL"

# Show summary for debugging
echo "--- Test Summary ---"
echo "$TEST_OUTPUT" | tail -20
