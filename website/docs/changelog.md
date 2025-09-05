---
sidebar_position: 99
title: Changelog
---

# Changelog

## [0.4.4] - TBD

Development version - changes will be documented at release.

## [0.4.3] - 2025-09-04

**Docker Volume Fix Release**

### 🐛 Bug Fixes
- **Docker Public Volume**: Fixed missing `raworc_public_data` volume creation and mounting in CLI Docker manager
- **CLI/Scripts Consistency**: CLI Docker configuration now mirrors scripts/start.sh behavior for volume management
- **Published Content Storage**: Public content server now has proper persistent volume mounting

### 🔧 Infrastructure Improvements
- **Volume Management**: Added `raworc_public_data` to volume creation list in CLI
- **Container Configuration**: Added `-v raworc_public_data:/public` mount to server container
- **Data Persistence**: Published agent content now survives container restarts

## [0.4.2] - 2025-09-04

**Docker Container Fix Release**

### 🐛 Bug Fixes
- **Docker Port Mapping**: Fixed public content server port 8000 not being exposed outside Docker container
- **External Access**: Public content server now properly accessible at `http://localhost:8000` for published agent content

### 🔧 Infrastructure Improvements  
- **Container Configuration**: Added missing `-p 8000:8000` port mapping to server container in CLI Docker manager
- **Published Content Access**: Users can now access published agent content externally as intended

## [0.4.1] - 2025-09-04

**Documentation & Content System Enhancement Release**

### 📚 Complete Documentation Overhaul
- **Agent Terminology Migration**: Comprehensive update from "session" to "agent" terminology across all documentation files
- **Website Build Fixes**: Resolved Docusaurus build errors and broken internal links throughout documentation site
- **CLI Reference Updates**: Updated all command examples and usage patterns to reflect agent-based architecture
- **Navigation Improvements**: Fixed sidebar configuration and document cross-references for seamless user experience

### 🎨 Agent Content Display System
- **Content Folder Instructions**: Enhanced agent system prompt to emphasize using `/agent/content/` folder for HTML display
- **Rich Content Support**: Added comprehensive guidance for creating images, maps, tables, reports, charts, games, and interactive apps
- **Example Content Page**: Created professional demonstration page showcasing 6 key content display categories with modern CSS
- **Content Workflow**: Established index.html as main entry point with relative linking best practices

### 🌐 Public Server Enhancement
- **Minimal Homepage**: Added clean 200 OK response for public server on port 8000
- **Professional Simplicity**: Streamlined public interface for better performance and user experience

### 🔧 Infrastructure Improvements
- **Terminal Width Adaptation**: Agent prompt separator lines now dynamically adapt to full terminal width
- **Version Consistency**: Updated remaining version references from 0.4.0 to 0.4.1 across all configuration files
- **Build System**: Enhanced release workflow with comprehensive file validation and lock file management

### 💡 User Experience Enhancements
- **Visual Content Creation**: Agents now guided to create rich HTML visualizations for better information display
- **Interactive Demonstrations**: Example content page shows practical applications of agent content capabilities
- **Simplified Public Interface**: Clean, minimal public server homepage reduces complexity

### 🏗️ Developer Experience
- **System Prompt Evolution**: Enhanced agent instructions for creating user-facing HTML content and visualizations
- **Documentation Quality**: Verified every page, command, and line across website for accuracy and consistency
- **Content Templates**: Provided comprehensive examples for various content display use cases

## v0.4.0

**Major Release: Complete Architecture Redesign & Enhanced Interactive Experience**

### 🚀 Major Architecture Changes
- **Complete Session → Agent Renaming**: System now uses "agents" throughout, more accurate terminology
- **Name-Based Primary Keys**: All operations use agent names directly, eliminating UUID complexity
- **Canvas → Content Renaming**: Canvas folders renamed to content/ for clearer purpose
- **Database Schema Overhaul**: Name as primary key, streamlined schema without container/volume IDs
- **Sleep/Wake Terminology**: Changed close/restore to sleep/wake for better user understanding

### ✨ New Interactive Agent Features
- **Interactive Session Commands**: Added `/sleep`, `/wake`, `/open`, `/publish` commands within agent sessions
- **Command Shortcuts**: Single-letter shortcuts (`/s`, `/w`, `/o`, `/p`) for faster interaction
- **Agent Content URLs**: New `raworc agent open` command shows private and public content links
- **Enhanced Help System**: Complete interactive command help with examples and shortcuts

### 🔧 Version Management Enhancements
- **Parameterized Pull Command**: `raworc pull [version]` supports specific version targeting
- **Flexible Bump Options**: Support for patch/minor/major version increments and specific versions
- **Smart Version Display**: Intelligent version formatting and CLI/Docker version coordination
- **Docker Image Versioning**: Version-specific Docker image pulling and management

