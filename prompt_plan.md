# Prompt Migration Plan

## Goals
- Replace JSON-based tool definitions with inline XML command descriptions embedded directly in the system prompt (`src/sandbox/task_handler.rs` → prompt builder, align with `sample_prompt.txt` patterns).
- Remove runtime tool registry/alias resolution (`src/sandbox/tool_registry.rs`, async registration in `TaskHandler::new`); commands will be defined statically in the prompt text and executed via a simple match.
- Enforce XML-only outputs from the model and reject any response that does not return a supported command tag (`src/sandbox/inference.rs` parser, `TaskHandler` retry loop).
- Retire planner concepts (update_plan tool, `/sandbox/plan.md` management) across `src/sandbox/builtin_tools.rs`, `src/sandbox/tools.rs`, and `src/sandbox/task_handler.rs`.

## Tasks
1. ✅ Inventory the concrete commands we must keep (shell, text editing, file search, output) and define XML schemas for each.
2. ✅ Update `TaskHandler::build_system_prompt` to embed the XML command catalog and require single-element replies.
3. ✅ Rework `InferenceClient` to drop JSON tool attachments and parse XML responses.
4. ✅ Replace `ToolRegistry` with the static `ToolCatalog` executor.
5. ✅ Remove planner tooling (`update_plan`, `/sandbox/plan.md` access) and associated guidance.
6. ✅ Preserve task telemetry by emitting JSON tool_call/tool_result segments derived from XML invocations.
7. ☐ Run `cargo fmt && cargo clippy && cargo test`, plus a manual sandbox smoke-test of the XML loop.

## Open Questions
- How should we encode free-form textual replies (e.g., `<output message=\"...\"/>` vs. `<![CDATA[...]]>` inner text) while avoiding quoting pitfalls?
- Do we maintain backward compatibility for existing logs (`/sandbox/logs/inference_*`) and Operator tooling that expect JSON bodies?
- Should we keep a fallback for legacy tasks that still send JSON instructions, or fail fast with a guidance note?
