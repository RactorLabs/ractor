const chalk = require('chalk');

// Force chalk to use colors in terminal
chalk.level = 1;
const inquirer = require('inquirer');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');
const display = require('../lib/display');
const { marked } = require('marked');
const TerminalRenderer = require('marked-terminal').default;

// Configure marked-terminal
marked.setOptions({
  renderer: new TerminalRenderer({
    blockquote: chalk.gray.italic,
    code: chalk.yellow,
    codespan: chalk.cyan,
    em: chalk.italic,
    heading: chalk.green.bold,
    link: chalk.blue,
    strong: chalk.bold
  })
});

// Preprocess markdown to fix formatting issues, then use marked-terminal
function formatMarkdown(text) {
  try {
    // Preprocess: Replace problematic formatting patterns
    let processedText = text
      // Convert bold in lists to a special placeholder
      .replace(/^(\s*[-*+]\s+.*?)(\*\*([^*]+)\*\*)/gm, (match, prefix, boldPart, boldText) => {
        return prefix + `__BOLD_START__${boldText}__BOLD_END__`;
      })
      // Convert italic in lists to a special placeholder
      .replace(/^(\s*[-*+]\s+.*?)(\*([^*]+)\*)/gm, (match, prefix, italicPart, italicText) => {
        return prefix + `__ITALIC_START__${italicText}__ITALIC_END__`;
      })
      // Convert code in lists to a special placeholder
      .replace(/^(\s*[-*+]\s+.*?)(`([^`]+)`)/gm, (match, prefix, codePart, codeText) => {
        return prefix + `__CODE_START__${codeText}__CODE_END__`;
      });
    
    // Process with marked-terminal
    let result = marked(processedText);
    
    // Post-process: Replace placeholders with actual formatting
    result = result
      .replace(/__BOLD_START__(.*?)__BOLD_END__/g, (match, text) => chalk.bold(text))
      .replace(/__ITALIC_START__(.*?)__ITALIC_END__/g, (match, text) => chalk.italic(text))
      .replace(/__CODE_START__(.*?)__CODE_END__/g, (match, text) => chalk.cyan(text));
    
    return result;
  } catch (error) {
    return text;
  }
}

const {
  SESSION_STATE_IDLE,
  SESSION_STATE_CLOSED,
  SESSION_STATE_BUSY,
  MESSAGE_ROLE_USER,
  MESSAGE_ROLE_HOST
} = require('../lib/constants');

module.exports = (program) => {
  const sessionCmd = program
    .command('session')
    .description('Session management and interactive sessions');

  // Default session start (no subcommand)
  sessionCmd
    .command('start', { isDefault: true })
    .description('Start a new interactive session')
    .option('-S, --secrets <secrets>', 'JSON string of secrets (key-value pairs) for new sessions')
    .option('-i, --instructions <text>', 'Direct instructions text')
    .option('-if, --instructions-file <file>', 'Path to instructions file')
    .option('-s, --setup <text>', 'Direct setup script text')
    .option('-sf, --setup-file <file>', 'Path to setup script file')
    .option('-p, --prompt <text>', 'Prompt to send after session creation')
    .option('-n, --name <name>', 'Name for the session')
    .option('-t, --timeout <seconds>', 'Session timeout in seconds (default: 60)')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session                           # Start a new session\n' +
      '  $ raworc session start -n "my-session"    # Start with name\n' +
      '  $ raworc session start -S \'{"DB_URL":"postgres://..."}\' # Start with user secrets\n' +
      '  $ raworc session start -p "Hello" -n "test" # Start with prompt and name\n' +
      '  $ raworc session start -t 120             # Start with 2 minute timeout\n')
    .action(async (options) => {
      await sessionStartCommand(options);
    });

  // Restore subcommand
  sessionCmd
    .command('restore <session-name>')
    .description('Restore an existing session by name')
    .option('-p, --prompt <text>', 'Prompt to send after restoring')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session restore abc123           # Restore by name\n' +
      '  $ raworc session restore my-session       # Restore by name\n' +
      '  $ raworc session restore my-session -p "Continue work" # Restore with prompt\n')
    .action(async (sessionName, options) => {
      await sessionRestoreCommand(sessionName, options);
    });

  // Remix subcommand  
  sessionCmd
    .command('remix <session-name>')
    .description('Create a new session remixing an existing session')
    .option('-n, --name <name>', 'Name for the new session')
    .option('-c, --code <boolean>', 'Include code files (default: true)')
    .option('-s, --secrets <boolean>', 'Include secrets (default: true)')
    .option('-p, --prompt <text>', 'Prompt to send after creation')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session remix abc123             # Remix by name\n' +
      '  $ raworc session remix my-session         # Remix by name\n' +
      '  $ raworc session remix my-session -n "new-name" # Remix with new name\n' +
      '  $ raworc session remix my-session -s false # Remix without secrets\n' +
      '  $ raworc session remix my-session --code false # Copy only secrets\n')
    .action(async (sessionName, options) => {
      await sessionRemixCommand(sessionName, options);
    });

  // Publish subcommand
  sessionCmd
    .command('publish <session-name>')
    .description('Publish a session for public access')
    .option('-c, --code <boolean>', 'Allow code remix (default: true)')
    .option('-s, --secrets <boolean>', 'Allow secrets remix (default: true)')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session publish abc123           # Publish with all permissions\n' +
      '  $ raworc session publish my-session       # Publish by name\n' +
      '  $ raworc session publish abc123 --secrets false # Publish without secrets remix\n' +
      '  $ raworc session publish abc123 --secrets false # Only allow code remix\n')
    .action(async (sessionName, options) => {
      await sessionPublishCommand(sessionName, options);
    });

  // Unpublish subcommand
  sessionCmd
    .command('unpublish <session-name>')
    .description('Remove session from public access')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session unpublish abc123         # Unpublish by name\n' +
      '  $ raworc session unpublish my-session     # Unpublish by name\n')
    .action(async (sessionName, options) => {
      await sessionUnpublishCommand(sessionName, options);
    });

  // Close subcommand
  sessionCmd
    .command('close <session-name>')
    .description('Close an active session')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session close abc123            # Close by name\n' +
      '  $ raworc session close my-session        # Close by name\n')
    .action(async (sessionName, options) => {
      await sessionCloseCommand(sessionName, options);
    });
};

async function sessionStartCommand(options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }


  let sessionName = null;

  try {
    // Create a new session

    // Prepare session creation payload
    const sessionPayload = {};

    // Add secrets if provided
    if (options.secrets) {
      try {
        sessionPayload.secrets = JSON.parse(options.secrets);
      } catch (error) {
        console.error(chalk.red('✗ Error:'), 'Secrets must be valid JSON');
        process.exit(1);
      }
    }

    // Note: ANTHROPIC_API_KEY is now generated automatically by the system
    // Users can still provide their own API key in secrets for custom code if needed

    // Add instructions if provided
    if (options.instructions) {
      sessionPayload.instructions = options.instructions;
    } else if (options.instructionsFile) {
      try {
        const fs = require('fs');
        sessionPayload.instructions = fs.readFileSync(options.instructionsFile, 'utf8');
      } catch (error) {
        console.error(chalk.red('✗ Error:'), error.message);
        process.exit(1);
      }
    }

    // Add setup if provided
    if (options.setup) {
      sessionPayload.setup = options.setup;
    } else if (options.setupFile) {
      try {
        const fs = require('fs');
        sessionPayload.setup = fs.readFileSync(options.setupFile, 'utf8');
      } catch (error) {
        console.error(chalk.red('✗ Error:'), error.message);
        process.exit(1);
      }
    }

    // Add prompt if provided
    if (options.prompt) {
      sessionPayload.prompt = options.prompt;
    }

    // Add name if provided
    if (options.name) {
      // Validate name format
      if (options.name.length === 0 || options.name.length > 100) {
        console.error(chalk.red('✗ Error:'), 'Session name must be 1-100 characters long');
        process.exit(1);
      }
      if (!/^[a-zA-Z0-9-]+$/.test(options.name)) {
        console.error(chalk.red('✗ Error:'), 'Session name must contain only alphanumeric characters and hyphens');
        console.log(chalk.gray('Examples:'), 'my-session, data-analysis, project1, test-run');
        process.exit(1);
      }
      sessionPayload.name = options.name;
    }

    // Add timeout if provided
    if (options.timeout) {
      const timeoutSeconds = parseInt(options.timeout);
      if (isNaN(timeoutSeconds) || timeoutSeconds <= 0) {
        console.error(chalk.red('✗ Error:'), 'Timeout must be a positive number in seconds');
        process.exit(1);
      }
      sessionPayload.timeout_seconds = timeoutSeconds;
    }

    const createResponse = await api.post('/sessions', sessionPayload);

    if (!createResponse.success) {
      console.error(chalk.red('✗ Error:'), createResponse.error);

      if (createResponse.status === 400) {
        console.log();
        console.log(chalk.yellow('ℹ') + ' Check if your session parameters are valid');
      }

      process.exit(1);
    }

    sessionName = createResponse.data.name;

    await startInteractiveSession(sessionName, options);

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionRestoreCommand(sessionName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }


  try {

    // Get session details first
    const sessionResponse = await api.get(`/sessions/${sessionName}`);

    if (!sessionResponse.success) {
      console.error(chalk.red('✗ Error:'), sessionResponse.error || 'Session does not exist');
      process.exit(1);
    }

    const session = sessionResponse.data;

    // Update sessionName to actual name for consistent display
    sessionName = session.name;

    // Handle different session states
    if (session.state === SESSION_STATE_CLOSED) {
      const restorePayload = {};
      if (options.prompt) {
        restorePayload.prompt = options.prompt;
      }

      const restoreResponse = await api.post(`/sessions/${sessionName}/restore`, restorePayload);

      if (!restoreResponse.success) {
        console.error(chalk.red('✗ Error:'), restoreResponse.error);
        process.exit(1);
      }

    } else if (session.state === SESSION_STATE_IDLE) {

      // If prompt provided for already-running session, send it as a message
      if (options.prompt) {
        console.log(chalk.blue('Sending prompt to running session:'), options.prompt);
        try {
          const messageResponse = await api.post(`/sessions/${sessionName}/messages`, {
            content: options.prompt,
            role: 'user'
          });

          if (messageResponse.success) {
            console.log(chalk.green('✓') + ' Prompt sent successfully');
          } else {
            console.log(chalk.yellow('Warning: Failed to send prompt:'), messageResponse.error);
          }
        } catch (error) {
          console.log(chalk.yellow('Warning: Failed to send prompt:'), error.message);
        }
        console.log();
      }
    } else if (session.state === SESSION_STATE_BUSY) {
      console.log(chalk.yellow('ℹ') + ' Session is currently processing. You can observe ongoing activity.');
      console.log();
    } else {
      process.exit(1);
    }

    await startInteractiveSession(sessionName, { ...options, isRestore: true, sessionState: session.state });

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionRemixCommand(sourceSessionName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }


  try {
    // Get session details first
    const sessionResponse = await api.get(`/sessions/${sourceSessionName}`);

    if (!sessionResponse.success) {
      console.error(chalk.red('✗ Error:'), sessionResponse.error || 'Session does not exist');
      process.exit(1);
    }

    const sourceSession = sessionResponse.data;
    // Update sessionName to actual name for consistent display
    sourceSessionName = sourceSession.name;

    // Prepare remix payload
    const remixPayload = {};

    if (options.code !== undefined) {
      remixPayload.code = options.code === 'true' || options.code === true;
    }

    if (options.secrets !== undefined) {
      remixPayload.secrets = options.secrets === 'true' || options.secrets === true;
    }

    // Canvas is always included by default
    remixPayload.canvas = true;

    // Add prompt if provided
    if (options.prompt) {
      remixPayload.prompt = options.prompt;
    }

    // Add name if provided
    if (options.name) {
      // Validate name format
      if (options.name.length === 0 || options.name.length > 100) {
        console.error(chalk.red('✗ Error:'), 'Session name must be 1-100 characters long');
        process.exit(1);
      }
      if (!/^[a-zA-Z0-9-]+$/.test(options.name)) {
        console.error(chalk.red('✗ Error:'), 'Session name must contain only alphanumeric characters and hyphens');
        console.log(chalk.gray('Examples:'), 'my-session, data-analysis, project1, test-run');
        process.exit(1);
      }
      remixPayload.name = options.name;
    }

    // Create remix session
    const remixResponse = await api.post(`/sessions/${sourceSessionName}/remix`, remixPayload);

    if (!remixResponse.success) {
      console.error(chalk.red('✗ Error:'), remixResponse.error);
      process.exit(1);
    }

    const sessionName = remixResponse.data.name;
    const newSession = remixResponse.data;

    // Show detailed remix success info
    if (newSession.name) {
      console.log(chalk.green('✓') + ` Session remixed as "${newSession.name}": ${sessionName}`);
    }


    await startInteractiveSession(sessionName, { ...options, sourceSessionName: sourceSessionName });

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function showSessionBox(sessionName, mode, user, source = null) {
  // Create descriptive title based on mode
  const modeIcons = {
    'New': `${display.icons.session} Session Start`,
    'Restore': `${display.icons.session} Session Restore`, 
    'Remix': `${display.icons.session} Session Remix`
  };
  
  const title = modeIcons[mode] || `${display.icons.session} Session`;
  const commands = '/help (for commands)';
  
  // Build base lines
  const lines = [
    `Session: ${sessionName}`,
    source ? `Source: ${source}` : null,
    `User: ${user}`,
    `Commands: ${commands}`
  ].filter(line => line !== null);
  
  // Try to get Canvas URL from session info
  try {
    const sessionResponse = await api.get(`/sessions/${sessionName}`);
    if (sessionResponse.success && sessionResponse.data && sessionResponse.data.canvas_port) {
      // Extract hostname from server URL instead of hardcoding localhost
      const serverUrl = config.getServerUrl();
      const serverHostname = new URL(serverUrl).hostname;
      const canvasUrl = `http://${serverHostname}:${sessionResponse.data.canvas_port}/`;
      lines.splice(-1, 0, `Canvas: ${canvasUrl}`); // Insert before Commands line
    }
  } catch (error) {
    // Continue without Canvas URL if API call fails
  }
  
  const maxWidth = Math.max(title.length, ...lines.map(line => line.length));
  const boxWidth = maxWidth + 4; // Add padding
  
  // Create box with title
  console.log();
  console.log('┌' + '─'.repeat(boxWidth - 2) + '┐');
  
  // Title row
  const titlePadding = ' '.repeat(boxWidth - title.length - 4);
  console.log(`│ ${title}${titlePadding} │`);
  
  // Content rows
  lines.forEach(line => {
    const padding = ' '.repeat(boxWidth - line.length - 4);
    console.log(`│ ${line}${padding} │`);
  });
  
  console.log('└' + '─'.repeat(boxWidth - 2) + '┘');
}

