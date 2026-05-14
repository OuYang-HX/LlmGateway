#!/bin/bash
set -euo pipefail

# Pre-check: does the project compile?
COMPILE_OUTPUT=$(cargo check 2>&1) || {
  echo "METRIC features_complete=0"
  echo "METRIC test_count=0"
  echo "FAIL: cargo check failed"
  echo "$COMPILE_OUTPUT" | tail -5
  exit 0
}

# Run tests, capture output
TEST_OUTPUT=$(cargo test 2>&1 || true)

# If cargo test itself failed to compile, report 0
if echo "$TEST_OUTPUT" | grep -q "could not compile"; then
  echo "METRIC features_complete=0"
  echo "METRIC test_count=0"
  echo "FAIL: compilation error"
  exit 0
fi

# Find the line with the most passed tests (integration tests line)
# Format: "test result: ok. 39 passed; 0 failed; ..."
PASSED=$(echo "$TEST_OUTPUT" | grep "passed" | grep -oP '\d+(?= passed)' | sort -rn | head -1 || echo "0")
FAILED=$(echo "$TEST_OUTPUT" | grep "passed" | grep -oP '\d+(?= failed)' | sort -rn | head -1 || echo "0")
TOTAL=$((PASSED + FAILED))

echo "METRIC features_complete=$PASSED"
echo "METRIC test_count=$TOTAL"

# Show summary for debugging
echo "--- Test Summary ---"
echo "Passed: $PASSED, Failed: $FAILED, Total: $TOTAL"
echo "$TEST_OUTPUT" | grep "test result:" | tail -5
