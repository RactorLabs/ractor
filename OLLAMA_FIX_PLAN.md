# Ollama Client Fix Plan

## Issues Identified
1. Missing Tool Result Handling
2. Incomplete Tool Flow 
3. Tool Message Role Handling Issues
4. Missing Tool Call ID Management
5. Static vs Dynamic Tool Definitions Inconsistency
6. Environment Variable Validation Missing
7. Response Format Issues (Harmony format)

## Fix Plan (Sequential Implementation)

### Phase 1: Core Tool Calling Infrastructure
**Commit 1: Add tool call ID management**
- Add `id` field to `ToolCall` struct
- Update serialization/deserialization to handle tool_call_id
- Ensure compatibility with OpenAI tool calling format

**Commit 2: Add tool result handling structures**
- Create `ToolResult` struct with proper fields
- Add methods to convert tool results to chat messages
- Update `ChatMessage` to support tool_call_id correlation

### Phase 2: Complete Tool Calling Cycle  
**Commit 3: Implement tool execution loop**
- Add method to execute tool calls and collect results
- Implement conversation continuation with tool results
- Add proper error handling for tool execution failures

**Commit 4: Fix tool message role handling**
- Remove placeholder "[tool output]" logic
- Implement proper tool result message formatting
- Ensure tool messages contain actual execution results

### Phase 3: Response Format & Compatibility
**Commit 5: Add harmony response format support**
- Research and implement proper harmony format handling
- Ensure compatibility with GPT-OSS thinking/reasoning output
- Add proper parsing for chain-of-thought responses

**Commit 6: Environment variable validation**
- Add validation for `OLLAMA_REASONING_EFFORT` (low/medium/high)
- Add input validation and error handling
- Maintain existing default values (no changes to defaults)

### Phase 4: Code Organization
**Commit 7: Clean up tool definitions**
- Remove static tool definitions fallback
- Rely entirely on dynamic tool registry
- Ensure backward compatibility if needed

**Commit 8: Integration testing and validation**
- Add comprehensive tests for tool calling flow
- Validate against GPT-OSS models
- Document the new tool calling behavior

## Implementation Notes

### Tool Calling Flow (New)
```
1. Model sends assistant message with tool_calls
2. Execute each tool call via registry
3. Create tool role messages with results and tool_call_id
4. Add tool messages to conversation history
5. Send updated conversation back to model
6. Repeat until model provides final response (no tool_calls)
```

### Key Data Structure Changes
```rust
// Enhanced ToolCall with ID
pub struct ToolCall {
    pub id: String,          // NEW: for correlation
    pub function: ToolCallFunction,
}

// New ToolResult structure
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub error: Option<String>,
}

// Enhanced ChatMessage for tool correlation
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,  // NEW: for tool result correlation
}
```

### Validation Rules
- `OLLAMA_REASONING_EFFORT`: must be one of "low", "medium", "high"
- Tool execution timeouts and error handling
- Proper JSON schema validation for tool parameters

## Success Criteria
- [ ] Complete tool calling cycle works end-to-end
- [ ] Tool results properly correlated with tool_call_ids  
- [ ] Harmony response format properly parsed
- [ ] Environment variables validated with helpful error messages
- [ ] No breaking changes to existing non-tool functionality
- [ ] Backward compatibility maintained where possible

## Testing Strategy
- Unit tests for each new structure and method
- Integration tests with actual GPT-OSS models
- Error handling tests for malformed responses
- Performance tests for tool execution loops