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

# Sum all "X passed" counts across all test suites
PASSED=$(echo "$TEST_OUTPUT" | grep -oP '\d+(?= passed)' | awk '{sum+=$1} END {print sum}')
FAILED=$(echo "$TEST_OUTPUT" | grep -oP '\d+(?= failed)' | awk '{sum+=$1} END {print sum}')
TOTAL=$((PASSED + FAILED))

echo "METRIC features_complete=$PASSED"
echo "METRIC test_count=$TOTAL"

# Show summary for debugging
echo "--- Test Summary ---"
echo "Passed: $PASSED, Failed: $FAILED, Total: $TOTAL"
echo "$TEST_OUTPUT" | grep "test result:" | tail -5