### 🛠️ Infrastructure Improvements
- **Docker Volume Standardization**: Renamed volumes with `raworc_` prefix for consistency
- **Removed Unused Resources**: Eliminated unused `operator_data` volume and related configurations
- **Permission System Overhaul**: Fixed comprehensive Docker volume and directory permissions
- **Unpublish Functionality**: Fixed agent unpublishing to properly remove published content

### 🎨 User Experience Improvements
- **Dynamic Terminal Width**: Agent prompt separator lines adapt to full terminal width
- **Improved CLI Design**: Consistent command boxes and status displays across all operations
- **Better Error Handling**: Enhanced error messages and recovery suggestions
- **Agent State Management**: Clear visual indicators for agent states (idle, busy, sleeping, etc.)

### 🏗️ Developer Experience
- **Simplified Container Management**: Streamlined Docker operations and volume handling
- **Enhanced Documentation**: Updated CLI reference and API documentation
- **Improved Build System**: Better version management and release automation
- **Comprehensive Testing**: Improved error handling and edge case management

### ⚠️ Breaking Changes
- **Complete Data Reset Required**: Architecture changes require fresh installation
- **CLI Command Changes**: Some command structures updated for consistency
- **API Schema Changes**: Database and API schemas completely redesigned
- **Container Naming**: New naming convention for Docker containers and volumes

## v0.3.8

- **Canvas Always Included**: Removed canvas flag - canvas files are now always included in remix and publish operations by default
- **Simplified CLI**: Removed `--canvas` flag from remix and publish commands - canvas is always copied and always allowed
- **Backend Simplification**: Canvas permissions always set to true, no longer requiring explicit user choice
- **Text Editor Enhancement**: Empty file creation now supported when no content parameter provided
- **Documentation Updates**: Updated CLI usage examples and option tables to reflect canvas behavior changes

## v0.3.7

- **Canvas Folder Support**: Added comprehensive Canvas folder copying support for session remix and publish operations with granular control via `--canvas` flag
- **Canvas Permissions Management**: Implemented canvas-specific permissions for published sessions with selective remix access control
- **Text Editor Tool Robustness**: Enhanced text editor tool with support for both `file_text` and `content` parameters, improved error messages showing available parameters
- **Assistant Reasoning Display**: Added display of Claude's explanations and reasoning in CLI before tool execution for transparent interaction experience
- **Canvas Remix Workflow**: Canvas folders now copied selectively during remix operations following same patterns as data/code/secrets with permission enforcement
- **CLI Canvas Flags**: Added `--canvas` flag to remix and publish commands with default true value and clear permission status display
- **Enhanced Error Diagnostics**: Text editor tool now provides debug logging and detailed error messages for easier troubleshooting
- **Backend Canvas Integration**: Canvas task payload includes copy_canvas flag with proper permission checking and task processing
- **API Canvas Support**: RemixSessionRequest and PublishSessionRequest structures include canvas field with strict boolean validation
- **Docker Canvas Management**: Selective copy functions enhanced to handle Canvas folder operations with proper error handling

## v0.3.6

- **Canvas HTTP Server Optimization**: Moved Canvas port allocation from container creation to session creation, eliminating timing issues where Canvas URL was unavailable immediately after session start
- **Enhanced Tool Display Labels**: Updated CLI tool execution labels to be more descriptive ("Edit" → "Text Editor", "Run" → "Run Bash", "Search" → "Web Search") for better user experience
- **Immediate Canvas URL Display**: Canvas HTTP server URLs now display immediately in both host container logs and CLI session command box upon session creation
- **Canvas Workflow Improvements**: Added comprehensive Canvas folder workflow guidance with index.html as main entry point and relative URL linking best practices
- **Session API Enhancements**: Added canvas_port field to session API responses and implemented get_session endpoint in host client for real-time Canvas information
- **URL Hostname Resolution**: Enhanced Canvas URL generation to extract hostname from server configuration instead of hardcoding localhost, supporting remote deployments
- **Database Schema Updates**: Sessions table now includes canvas_port field populated during session creation for consistent Canvas port management
- **Container Integration Optimization**: Docker manager now fetches existing Canvas ports from session database instead of allocating new ports during container creation
- **Technical Reliability**: Added url crate dependency for proper URL parsing, cleaned up unused imports, and resolved build warnings for improved code quality

## v0.3.5

- **Website Documentation Overhaul**: Comprehensive update of all website documentation to reflect 100+ commits of CLI and feature improvements
- **CLI Interface Documentation**: Updated all interactive session examples with new geometric icon design system and visual state indicators
- **Tool Integration Documentation**: Added comprehensive documentation for bash and text_editor tools with visual execution examples
- **Session Management Updates**: Updated session state machine documentation to include all states (init/idle/busy/closed/deleted)
- **Interactive Command Reference**: Complete documentation of interactive session commands (/help, /status, /timeout, /name, /detach, /quit)
- **API Documentation Verification**: Updated version references and verified all endpoints match actual API implementation
- **Command Syntax Fixes**: Fixed troubleshooting commands and authentication flow examples throughout documentation
- **Visual Consistency**: Replaced all spinner-based examples with flat icon system for professional appearance
- **Session State Indicators**: Added visual state indicators (◯, ●, ◐, ◻, ◆) throughout session documentation