async function startInteractiveSession(sessionName, options) {
  // Get user info and determine mode
  const authData = config.getAuth();
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  const user = userName + userType;
  
  let mode = 'New';
  let source = null;
  
  if (options.isRestore) {
    mode = 'Restore';
  } else if (options.sourceSessionName) {
    mode = 'Remix';
    source = options.sourceSessionName;
  }
  
  await showSessionBox(sessionName, mode, user, source);

  // Show recent conversation history for restored sessions
  if (options.isRestore) {
    try {
      const messagesResponse = await api.get(`/sessions/${sessionName}/messages`);

      if (messagesResponse.success && messagesResponse.data && messagesResponse.data.length > 0) {
        const messages = messagesResponse.data;
        const recentMessages = messages.slice(-6); // Show last 6 messages (3 exchanges)

        console.log();

        recentMessages.forEach((msg, index) => {
          console.log();

          // Truncate long messages for history display
          const content = msg.content.length > 200
            ? msg.content.substring(0, 200) + '...'
            : msg.content;

          if (msg.role === 'user') {
            // User messages: show with green "> " prefix
            const lines = content.split('\n');
            lines.forEach(line => {
              if (line.trim() === '') {
                console.log(chalk.green('> '));
              } else {
                console.log(chalk.green('> ') + chalk.white(line));
              }
            });
          } else {
            // Host messages: show content without "> " prefix, just normal text
            const lines = content.split('\n');
            lines.forEach(line => {
              console.log(line);
            });
          }
        });

        console.log();
        console.log(chalk.gray('─'.repeat(50)));

      } else {
      }
    } catch (error) {
      console.log(chalk.yellow('Warning: Could not load conversation history'));
    }
  }

  console.log();

  // Handle prompt if provided (for any session type)
  if (options.prompt) {
    console.log(chalk.green('> ') + chalk.white(options.prompt));

    // Create comprehensive prompt manager
    const promptManager = createPromptManager(sessionName);
    await promptManager.show();
    promptManager.startMonitoring();
    
    try {
      // Send the prompt message to the API
      const sendTime = Date.now();
      const sendResponse = await api.post(`/sessions/${sessionName}/messages`, {
        content: options.prompt,
        role: 'user'
      });
      
      if (!sendResponse.success) {
        console.log(chalk.yellow('Warning: Failed to send prompt:'), sendResponse.error);
        promptManager.stopMonitoring();
        return;
      }
      
      // Wait for all host responses to the prompt (tool calls + final response)
      await waitForAllHostResponses(sessionName, sendTime, 60000, {
        clearPromptFn: () => promptManager.hide(),
        showPromptFn: (state) => {
          showPrompt(state);
          promptManager.visible = true;
        },
        currentState: promptManager.currentState
      });
      
      // Don't show final prompt here - let chatLoop handle it
      
    } catch (error) {
      console.log(chalk.yellow('Warning:'), error.message);
      console.log();
    } finally {
      // Stop prompt manager monitoring and clear any existing prompt before transitioning to chatLoop
      promptManager.stopMonitoring();
      promptManager.hide();
    }
  }

  // If connecting to a busy session, start monitoring for ongoing activity
  if (options.sessionState === SESSION_STATE_BUSY) {
    console.log(chalk.blue('ℹ') + ' Monitoring ongoing session activity...');
    console.log();

    // Start monitoring without a user message time (will show any new messages)
    const monitoringPromise = monitorForResponses(sessionName, 0);

    // Start chat loop concurrently so user can still interact
    const chatPromise = chatLoop(sessionName, options);

    // Wait for either to complete (though monitoring should complete when host finishes)
    await Promise.race([monitoringPromise, chatPromise]);
  } else {
    // Start synchronous chat loop - don't skip initial prompt, let chatLoop show the correct state
    await chatLoop(sessionName, { ...options, skipInitialPrompt: false });
  }
}

