---
sidebar_position: 99
title: Changelog
---

# Changelog

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

