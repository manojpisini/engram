#!/usr/bin/env bash
# ENGRAM — Clean Demo Seed Data
# Archives (soft-deletes) all demo/sample data from Notion databases.
#
# Usage:
#   ./scripts/clean-seed.sh [engram-url]
#
# What it does:
#   Queries each ENGRAM database for pages, then archives them via the Notion API.
#   Uses the ENGRAM backend proxy endpoints so you only need the server running.
#
# ⚠ WARNING: This archives ALL pages in every ENGRAM database.
#   Only use this to reset a demo workspace.

set -euo pipefail

ENGRAM_URL="${1:-http://localhost:3000}"

echo "═══════════════════════════════════════════"
echo "  ENGRAM — Clean Demo Seed Data"
echo "═══════════════════════════════════════════"
echo ""
echo "Server: $ENGRAM_URL"
echo ""
echo "⚠  This will archive ALL pages in every ENGRAM database."
read -rp "Continue? [y/N] " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi
echo ""

# Database IDs from engram.toml
declare -A DATABASES=(
    [projects]="c302f782-202f-4914-83b3-89a66e199cbd"
    [rfcs]="e33f0b1c-983b-4811-8202-fc8a53883a08"
    [rfc_comments]="982f155d-3ecf-4ef8-857a-03b08de795e2"
    [benchmarks]="e5f03cb7-231d-4d2f-b444-5fec0953c56e"
    [regressions]="4e3894d1-5d19-4597-a187-db5eafff9345"
    [performance_baselines]="6242af62-7d3c-4a71-8534-e4126bb96158"
    [dependencies]="70d1fbf5-f3f2-4fdd-8165-d82bf94ead6e"
    [audit_runs]="4ac241a1-9c76-43bb-896e-86ab8e79cceb"
    [modules]="87131a15-bf9b-479f-a5c6-25b6c7983cd0"
    [onboarding_tracks]="f8dedad3-10c0-4045-aff8-506201545c6f"
    [onboarding_steps]="54faa12c-2335-4b6a-a8a3-fb40ef08b824"
    [knowledge_gaps]="11defa8b-7daa-4d90-bca1-6dcc37ff6645"
    [env_config]="a96fef42-434e-43a1-9a25-2f6ec30f2093"
    [config_snapshots]="6daa0944-cdc1-49e0-a816-16732af78930"
    [secret_rotation_log]="82842f28-1b67-40a9-b69d-639dfd684e9c"
    [pr_reviews]="7e6a780f-75ca-47c7-8ca2-b16638ad9297"
    [review_playbook]="e1831604-5046-4fc0-a493-c33780aa4614"
    [review_patterns]="6a1fb341-847b-4616-8199-41cc762e1ca0"
    [tech_debt]="71a25e03-775e-4fd3-b11c-4c7374fb1f8b"
    [health_reports]="ffc26d3e-3852-4bb5-876e-064c42b05112"
    [engineering_digest]="5a91dd49-a7ea-471c-abdd-e8b2ee5c396d"
    [events]="cda3d30b-5831-4d37-84ab-5d0802ab484e"
    [releases]="d69e2cfd-b7e2-4efe-9c6d-4b239ac7f344"
)

# Read Notion token from engram.toml or env
NOTION_TOKEN="${NOTION_MCP_TOKEN:-}"
if [ -z "$NOTION_TOKEN" ]; then
    # Try to extract from engram.toml
    TOML_FILE="$(dirname "$0")/../engram.toml"
    if [ -f "$TOML_FILE" ]; then
        NOTION_TOKEN=$(grep 'notion_mcp_token' "$TOML_FILE" | sed 's/.*= *"//' | sed 's/".*//')
    fi
fi

if [ -z "$NOTION_TOKEN" ]; then
    echo "ERROR: No Notion token found. Set NOTION_MCP_TOKEN or check engram.toml"
    exit 1
fi

NOTION_API="https://api.notion.com/v1"
total_archived=0

archive_page() {
    local page_id="$1"
    curl -s -X PATCH "${NOTION_API}/pages/${page_id}" \
        -H "Authorization: Bearer ${NOTION_TOKEN}" \
        -H "Notion-Version: 2022-06-28" \
        -H "Content-Type: application/json" \
        -d '{"archived": true}' > /dev/null 2>&1
}

clean_database() {
    local name="$1"
    local db_id="$2"
    local count=0

    # Query all pages in this database
    local response
    response=$(curl -s -X POST "${NOTION_API}/databases/${db_id}/query" \
        -H "Authorization: Bearer ${NOTION_TOKEN}" \
        -H "Notion-Version: 2022-06-28" \
        -H "Content-Type: application/json" \
        -d '{"page_size": 100}')

    # Extract page IDs and archive each
    local page_ids
    page_ids=$(echo "$response" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for r in data.get('results', []):
        print(r['id'])
except: pass
" 2>/dev/null || true)

    if [ -z "$page_ids" ]; then
        printf "  %-25s → 0 pages (empty)\n" "$name"
        return
    fi

    while IFS= read -r pid; do
        if [ -n "$pid" ]; then
            archive_page "$pid"
            count=$((count + 1))
        fi
    done <<< "$page_ids"

    total_archived=$((total_archived + count))
    printf "  %-25s → %d pages archived\n" "$name" "$count"
}

echo "Cleaning databases..."
echo ""

for name in "${!DATABASES[@]}"; do
    clean_database "$name" "${DATABASES[$name]}"
done

echo ""
echo "═══════════════════════════════════════════"
echo "  Cleanup complete!"
echo "  Total pages archived: $total_archived"
echo ""
echo "  Archived pages can be restored from"
echo "  Notion's trash within 30 days."
echo "═══════════════════════════════════════════"
