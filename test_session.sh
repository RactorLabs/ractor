#\!/bin/bash
export ANTHROPIC_API_KEY=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

# Test sending a message via API to trigger tool execution
curl -X POST http://localhost:9000/api/v0/sessions/d7e4a9c4-a4fe-4ac7-9c67-01373bf35c44/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $(raworc auth | grep -o "Token: [^[:space:]]*" | cut -d" " -f2 2>/dev/null || echo "test")" \
  -d "{\"content\": \"list files in current directory\", \"role\": \"user\"}" \
  -s | jq .

echo "Message sent\! Checking for responses..."
sleep 3

# Check for host responses
curl -X GET "http://localhost:9000/api/v0/sessions/d7e4a9c4-a4fe-4ac7-9c67-01373bf35c44/messages" \
  -H "Authorization: Bearer $(raworc auth | grep -o "Token: [^[:space:]]*" | cut -d" " -f2 2>/dev/null || echo "test")" \
  -s | jq ".[-2:]"

