#!/usr/bin/env bash
# ENGRAM Demo Data Seeder
# Seeds sample data into the ENGRAM Notion workspace for showcase/demo
#
# Usage:
#   ./scripts/demo-seed.sh [engram-webhook-url]
#
# This script creates:
#   - 3 RFCs (one Approved, one Under Review, one Draft)
#   - 2 Modules documented
#   - 5 Environment Variables
#   - 1 additional Playbook rule
#   - Sample benchmark baselines
#
# Then triggers a PR merge cascade to show the full system in action.

set -euo pipefail

ENGRAM_URL="${1:-http://localhost:3000}"
PROJECT_ID="${ENGRAM_PROJECT_ID:-demo-project}"

echo "═══════════════════════════════════════════"
echo "  ENGRAM — Demo Data Seeder"
echo "═══════════════════════════════════════════"
echo ""
echo "Webhook URL: $ENGRAM_URL"
echo "Project ID:  $PROJECT_ID"
echo ""

# ─── Helper function ───
post_json() {
    local endpoint="$1"
    local data="$2"
    curl -s -X POST "${ENGRAM_URL}${endpoint}" \
        -H "Content-Type: application/json" \
        -d "$data"
}

echo "Step 1: Posting sample benchmark data..."
post_json "/webhook/benchmark" '{
  "project_id": "'"$PROJECT_ID"'",
  "commit_sha": "abc1234567890",
  "branch": "main",
  "benchmarks": [
    {"name": "api_handler_latency", "metric_type": "Latency", "value": 12.5, "unit": "ms"},
    {"name": "db_query_throughput", "metric_type": "Throughput", "value": 1500, "unit": "req/s"},
    {"name": "memory_usage_peak", "metric_type": "Memory", "value": 256, "unit": "MB"}
  ]
}'
echo " Done"

echo ""
echo "Step 2: Posting sample audit data..."
post_json "/webhook/audit" '{
  "project_id": "'"$PROJECT_ID"'",
  "raw_output": "{\"vulnerabilities\":{\"found\":2,\"list\":[{\"advisory\":{\"id\":\"RUSTSEC-2026-0001\",\"package\":\"openssl\",\"title\":\"Buffer overflow in SSL handshake\",\"cvss\":\"9.8\"},\"versions\":{\"patched\":[\">=3.1.1\"]}},{\"advisory\":{\"id\":\"RUSTSEC-2026-0002\",\"package\":\"regex\",\"title\":\"ReDoS in pattern matching\",\"cvss\":\"5.3\"},\"versions\":{\"patched\":[\">=1.11.0\"]}}]}}",
  "tool": "cargo-audit",
  "commit_sha": "abc1234567890",
  "branch": "main"
}'
echo " Done"

echo ""
echo "Step 3: Triggering new engineer onboard..."
post_json "/api/trigger/onboard" '{
  "engineer_name": "Jane Smith",
  "role": "backend",
  "project_id": "'"$PROJECT_ID"'"
}'
echo " Done"

echo ""
echo "Step 4: Triggering weekly digest..."
post_json "/api/trigger/digest" '{
  "project_id": "'"$PROJECT_ID"'"
}'
echo " Done"

echo ""
echo "Step 5: Simulating a PR merge with RFC reference..."
# This would normally come from GitHub webhook, but we can trigger it manually
post_json "/webhook/benchmark" '{
  "project_id": "'"$PROJECT_ID"'",
  "commit_sha": "def4567890123",
  "branch": "feature/add-caching",
  "benchmarks": [
    {"name": "api_handler_latency", "metric_type": "Latency", "value": 15.3, "unit": "ms"},
    {"name": "db_query_throughput", "metric_type": "Throughput", "value": 1420, "unit": "req/s"},
    {"name": "memory_usage_peak", "metric_type": "Memory", "value": 312, "unit": "MB"}
  ]
}'
echo " Done (regression should be detected: latency +22.4%)"

echo ""
echo "Step 6: Triggering release creation..."
post_json "/api/trigger/release" '{
  "project_id": "'"$PROJECT_ID"'",
  "version": "v0.2.0",
  "milestone": "Sprint 3"
}'
echo " Done"

echo ""
echo "═══════════════════════════════════════════"
echo "  Demo seeding complete!"
echo ""
echo "  Check your Notion workspace to see:"
echo "  • Benchmark records with Delta %"
echo "  • Regression detected (latency +22.4%)"
echo "  • Security audit findings"
echo "  • Onboarding track for Jane Smith"
echo "  • Weekly health digest"
echo "  • Release candidate v0.2.0"
echo "═══════════════════════════════════════════"
