# OpenAI Harmony Format Implementation Plan

## Overview
Implement full OpenAI Harmony format support to fix GPT-OSS tool calling issues and enable proper multi-channel communication (analysis, commentary, final).

## Current Issues Addressed
- Tool call JSON parsing failures in Ollama (500 errors)
- GPT-OSS models require harmony format for proper functionality  
- Multi-channel responses not properly handled
- Need separation of reasoning, tool calls, and final responses

## Implementation Phases

### Phase 1: Dependencies and Basic Setup
**Goal**: Add harmony library and basic integration structure

#### Step 1.1: Add Dependencies
```toml
[dependencies]
openai-harmony = { git = "https://github.com/openai/harmony" }
anyhow = "1.0"  # For error handling compatibility
```

#### Step 1.2: Create Harmony Client Wrapper  
- New file: `src/agent/harmony_client.rs`
- Wrapper around `openai-harmony` for our use case
- Basic conversation rendering and parsing

#### Step 1.3: Integration Points
- Modify `OllamaClient` to optionally use harmony format
- Add feature flag for gradual rollout
- Maintain backward compatibility

### Phase 2: Message Format Conversion
**Goal**: Convert between standard chat format and harmony tokens

#### Step 2.1: Message Conversion Layer
```rust
// Convert our ChatMessage to Harmony Message
impl From<ChatMessage> for harmony::Message {
    fn from(chat_msg: ChatMessage) -> Self {
        // Implementation with proper role/content mapping
    }
}

// Convert Harmony back to our format, filtering by channel
impl TryFrom<harmony::Message> for ChatMessage {
    fn try_from(harmony_msg: harmony::Message) -> Result<Self> {
        // Extract only "final" channel content for standard workflow
    }
}
```

#### Step 2.2: Channel Handler
```rust
pub struct ChannelHandler {
    analysis: Vec<String>,    // Chain of thought
    commentary: Vec<String>,  // Tool call explanations  
    final_content: String,    // User-facing response
}
```

### Phase 3: Tool Calling Integration
**Goal**: Implement harmony-compatible tool calling

#### Step 3.1: Developer Message Creation
- System prompt → System Message (reasoning effort, etc.)
- Tool definitions → Developer Message (tool namespace)
- Clear separation of concerns

#### Step 3.2: Tool Call Processing
```rust
// Parse commentary channel for tool calls
// Format: recipient="functions.tool_name", content=JSON params
pub fn parse_commentary_tool_calls(commentary_msgs: &[Message]) -> Vec<ToolCall> {
    // Implementation to extract tool calls from commentary
}
```

#### Step 3.3: Tool Result Integration
- Tool results go back as Tool role messages
- Proper recipient/namespace handling
- Continuation of conversation flow

### Phase 4: Ollama Integration
**Goal**: Use harmony tokens with Ollama instead of JSON chat

#### Step 4.1: Token-based Communication
```rust
// Instead of ChatRequest JSON:
let tokens = harmony.render_conversation_for_completion(&conversation, Role::Assistant)?;
let ollama_request = GenerateRequest {
    model: "gpt-oss:20b",
    prompt_tokens: tokens,  // Send tokens instead of JSON messages
    options: RequestOptions { num_ctx: 131072 },
};
```

#### Step 4.2: Response Parsing
```rust
// Parse Ollama token response back to structured messages
let response_tokens = ollama_response.tokens;
let messages = harmony.parse_messages_from_completion_tokens(response_tokens)?;
```

### Phase 5: Multi-Channel Response Handling
**Goal**: Properly handle analysis, commentary, and final channels

#### Step 5.1: Response Separation
```rust
pub struct HarmonyResponse {
    pub final_content: String,        // User-visible response
    pub thinking: Option<String>,     // Analysis channel content
    pub tool_calls: Vec<ToolCall>,    // Extracted from commentary
    pub metadata: serde_json::Value,  // Channel timing, etc.
}
```

#### Step 5.2: UI Integration
- Analysis channel → thinking metadata for operator UI
- Commentary channel → tool call processing
- Final channel → main response content

### Phase 6: Configuration and Optimization
**Goal**: Performance optimization and configuration options

#### Step 6.1: Environment Configuration
```bash
HARMONY_MODE=enabled          # Toggle harmony vs standard format
HARMONY_CHANNELS=all         # Which channels to capture
HARMONY_REASONING_EFFORT=high # Default reasoning effort
```

#### Step 6.2: Performance Optimizations
- Token caching for repeated conversations
- Streaming parser for real-time responses
- Memory management for large token sequences

## Implementation Strategy

### Gradual Rollout Approach
1. **Parallel implementation** - Keep existing chat API working
2. **Feature flag control** - `ENABLE_HARMONY_FORMAT=true`
3. **Agent-by-agent testing** - Test with specific agents first
4. **Performance monitoring** - Compare response times and accuracy

### Backward Compatibility Strategy
```rust
pub enum ChatMode {
    Standard,   // Current OpenAI JSON format
    Harmony,    // New harmony token format
}

impl OllamaClient {
    pub async fn complete_with_mode(
        &self,
        messages: Vec<ChatMessage>,
        mode: ChatMode,
    ) -> Result<ModelResponse> {
        match mode {
            ChatMode::Standard => self.complete_legacy(messages).await,
            ChatMode::Harmony => self.complete_harmony(messages).await,
        }
    }
}
```

### Testing Strategy
1. **Unit tests** for each conversion function
2. **Integration tests** with real GPT-OSS model responses
3. **Performance benchmarks** comparing formats
4. **Tool calling accuracy** tests

## Expected Benefits

### Tool Calling Fixes
- ✅ No more JSON parsing errors in tool calls
- ✅ Proper tool call format that GPT-OSS expects
- ✅ Better tool execution flow

### Enhanced Capabilities  
- ✅ Chain-of-thought reasoning visible in UI
- ✅ Tool call explanations and context
- ✅ Proper separation of thinking vs. final responses
- ✅ Better structured conversation flow

### Performance Improvements
- ✅ Faster parsing with Rust-based harmony library
- ✅ More efficient token handling
- ✅ Reduced JSON serialization overhead

## Implementation Milestones

- [ ] **Milestone 1**: Dependencies added, basic harmony client created
- [ ] **Milestone 2**: Message conversion working, tests passing
- [ ] **Milestone 3**: Tool calling functional with harmony format
- [ ] **Milestone 4**: Multi-channel responses properly handled
- [ ] **Milestone 5**: Performance optimized, production ready

## Risk Mitigation

### Rollback Plan
- Keep existing code intact with feature flags
- Easy switch back to standard format if issues arise
- Gradual agent migration strategy

### Monitoring Plan  
- Track response times and accuracy
- Monitor tool calling success rates
- Watch for new error patterns

## Success Criteria
- [ ] Tool calling works without 500 errors
- [ ] GPT-OSS models show proper chain-of-thought
- [ ] Operator UI displays thinking content correctly
- [ ] No regression in existing functionality
- [ ] Performance is equal or better than current implementation