async function waitForHostResponse(sessionName, userMessageTime, timeoutMs = 60000) {
  const startTime = Date.now();
  const pollInterval = 1500; // Check every 1.5 seconds
  let lastCheckedCount = 0;

  // Get initial message count to detect new messages
  try {
    const initialResponse = await api.get(`/sessions/${sessionName}/messages`);
    if (initialResponse.success && initialResponse.data) {
      const messages = Array.isArray(initialResponse.data) ? initialResponse.data : initialResponse.data.messages || [];
      lastCheckedCount = messages.length;
    }
  } catch (error) {
    // Continue with 0 count
  }

  while (Date.now() - startTime < timeoutMs) {
    try {
      const response = await api.get(`/sessions/${sessionName}/messages`);

      if (response.success && response.data) {
        const messages = Array.isArray(response.data) ? response.data : response.data.messages || [];

        // Check if we have new messages
        if (messages.length > lastCheckedCount) {
          // Look for the newest host message that was created after our user message
          for (let i = messages.length - 1; i >= 0; i--) {
            const message = messages[i];
            if (message.role === MESSAGE_ROLE_HOST) {
              const messageTime = new Date(message.created_at).getTime();
              if (messageTime > userMessageTime) {
                return message;
              }
            }
          }
        }

        lastCheckedCount = messages.length;
      }
    } catch (error) {
      // Continue polling on error
    }

    // Wait before polling again
    await new Promise(resolve => setTimeout(resolve, pollInterval));
  }

  throw new Error('Timeout waiting for host response');
}