## v0.3.4

- **Session State Management**: Fixed session initialization to properly show 'init' state during container startup instead of prematurely showing 'idle'
- **Backend State Corrections**: Sessions now set to 'init' state during creation and restoration, transitioning to 'idle' only when host containers are actually ready
- **Prompt System Reliability**: Implemented comprehensive prompt manager to handle state monitoring, animation, and message processing consistently
- **Prompt Option Fixes**: Fixed `-p` prompt option to properly send messages to API before waiting for responses, resolving lost message issues
- **Timestamp-based Message Processing**: Implemented task creation timestamp tracking to prevent processing messages sent before operator task starts
- **Message Handler Improvements**: Enhanced message processing with proper timestamp filtering and environment variable passing between operator and host containers
- **CLI Animation Fixes**: Resolved duplicate prompt displays and animation conflicts between prompt processing and interactive session systems
- **Session Timeout Increase**: Increased default session auto-close timeout from 60 seconds to 5 minutes (300 seconds) for better user experience
- **Interactive Session Enhancements**: Unified message handling between prompt and interactive modes with comprehensive tool execution display
- **Error Handling Improvements**: Enhanced session close error handling to gracefully ignore API calls for already closed containers
- **Prompt Display Cleanup**: Eliminated extra blank lines and duplicate prompts in CLI output for cleaner user interface
- **State Transition Accuracy**: Sessions now accurately reflect container readiness state throughout initialization and restoration processes

## v0.3.3

- **CLI Design System**: Implemented consistent flat geometric icon system throughout CLI interface, replacing emojis with professional Unicode characters
- **Session Interface Overhaul**: Major redesign of session interface with clean formatting, visual state indicators, and improved user experience
- **Session Detach Functionality**: Added `/detach` and `/d` commands to detach from sessions while keeping them running in background
- **Session Name Resolution**: Enhanced session name handling with alphanumeric validation and automatic ID resolution
- **Markdown Formatting**: Integrated marked-terminal for proper markdown rendering in CLI session output
- **Visual State Indicators**: Added geometric shape indicators for session states (idle, busy, init, closed, deleted)
- **Conversation History**: Implemented clean conversation history display with user input prefixes and improved readability
- **Command Box Layout**: Standardized command box format across all CLI operations with consistent server/user context
- **Session Commands**: Added comprehensive session management commands (`/help`, `/status`, `/timeout`, `/name`, `/quit`)
- **Spinner Removal**: Replaced all loading spinners with immediate feedback for better terminal compatibility
- **Tool Integration**: Enhanced bash tool implementation with proper Anthropic specification compliance
- **Web Search Integration**: Added Anthropic web search tool for enhanced session capabilities
- **Text Editor Tool**: Implemented comprehensive text editor tool with latest Anthropic specifications
- **State Management**: Improved session state handling with real-time monitoring and better error recovery
- **Interface Cleanup**: Removed redundant messages, status displays, and visual clutter for minimalist experience
- **Prompt Standardization**: Unified prompt display formatting across all session interaction points
- **Security Enhancements**: Reduced excessive guardrails while maintaining essential system security
- **Performance Improvements**: Optimized session monitoring and message handling for better responsiveness

## v0.3.2

- **API Input Validation**: Added comprehensive input validation with strict type checking and clear error messages
- **Boolean Parameter Validation**: Session remix/publish parameters now reject non-boolean values (strings, numbers) 
- **Message Role Validation**: Message roles restricted to valid values (user, host, system) with automatic default to "user"
- **Numeric Parameter Validation**: Query parameters (limit/offset) and session timeouts validated with range checks
- **Session Performance Optimization**: Eliminated 10-second startup delay by adding RAWORC_HAS_SETUP environment hint
- **CLI Session Management**: Added `raworc session close <session-name>` command with state checking and user feedback
- **Admin Security Enhancement**: Restricted token creation API to admin users only with explicit role checking
- **Query Parameter Fixes**: Resolved deserialization issues while maintaining type safety for optional parameters
- **Enhanced Error Messages**: All validation errors now provide specific, actionable error messages
- **Setup Script Optimization**: Reduced setup script wait from 10s to 2s when expected, skip entirely when not needed

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
## v0.4.4

- Model Runtime Migration: Replaced Anthropic/Claude integration with local model inference via Ollama (default model: gpt-oss)
- New `ollama` component: `./scripts/start.sh ollama` runs dedicated `raworc_ollama` container with persistent model cache
- Resource flags: Added `OLLAMA_CPUS`, `OLLAMA_MEMORY`, `OLLAMA_SHM_SIZE`, and `OLLAMA_ENABLE_GPU` to tune Ollama container resources
- CLI updates: Removed Anthropic key validation; added `ollama` service mapping; updated examples to use `OLLAMA_HOST`
- Docs: Updated Getting Started, CLI Usage, Architecture, and Agents guides to reflect Ollama-based runtime
