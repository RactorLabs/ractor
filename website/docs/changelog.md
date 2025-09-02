---
sidebar_position: 99
title: Changelog
---

# Changelog

## v0.3.1

- **CLI ANTHROPIC_API_KEY Validation**: Added validation for required ANTHROPIC_API_KEY environment variable in `raworc start` command
- **Operator Container Fix**: Fixed CLI to properly pass ANTHROPIC_API_KEY to operator container, resolving startup failures
- **CLI Help Consistency**: Fixed inconsistent help messages across all CLI commands, replacing incorrect `raworc auth login` references with `raworc login`
- **API Endpoint Fix**: Corrected help text to reference correct `raworc api version` endpoint instead of non-existent `health` endpoint
- **User Experience**: Enhanced error messages with clear instructions for setting up ANTHROPIC_API_KEY

## v0.3.0

- **Session-Based Architecture**: Simplified to session-based system with Host as Raworc's Computer Use Agent implementation
- **ANTHROPIC_API_KEY Required**: All new sessions now require ANTHROPIC_API_KEY for Host functionality  
- **Host Nomenclature**: Updated terminology - Host is Raworc's Computer Use Agent, CUA abbreviation removed
- **Selective Session Remix**: Enhanced remix functionality with selective copying of data and code files
- **Improved Session Restore**: Reliable session persistence with no message reprocessing
- **Documentation Overhaul**: Comprehensive CLI command syntax fixes throughout all documentation
- **Session Command Corrections**: Fixed incorrect session commands (removed `raworc session start start` patterns)
- **Environment Variable Migration**: Moved ANTHROPIC_API_KEY from secrets to environment variable prerequisite
- **Session Management Guide**: Added comprehensive session names and publishing documentation
- **Authentication Simplification**: Streamlined RBAC documentation to focus on Operators vs Users
- **Dev Mode Rewrite**: Removed "Coding Agent" terminology, focused on `/session/code` folder access
- **Session Playground Fixes**: Updated all examples to use correct CLI syntax and environment variables
- **Command Reference Updates**: Synchronized CLI documentation with actual implementation
- **Security Improvements**: Removed inline API key examples, promoted environment variable usage
- **Homepage Styling**: Changed white backgrounds to black for visual consistency

## v0.2.10

- **Documentation Reorganization**: Restructured session documentation for better clarity
- **Terminology Updates**: Improved consistency in session management documentation
- **Developer Experience**: Enhanced internal documentation and workflow clarity

## v0.2.9

- **Enhanced CLI Reset**: Consolidated comprehensive reset functionality from shell script into CLI
- **8-Step Cleanup Process**: Added systematic Docker cleanup with progress indicators
- **Image Removal**: CLI reset now removes all Raworc images including session images
- **Volume Management**: Comprehensive Docker volume cleanup with graceful fallbacks
- **Session Remix**: Added remix functionality to create new sessions from existing ones
- **Documentation Overhaul**: Updated README, development guides, and release workflows
- **Simplified Workflow**: Removed redundant shell scripts in favor of unified CLI approach
- **Improved Error Handling**: Better error recovery in reset operations with detailed logging

## v0.2.8

- **Critical Fix**: Resolved message loop reliability issues preventing second messages from processing
- **Session Restore**: Enhanced restore functionality with proper message tracking
- **Agent Delegation**: Fixed hanging agent execution that blocked message processing
- **Polling Improvements**: Simplified message detection logic for better reliability
- **CLI Constants**: Added proper state and role constants for consistent behavior
- **Error Recovery**: Improved polling loop robustness with continued operation on errors
- **Debug Cleanup**: Removed temporary debug logging for production readiness

## v0.2.7

- Fixed session restore functionality to prevent message reprocessing
- Improved CLI session startup performance after restore
- Enhanced message polling logic for better reliability
- Added constants for consistent state and role management
- Updated API version response to reflect current version

## v0.1.1

- Published npm package to simplify developer experience
- Version bump to 0.1.1

## v0.1.0

- Initial public docs and REST API.
- Sessions, spaces, secrets, agents, operator, MySQL.