// Shared function to display host messages (tool calls + final response)
function displayHostMessage(message, options = {}) {
  const { clearPromptFn, showPromptFn, updateStateFn, setPromptVisibleFn } = options;
  const metadata = message.metadata;
  
  if (metadata && metadata.type === 'tool_execution') {
    // Handle tool execution message
    if (clearPromptFn) {
      clearPromptFn();
      if (setPromptVisibleFn) setPromptVisibleFn(false);
    }
    
    let toolType = message.metadata?.tool_type || 'unknown';
    const toolNameMap = {
      'text_editor': 'Text Editor',
      'bash': 'Run Bash',
      'web_search': 'Web Search'
    };
    toolType = toolNameMap[toolType] || toolType;
    console.log();
    console.log(chalk.green('● ') + chalk.white(toolType));
    console.log(chalk.dim('└─ ') + chalk.gray(message.content));
    
    if (showPromptFn) {
      if (updateStateFn) {
        updateStateFn().then(() => {
          showPromptFn(options.currentState || 'init');
          if (setPromptVisibleFn) setPromptVisibleFn(true);
        });
      } else {
        // In prompt context, just re-show the prompt directly
        showPromptFn(options.currentState || 'init');
      }
    }
    return 'tool_execution';
  } else if (metadata && metadata.type === 'assistant_reasoning') {
    // Handle Claude's reasoning/explanation before tool execution
    if (clearPromptFn) {
      clearPromptFn();
      if (setPromptVisibleFn) setPromptVisibleFn(false);
    }
    
    console.log();
    const formattedContent = formatMarkdown(message.content);
    console.log(formattedContent.trim());
    
    // Don't show prompt after reasoning - tool execution will handle it
    return 'assistant_reasoning';
  } else {
    // Handle conversational response
    if (clearPromptFn) {
      clearPromptFn();
      if (setPromptVisibleFn) setPromptVisibleFn(false);
    }
    
    console.log();
    const formattedContent = formatMarkdown(message.content);
    console.log(formattedContent.trim());
    
    if (showPromptFn) {
      if (updateStateFn) {
        updateStateFn().then(() => {
          showPromptFn(options.currentState || 'init');
          if (setPromptVisibleFn) setPromptVisibleFn(true);
        });
      } else {
        // In prompt context, just re-show the prompt directly
        showPromptFn(options.currentState || 'init');
      }
    }
    return 'final_response';
  }
}

