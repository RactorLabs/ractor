# /commit - Comprehensive Change Review and Commit

## Description

Review all current changes and create a comprehensive commit with proper summary.

## Execution Steps

1. Run `git status` to see all changed files
2. Run `git diff` to understand all modifications  
3. Run `git log --oneline -5` to check recent commit history for context and style
4. Stage all changes with `git add .`
5. Create commit with comprehensive message summarizing all changes
6. Run `git status` to verify commit succeeded

## Commit Message Guidelines

- Use conventional commit format: `type: brief description`
- Include detailed body explaining what changed and why
- NO mention of AI assistants, automation, or generated content
- Write as human developer would write
- Follow existing project commit style from recent history