async function waitForAllHostResponses(sessionName, userMessageTime, timeoutMs = 60000, promptOptions = {}) {
  const startTime = Date.now();
  const pollInterval = 1500; // Check every 1.5 seconds
  let lastMessageCount = 0;
  let foundFinalResponse = false;

  // Get initial message count
  try {
    const initialResponse = await api.get(`/sessions/${sessionName}/messages`);
    if (initialResponse.success && initialResponse.data) {
      const messages = Array.isArray(initialResponse.data) ? initialResponse.data : initialResponse.data.messages || [];
      lastMessageCount = messages.length;
    }
  } catch (error) {
    // Continue with 0 count
  }

  while (Date.now() - startTime < timeoutMs && !foundFinalResponse) {
    try {
      const response = await api.get(`/sessions/${sessionName}/messages`);

      if (response.success && response.data) {
        const messages = Array.isArray(response.data) ? response.data : response.data.messages || [];
        
        // Process any new messages
        if (messages.length > lastMessageCount) {
          const newMessages = messages.slice(lastMessageCount);
          
          for (const message of newMessages) {
            if (message.role === MESSAGE_ROLE_HOST) {
              const messageTime = new Date(message.created_at).getTime();
              if (messageTime > userMessageTime) {
                const result = displayHostMessage(message, {
                  clearPromptFn: promptOptions.clearPromptFn,
                  showPromptFn: promptOptions.showPromptFn,
                  updateStateFn: null, // No state update needed in prompt context
                  setPromptVisibleFn: null,
                  currentState: promptOptions.currentState
                });
                if (result === 'final_response') {
                  foundFinalResponse = true;
                  break;
                }
              }
            }
          }
          
          lastMessageCount = messages.length;
        }
      }
    } catch (error) {
      // Continue polling on error
    }

    if (!foundFinalResponse) {
      await new Promise(resolve => setTimeout(resolve, pollInterval));
    }
  }

  if (!foundFinalResponse) {
    console.log();
    console.log(chalk.yellow('No final response received within timeout'));
    console.log();
  }
}

// Comprehensive prompt manager - handles all prompt operations
function createPromptManager(sessionName, userInput = '') {
  let currentState = 'init';
  let currentUserInput = userInput;
  let promptVisible = false;
  let stateMonitorInterval = null;
  let dotAnimationInterval = null;
  let isRestoringFromClosed = false;
  
  const updateState = async () => {
    try {
      const sessionResponse = await api.get(`/sessions/${sessionName}`);
      if (sessionResponse.success) {
        const newState = sessionResponse.data.state;
        
        // Special handling for sessions being restored from closed state
        if (isRestoringFromClosed) {
          // Stay in 'init' until server confirms session is ready (idle or busy)
          if (newState === 'idle' || newState === 'busy') {
            isRestoringFromClosed = false; // Clear the flag
            currentState = newState;
          }
          // Otherwise keep showing 'init' state
        } else {
          // Normal state transitions
          if (newState !== currentState) {
            currentState = newState;
          }
        }
        
        // Only redraw if prompt is currently visible
        if (promptVisible) {
          clearPrompt();
          if (currentUserInput) {
            showPromptWithInput(currentState, currentUserInput);
          } else {
            showPrompt(currentState);
          }
        }
      }
    } catch (error) {
      // Keep current state if API fails
    }
    return currentState;
  };
  
  const startMonitoring = (isRestoring = false) => {
    isRestoringFromClosed = isRestoring;
    
    if (stateMonitorInterval || dotAnimationInterval) return; // Already started
    
    // Monitor session state changes every 2 seconds
    stateMonitorInterval = setInterval(updateState, 2000);
    
    // Animation interval for dots (every 500ms)
    dotAnimationInterval = setInterval(async () => {
      await updateState();
      if (promptVisible && (currentState === 'init' || currentState === 'busy')) {
        clearPrompt();
        if (currentUserInput) {
          showPromptWithInput(currentState, currentUserInput);
        } else {
          showPrompt(currentState);
        }
      }
    }, 500);
  };
  
  const stopMonitoring = () => {
    if (stateMonitorInterval) {
      clearInterval(stateMonitorInterval);
      stateMonitorInterval = null;
    }
    if (dotAnimationInterval) {
      clearInterval(dotAnimationInterval);
      dotAnimationInterval = null;
    }
  };
  
  const show = async (skipInitial = false) => {
    if (!skipInitial) {
      await updateState();
      if (currentUserInput) {
        showPromptWithInput(currentState, currentUserInput);
      } else {
        showPrompt(currentState);
      }
      promptVisible = true;
    }
  };
  
  const hide = () => {
    if (promptVisible) {
      clearPrompt();
      promptVisible = false;
    }
  };
  
  const updateUserInput = (input) => {
    currentUserInput = input;
  };
  
  const getCurrentState = async () => {
    return await updateState();
  };
  
  return {
    startMonitoring,
    stopMonitoring,
    show,
    hide,
    updateUserInput,
    getCurrentState,
    get currentState() { return currentState; },
    get visible() { return promptVisible; },
    set visible(value) { promptVisible = value; }
  };
}

function showPrompt(state = 'init') {
  const stateIcons = {
    'init': '◯',      // empty circle - initializing
    'idle': '●',      // solid circle - ready
    'busy': '◐',      // half circle - working
    'closed': '◻',    // empty square - closed/slept
    'errored': '◆',   // diamond - error
    'deleted': '◼'    // filled square - deleted
  };

  const stateLabels = {
    'init': 'initializing',
    'idle': 'idle',
    'busy': 'working',
    'closed': 'slept',
    'errored': 'error',
    'deleted': 'deleted'
  };
  
  const stateColors = {
    'init': chalk.blue,
    'idle': chalk.green,
    'busy': chalk.yellow,
    'closed': chalk.cyan,     // brighter than gray
    'errored': chalk.red,
    'deleted': chalk.magenta
  };
  
  const icon = stateIcons[state] || '◯';
  let label = stateLabels[state] || state;
  
  // Add animated dots for init and busy states
  if (state === 'init' || state === 'busy') {
    const dots = Math.floor(Date.now() / 500) % 3 + 1;
    label += '.'.repeat(dots);
  }
  
  const color = stateColors[state] || chalk.gray;
  console.log();
  console.log(color(`${icon} ${label}`));
  console.log(chalk.gray('─'.repeat(50)));
  process.stdout.write(chalk.cyanBright('> '));
}

function showPromptWithInput(state = 'init', userInput = '') {
  const stateIcons = {
    'init': '◯',      // empty circle - initializing
    'idle': '●',      // solid circle - ready
    'busy': '◐',      // half circle - working
    'closed': '◻',    // empty square - closed/slept
    'errored': '◆',   // diamond - error
    'deleted': '◼'    // filled square - deleted
  };

  const stateLabels = {
    'init': 'initializing',
    'idle': 'idle',
    'busy': 'working',
    'closed': 'slept',
    'errored': 'error',
    'deleted': 'deleted'
  };
  
  const stateColors = {
    'init': chalk.blue,
    'idle': chalk.green,
    'busy': chalk.yellow,
    'closed': chalk.cyan,     // brighter than gray
    'errored': chalk.red,
    'deleted': chalk.magenta
  };
  
  const icon = stateIcons[state] || '◯';
  let label = stateLabels[state] || state;
  
  // Add animated dots for init and busy states
  if (state === 'init' || state === 'busy') {
    const dots = Math.floor(Date.now() / 500) % 3 + 1;
    label += '.'.repeat(dots);
  }
  
  const color = stateColors[state] || chalk.gray;
  console.log();
  console.log(color(`${icon} ${label}`));
  console.log(chalk.gray('─'.repeat(50)));
  process.stdout.write(chalk.cyanBright('> ') + userInput);
}

function clearPrompt() {
  // Clear the 4-line prompt structure:
  // Line 4: "> " cursor (current line, no newline)
  // Line 3: dash line 
  // Line 2: state line
  // Line 1: empty line
  process.stdout.write('\r\x1b[2K');      // Clear current line (cursor line)
  process.stdout.write('\x1b[1A\x1b[2K'); // Move up and clear dash line
  process.stdout.write('\x1b[1A\x1b[2K'); // Move up and clear state line  
  process.stdout.write('\x1b[1A\x1b[2K'); // Move up and clear empty line
}

async function monitorForResponses(sessionName, userMessageTime, getCurrentState, updateState, getPromptVisible, setPromptVisible) {
  let lastMessageCount = 0;

  try {
    const initialResponse = await api.get(`/sessions/${sessionName}/messages`);
    if (initialResponse.success) {
      lastMessageCount = initialResponse.data.length;
    }
  } catch (error) {
    return;
  }

  while (true) {
    try {
      const response = await api.get(`/sessions/${sessionName}/messages`);
      if (response.success && response.data.length > lastMessageCount) {
        const newMessages = response.data.slice(lastMessageCount);

        for (const message of newMessages) {
          if (message.role === 'host') {
            const result = displayHostMessage(message, {
              clearPromptFn: getPromptVisible() ? clearPrompt : null,
              showPromptFn: showPrompt,
              updateStateFn: updateState,
              setPromptVisibleFn: setPromptVisible,
              currentState: getCurrentState()
            });
            
            if (result === 'final_response') {
              return;
            }
          }
        }
        lastMessageCount = response.data.length;
      }
      await new Promise(resolve => setTimeout(resolve, 1500));
    } catch (error) {
      break;
    }
  }
}

async function chatLoop(sessionName, options = {}) {
  const readline = require('readline');
  // For restored sessions from closed state, start with 'init' and wait for server to confirm ready
  // For new sessions, start with 'init' 
  // For other cases, use current session state
  let currentSessionState = 'init';
  let currentUserInput = '';
  let promptVisible = false; // Track if prompt is currently displayed
  let isRestoringFromClosed = options.isRestore && options.sessionState === SESSION_STATE_CLOSED;

  // Function to fetch and update session state
  async function updateSessionState() {
    try {
      const sessionResponse = await api.get(`/sessions/${sessionName}`);
      if (sessionResponse.success) {
        const newState = sessionResponse.data.state;
        
        // Special handling for sessions being restored from closed state
        if (isRestoringFromClosed) {
          // Stay in 'init' until server confirms session is ready (idle or busy)
          if (newState === 'idle' || newState === 'busy') {
            isRestoringFromClosed = false; // Clear the flag
            currentSessionState = newState;
          }
          // Otherwise keep showing 'init' state
        } else {
          // Normal state transitions
          if (newState !== currentSessionState) {
            currentSessionState = newState;
          }
        }
        
        // Only redraw if prompt is currently visible
        if (promptVisible) {
          clearPrompt();
          showPromptWithInput(currentSessionState, currentUserInput);
        }
        
        return currentSessionState;
      }
    } catch (error) {
      // Keep current state if API fails
    }
    return currentSessionState;
  }

  // Get initial session state
  await updateSessionState();

  // Enable keypress events
  readline.emitKeypressEvents(process.stdin);
  if (process.stdin.setRawMode) {
    process.stdin.setRawMode(true);
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  // Monitor session state changes every 2 seconds
  const stateMonitorInterval = setInterval(updateSessionState, 2000);
  
  // Animation interval for dots (every 500ms)
  const dotAnimationInterval = setInterval(() => {
    if (promptVisible && (currentSessionState === 'init' || currentSessionState === 'busy')) {
      clearPrompt();
      showPromptWithInput(currentSessionState, currentUserInput);
    }
  }, 500);

  // Only show initial prompt if not skipping it
  if (!options.skipInitialPrompt) {
    showPrompt(currentSessionState);
    promptVisible = true;
  }

  // Track user input as they type
  process.stdin.on('keypress', (str, key) => {
    if (!key) return;
    
    if (key.name === 'backspace' || key.name === 'delete') {
      currentUserInput = currentUserInput.slice(0, -1);
    } else if (key.name === 'return') {
      currentUserInput = '';
    } else if (str && str.length === 1 && !key.ctrl && !key.meta) {
      currentUserInput += str;
    }
  });

  rl.on('line', async (input) => {
    const userInput = input.trim();
    currentUserInput = ''; // Reset after line submitted

    if (!userInput) {
      clearPrompt();
      promptVisible = false;
      showPrompt(currentSessionState);
      promptVisible = true;
      return;
    }

    // Handle quit command
    if (userInput.toLowerCase() === '/quit' || userInput.toLowerCase() === '/q' || userInput.toLowerCase() === 'exit') {
      clearPrompt();
      promptVisible = false;
      await cleanup();
      return;
    }

    // Handle detach command - exit interactive mode without closing session
    if (userInput.toLowerCase() === '/detach' || userInput.toLowerCase() === '/d') {
      clearPrompt();
      promptVisible = false;
      console.log(chalk.green('◊ Detached from session. Session continues running.'));
      console.log(chalk.gray('Reconnect with: ') + chalk.white(`raworc session restore ${sessionName}`));
      process.exit(0);
    }

    // Handle commands - don't return early, just set a flag
    let shouldSendMessage = true;
    
    // Handle status command
    if (userInput === '/status') {
      clearPrompt(); // Short command, no enter to clear
      promptVisible = false;
      await showSessionStatus(sessionName);
      shouldSendMessage = false;
    }
    // Handle help command
    else if (userInput === '/help' || userInput === '/h') {
      clearPrompt(); // Short command, no enter to clear
      promptVisible = false;
      showHelp();
      shouldSendMessage = false;
    }
    // Handle timeout commands
    else {
      const timeoutMatch = userInput.match(/^(?:\/t|\/timeout|timeout)\s+(\d+)$/);
      if (timeoutMatch) {
        clearPrompt(); // Short command, no enter to clear
        promptVisible = false;
        await handleTimeoutCommand(sessionName, parseInt(timeoutMatch[1], 10));
        shouldSendMessage = false;
      }
    }
    
    // Show prompt after command execution or send message
    if (!shouldSendMessage) {
      showPrompt(currentSessionState);
      promptVisible = true;
    } else {
      // Send message to session - clear with enter since it's regular input
      clearPrompt();
      promptVisible = false;
      await sendMessage(sessionName, userInput);
    }
  });

  async function cleanup() {
    clearInterval(stateMonitorInterval);
    clearInterval(dotAnimationInterval);
    rl.close();
    
    // Close the session on the server silently
    try {
      await api.post(`/sessions/${sessionName}/close`);
    } catch (error) {
      // Ignore all errors during cleanup
    }
    
    console.log();
    console.log(chalk.cyan('Goodbye! 👋'));
    
    process.exit(0);
  }

  process.on('SIGINT', () => cleanup());
  process.on('SIGTERM', () => cleanup());

  async function sendMessage(sessionName, userInput) {
    console.log(chalk.green('> ') + chalk.white(userInput));
    
    // Show prompt with current actual state
    showPrompt(currentSessionState);
    promptVisible = true;

    try {
      const sendResponse = await api.post(`/sessions/${sessionName}/messages`, {
        content: userInput,
        role: 'user'
      });

      if (!sendResponse.success) {
        clearPrompt();
        promptVisible = false;
        console.log(chalk.red('✗ Failed to send message:'), sendResponse.error);
        // Update state from server after error
        await updateSessionState();
        showPrompt(currentSessionState);
        promptVisible = true;
        return;
      }

      await monitorForResponses(sessionName, Date.now(), () => currentSessionState, updateSessionState, () => promptVisible, (visible) => { promptVisible = visible; });

    } catch (error) {
      clearPrompt();
      promptVisible = false;
      console.log(chalk.red('✗ Error sending message:'), error.message);
      // Update state from server after error
      await updateSessionState();
      showPrompt(currentSessionState);
      promptVisible = true;
    }
  }

  return new Promise((resolve) => {
    rl.on('close', resolve);
  });
}

async function showSessionStatus(sessionName) {
  try {
    const statusResponse = await api.get(`/sessions/${sessionName}`);
    if (statusResponse.success) {
      console.log();
      console.log(chalk.blue('ℹ') + ' Session Status:');
      console.log(chalk.gray('  Name:'), statusResponse.data.name || 'Unnamed');
      console.log(chalk.gray('  State:'), getStateDisplay(statusResponse.data.state));
      console.log(chalk.gray('  Created:'), new Date(statusResponse.data.created_at).toLocaleString());
      console.log(chalk.gray('  Updated:'), new Date(statusResponse.data.updated_at).toLocaleString());
      console.log();
    } else {
      console.log(chalk.red('✗ Failed to get session status:'), statusResponse.error);
    }
  } catch (error) {
    console.log(chalk.red('✗ Error getting session status:'), error.message);
  }
}

function showHelp() {
  console.log(chalk.blue('ℹ') + ' Available Commands:');
  console.log(chalk.gray('  /help       '), 'Show this help message');
  console.log(chalk.gray('  /status     '), 'Show session status');
  console.log(chalk.gray('  /timeout <s>'), 'Change session timeout (1-3600 seconds)');
  console.log(chalk.gray('  /name <name>'), 'Change session name (alphanumeric and hyphens)');
  console.log(chalk.gray('  /detach     '), 'Detach from session (keeps session running)');
  console.log(chalk.gray('  /quit       '), 'End the session');
}

async function handleTimeoutCommand(sessionName, timeoutSeconds) {
  if (timeoutSeconds >= 1 && timeoutSeconds <= 3600) {
    try {
      const updateResponse = await api.put(`/sessions/${sessionName}`, {
        timeout_seconds: timeoutSeconds
      });
      if (updateResponse.success) {
        console.log(chalk.green('✓') + ` Session timeout updated to ${timeoutSeconds} seconds`);
      } else {
        console.log(chalk.red('✗ Failed to update timeout:'), updateResponse.error || 'Unknown error');
      }
    } catch (error) {
      console.log(chalk.red('✗ Failed to update timeout:'), error.message);
    }
  } else {
    console.log(chalk.red('✗') + ' Invalid timeout value. Must be between 1 and 3600 seconds (1 hour).');
  }
}


async function sessionPublishCommand(sessionName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('📢 Publishing Raworc Session'));
  console.log(chalk.gray('Session:'), sessionName);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);

  // Show publishing permissions (same logic as remix)
  const code = options.code === undefined ? true : (options.code === 'true' || options.code === true);
  const secrets = options.secrets === undefined ? true : (options.secrets === 'true' || options.secrets === true);
  // Canvas is always allowed by default
  const canvas = true;

  console.log();
  console.log(chalk.yellow('📋 Remix Permissions:'));
  console.log(chalk.gray('  Code:'), code ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log(chalk.gray('  Secrets:'), secrets ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log(chalk.gray('  Canvas:'), chalk.green('✓ Allowed'));
  console.log();

  try {

    const publishPayload = {
      code: code,
      secrets: secrets,
      canvas: canvas
    };

    const response = await api.post(`/sessions/${sessionName}/publish`, publishPayload);

    if (!response.success) {
      console.error(chalk.red('✗ Error:'), response.error);
      process.exit(1);
    }

    console.log(chalk.green('✓') + ` Session published: ${sessionName}`);

    console.log();
    console.log(chalk.green('✓') + ' Session is now publicly accessible!');
    console.log();
    console.log(chalk.blue('📋 Public Access:'));
    console.log(chalk.gray('  • View:'), `raworc api published/sessions/${sessionName}`);
    console.log(chalk.gray('  • List all:'), 'raworc api published/sessions');
    console.log(chalk.gray('  • Remix:'), `raworc session remix ${sessionName}`);
    console.log();

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionUnpublishCommand(sessionName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🔒 Unpublishing Raworc Session'));
  console.log(chalk.gray('Session:'), sessionName);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);
  console.log();

  try {

    const response = await api.post(`/sessions/${sessionName}/unpublish`);

    if (!response.success) {
      console.error(chalk.red('✗ Error:'), response.error);
      process.exit(1);
    }

    console.log(chalk.green('✓') + ` Session unpublished: ${sessionName}`);

    console.log();
    console.log(chalk.green('✓') + ' Session is now private again');

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionCloseCommand(sessionName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  display.showCommandBox(`${display.icons.stop} Session Close`, {
    session: sessionName,
    operation: 'Close and cleanup resources'
  });

  try {
    // Get session details first (to show current state)
    const sessionResponse = await api.get(`/sessions/${sessionName}`);

    if (!sessionResponse.success) {
      console.error(chalk.red('✗ Error:'), sessionResponse.error || 'Session does not exist');
      process.exit(1);
    }

    const session = sessionResponse.data;
    // Update sessionName to actual name for consistent display
    sessionName = session.name;
console.log(chalk.gray('Current state:'), getStateDisplay(session.state));

    // Check if session is already closed
    if (session.state === SESSION_STATE_CLOSED) {
      display.info('Session was already in closed state');
      return;
    }

    // Close the session
    const closeResponse = await api.post(`/sessions/${sessionName}/close`);

    if (!closeResponse.success) {
      display.error('Close failed: ' + closeResponse.error);
      process.exit(1);
    }

    display.success(`Session closed: ${sessionName}`);

    console.log();
    display.success('Session has been closed and resources cleaned up');
    console.log();
    display.info('Session Operations:');
    console.log(chalk.gray('  • Restore:'), `raworc session restore ${sessionName}`);
    console.log(chalk.gray('  • Remix:'), `raworc session remix ${sessionName}`);
    console.log();

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

function getStateDisplay(state) {
  const stateIcons = {
    'init': '◯',      // empty circle - initializing
    'idle': '●',      // solid circle - ready
    'busy': '◐',      // half circle - working  
    'closed': '◻',    // empty square - closed/slept
    'errored': '◆',   // diamond - error
    'deleted': '◼'    // filled square - deleted
  };

  const stateColors = {
    'init': chalk.blue,
    'idle': chalk.green,
    'busy': chalk.yellow, 
    'closed': chalk.cyan,     // brighter than gray
    'errored': chalk.red,
    'deleted': chalk.magenta
  };
  
  const icon = stateIcons[state] || '◯';
  const color = stateColors[state] || chalk.gray;
  return color(`${icon} ${state}`);
}