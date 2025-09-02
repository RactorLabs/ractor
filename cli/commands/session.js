const chalk = require('chalk');
const inquirer = require('inquirer');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

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
    .command('restore <session-id>')
    .description('Restore an existing session by ID or name')
    .option('-p, --prompt <text>', 'Prompt to send after restoring')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session restore abc123           # Restore by ID\n' +
      '  $ raworc session restore my-session       # Restore by name\n' +
      '  $ raworc session restore my-session -p "Continue work" # Restore with prompt\n')
    .action(async (sessionId, options) => {
      await sessionRestoreCommand(sessionId, options);
    });

  // Remix subcommand  
  sessionCmd
    .command('remix <session-id>')
    .description('Create a new session remixing an existing session')
    .option('-n, --name <name>', 'Name for the new session')
    .option('-d, --data <boolean>', 'Include data files (default: true)')
    .option('-c, --code <boolean>', 'Include code files (default: true)')
    .option('-s, --secrets <boolean>', 'Include secrets (default: true)')
    .option('-p, --prompt <text>', 'Prompt to send after creation')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session remix abc123             # Remix by ID\n' +
      '  $ raworc session remix my-session         # Remix by name\n' +
      '  $ raworc session remix my-session -n "new-name" # Remix with new name\n' +
      '  $ raworc session remix my-session -s false # Remix without secrets\n' +
      '  $ raworc session remix my-session --data false --code false # Copy only secrets\n')
    .action(async (sessionId, options) => {
      await sessionRemixCommand(sessionId, options);
    });

  // Publish subcommand
  sessionCmd
    .command('publish <session-id>')
    .description('Publish a session for public access')
    .option('-d, --data <boolean>', 'Allow data remix (default: true)')
    .option('-c, --code <boolean>', 'Allow code remix (default: true)')
    .option('-s, --secrets <boolean>', 'Allow secrets remix (default: true)')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session publish abc123           # Publish with all permissions\n' +
      '  $ raworc session publish my-session       # Publish by name\n' +
      '  $ raworc session publish abc123 --secrets false # Publish without secrets remix\n' +
      '  $ raworc session publish abc123 --data false --secrets false # Only allow code remix\n')
    .action(async (sessionId, options) => {
      await sessionPublishCommand(sessionId, options);
    });

  // Unpublish subcommand
  sessionCmd
    .command('unpublish <session-id>')
    .description('Remove session from public access')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session unpublish abc123         # Unpublish by ID\n' +
      '  $ raworc session unpublish my-session     # Unpublish by name\n')
    .action(async (sessionId, options) => {
      await sessionUnpublishCommand(sessionId, options);
    });

  // Close subcommand
  sessionCmd
    .command('close <session-id>')
    .description('Close an active session')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session close abc123            # Close by ID\n' +
      '  $ raworc session close my-session        # Close by name\n')
    .action(async (sessionId, options) => {
      await sessionCloseCommand(sessionId, options);
    });
};

async function sessionStartCommand(options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🤖 Starting New Raworc AI Session'));
  console.log(chalk.gray('Mode:'), 'New Session');
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);

  // Show session creation parameters if provided
  if (options.secrets) {
    console.log(chalk.gray('Secrets:'), 'Provided');
  }
  if (options.instructions) {
    console.log(chalk.gray('Instructions:'), options.instructions);
  }
  if (options.setup) {
    console.log(chalk.gray('Setup:'), options.setup);
  }
  if (options.name) {
    console.log(chalk.gray('Name:'), options.name);
  }
  console.log();

  let sessionId = null;

  try {
    // Create a new session
    const spinner = ora('Creating session...').start();

    // Prepare session creation payload
    const sessionPayload = {};

    // Add secrets if provided
    if (options.secrets) {
      try {
        sessionPayload.secrets = JSON.parse(options.secrets);
      } catch (error) {
        spinner.fail('Invalid secrets JSON format');
        console.error(chalk.red('Error:'), 'Secrets must be valid JSON');
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
        spinner.fail('Failed to read instructions file');
        console.error(chalk.red('Error:'), error.message);
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
        spinner.fail('Failed to read setup script');
        console.error(chalk.red('Error:'), error.message);
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
        spinner.fail('Invalid session name');
        console.error(chalk.red('Error:'), 'Session name must be 1-100 characters long');
        process.exit(1);
      }
      if (!/^[a-zA-Z0-9-]+$/.test(options.name)) {
        spinner.fail('Invalid session name');
        console.error(chalk.red('Error:'), 'Session name must contain only alphanumeric characters and hyphens');
        console.log(chalk.gray('Examples:'), 'my-session, data-analysis, project1, test-run');
        process.exit(1);
      }
      sessionPayload.name = options.name;
    }

    // Add timeout if provided
    if (options.timeout) {
      const timeoutSeconds = parseInt(options.timeout);
      if (isNaN(timeoutSeconds) || timeoutSeconds <= 0) {
        spinner.fail('Invalid timeout value');
        console.error(chalk.red('Error:'), 'Timeout must be a positive number in seconds');
        process.exit(1);
      }
      sessionPayload.timeout_seconds = timeoutSeconds;
    }

    const createResponse = await api.post('/sessions', sessionPayload);

    if (!createResponse.success) {
      spinner.fail('Failed to create session');
      console.error(chalk.red('Error:'), createResponse.error);

      if (createResponse.status === 400) {
        console.log();
        console.log(chalk.yellow('💡 Check if your session parameters are valid'));
      }

      process.exit(1);
    }

    sessionId = createResponse.data.id;
    spinner.succeed(`Session created: ${sessionId}`);
    
    await startInteractiveSession(sessionId, options);

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionRestoreCommand(sessionId, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🤖 Restoring Raworc AI Session'));
  console.log(chalk.gray('Mode:'), 'Restore');
  console.log(chalk.gray('Session:'), sessionId);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);
  console.log();

  try {
    const spinner = ora('Restoring session...').start();

    // Get session details first
    const sessionResponse = await api.get(`/sessions/${sessionId}`);

    if (!sessionResponse.success) {
      spinner.fail('Failed to fetch session');
      console.error(chalk.red('Error:'), sessionResponse.error || 'Session does not exist');
      process.exit(1);
    }

    const session = sessionResponse.data;
    console.log(chalk.gray('Session state:'), session.state);
    console.log();
    
    // Update sessionId to actual UUID for consistent display
    sessionId = session.id;

    // Handle different session states
    if (session.state === SESSION_STATE_CLOSED) {
      const restorePayload = {};
      if (options.prompt) {
        restorePayload.prompt = options.prompt;
      }
      
      const restoreResponse = await api.post(`/sessions/${sessionId}/restore`, restorePayload);

      if (!restoreResponse.success) {
        spinner.fail('Failed to restore session');
        console.error(chalk.red('Error:'), restoreResponse.error);
        process.exit(1);
      }

      spinner.succeed(`Session restored: ${sessionId}`);
    } else if (session.state === SESSION_STATE_IDLE) {
      spinner.succeed(`Session already ready: ${sessionId}`);
      
      // If prompt provided for already-running session, send it as a message
      if (options.prompt) {
        console.log(chalk.blue('Sending prompt to running session:'), options.prompt);
        try {
          const messageResponse = await api.post(`/sessions/${sessionId}/messages`, {
            content: options.prompt,
            role: 'user'
          });
          
          if (messageResponse.success) {
            console.log(chalk.green('Prompt sent successfully'));
          } else {
            console.log(chalk.yellow('Warning: Failed to send prompt:'), messageResponse.error);
          }
        } catch (error) {
          console.log(chalk.yellow('Warning: Failed to send prompt:'), error.message);
        }
        console.log();
      }
    } else if (session.state === SESSION_STATE_BUSY) {
      spinner.succeed(`Session connected (currently busy): ${sessionId}`);
      console.log(chalk.yellow('💡 Session is currently processing. You can observe ongoing activity.'));
      console.log();
    } else {
      spinner.fail(`Cannot restore session in state: ${session.state}`);
      process.exit(1);
    }

    await startInteractiveSession(sessionId, { ...options, isRestore: true, sessionState: session.state });

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionRemixCommand(sourceSessionId, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🤖 Remixing Raworc AI Session'));
  console.log(chalk.gray('Mode:'), 'Remix');
  console.log(chalk.gray('Source:'), sourceSessionId);
  if (options.name) {
    console.log(chalk.gray('New Name:'), options.name);
  }
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);

  // Show remix parameters
  if (options.data !== undefined) {
    console.log(chalk.gray('Copy Data:'), options.data === 'true' || options.data === true ? 'Yes' : 'No');
  }
  if (options.code !== undefined) {
    console.log(chalk.gray('Copy Code:'), options.code === 'true' || options.code === true ? 'Yes' : 'No');
  }
  if (options.secrets !== undefined) {
    console.log(chalk.gray('Copy Secrets:'), options.secrets === 'true' || options.secrets === true ? 'Yes' : 'No');
  }
  console.log();

  try {
    const spinner = ora('Remixing session...').start();

    // Prepare remix payload
    const remixPayload = {};

    if (options.data !== undefined) {
      remixPayload.data = options.data === 'true' || options.data === true;
    }

    if (options.code !== undefined) {
      remixPayload.code = options.code === 'true' || options.code === true;
    }

    if (options.secrets !== undefined) {
      remixPayload.secrets = options.secrets === 'true' || options.secrets === true;
    }

    // Add prompt if provided
    if (options.prompt) {
      remixPayload.prompt = options.prompt;
    }

    // Add name if provided
    if (options.name) {
      // Validate name format
      if (options.name.length === 0 || options.name.length > 100) {
        spinner.fail('Invalid session name');
        console.error(chalk.red('Error:'), 'Session name must be 1-100 characters long');
        process.exit(1);
      }
      if (!/^[a-zA-Z0-9-]+$/.test(options.name)) {
        spinner.fail('Invalid session name');
        console.error(chalk.red('Error:'), 'Session name must contain only alphanumeric characters and hyphens');
        console.log(chalk.gray('Examples:'), 'my-session, data-analysis, project1, test-run');
        process.exit(1);
      }
      remixPayload.name = options.name;
    }

    // Create remix session
    const remixResponse = await api.post(`/sessions/${sourceSessionId}/remix`, remixPayload);

    if (!remixResponse.success) {
      spinner.fail('Failed to remix session');
      console.error(chalk.red('Error:'), remixResponse.error);
      process.exit(1);
    }

    const sessionId = remixResponse.data.id;
    const newSession = remixResponse.data;
    
    // Show detailed remix success info
    if (newSession.name) {
      spinner.succeed(`Session remixed as "${newSession.name}": ${sessionId}`);
    } else {
      spinner.succeed(`Session remixed: ${sessionId}`);
    }
    
    console.log(chalk.gray('Source session:'), sourceSessionId);
    if (newSession.name) {
      console.log(chalk.gray('New session name:'), newSession.name);
    }
    console.log(chalk.gray('New session ID:'), sessionId);
    console.log();

    await startInteractiveSession(sessionId, options);

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}

async function startInteractiveSession(sessionId, options) {
  console.log();
  console.log(chalk.green('✅ Session active! Type your messages below.'));
  console.log(chalk.gray('Commands: /status, /timeout <s>, /name <name>, /quit, /help'));
  console.log(chalk.gray('Session ID:'), sessionId);
  
  // Show recent conversation history for restored sessions
  if (options.isRestore) {
    try {
      const historySpinner = ora('Loading conversation history...').start();
      const messagesResponse = await api.get(`/sessions/${sessionId}/messages`);
      
      if (messagesResponse.success && messagesResponse.data && messagesResponse.data.length > 0) {
        const messages = messagesResponse.data;
        const recentMessages = messages.slice(-6); // Show last 6 messages (3 exchanges)
        
        historySpinner.succeed('Conversation history loaded');
        console.log();
        console.log(chalk.blue('📜 Recent conversation history:'));
        console.log(chalk.gray('─'.repeat(50)));
        
        recentMessages.forEach((msg, index) => {
          const timestamp = new Date(msg.created_at).toLocaleTimeString();
          const roleColor = msg.role === 'user' ? chalk.green : chalk.cyan;
          const roleLabel = msg.role === 'user' ? 'You' : 'Host';
          
          console.log();
          console.log(roleColor(`${roleLabel} (${timestamp}):`));
          
          // Truncate long messages for history display
          const content = msg.content.length > 200 
            ? msg.content.substring(0, 200) + '...'
            : msg.content;
          
          console.log(chalk.white(content));
        });
        
        console.log();
        console.log(chalk.gray('─'.repeat(50)));
        console.log(chalk.blue('💬 Continue the conversation below:'));
      } else {
        historySpinner.succeed('No previous messages found');
      }
    } catch (error) {
      console.log(chalk.yellow('Warning: Could not load conversation history'));
    }
  }
  
  console.log();

  // Handle prompt if provided (for any session type)
  if (options.prompt) {
    console.log(chalk.blue('Prompt sent:'), options.prompt);
    
    const responseSpinner = ora('Waiting for host response...').start();
    
    try {
      // Wait for the host to respond to the prompt
      const hostResponse = await waitForHostResponse(sessionId, Date.now());
      
      if (hostResponse) {
        responseSpinner.succeed('Host responded');
        console.log();
        console.log(chalk.cyan('Host:'), chalk.whiteBright(hostResponse.content));
        console.log();
      } else {
        responseSpinner.warn('No host response received within timeout');
        console.log();
      }
    } catch (error) {
      responseSpinner.fail('Error waiting for host response');
      console.log(chalk.yellow('Warning:'), error.message);
      console.log();
    }
  }

  // If connecting to a busy session, start monitoring for ongoing activity
  if (options.sessionState === SESSION_STATE_BUSY) {
    console.log(chalk.blue('🔄 Monitoring ongoing session activity...'));
    console.log();
    
    // Start monitoring without a user message time (will show any new messages)
    const monitoringPromise = monitorForResponses(sessionId, 0);
    
    // Start chat loop concurrently so user can still interact
    const chatPromise = chatLoop(sessionId);
    
    // Wait for either to complete (though monitoring should complete when host finishes)
    await Promise.race([monitoringPromise, chatPromise]);
  } else {
    // Start synchronous chat loop
    await chatLoop(sessionId);
  }
}

async function waitForHostResponse(sessionId, userMessageTime, timeoutMs = 60000) {
  const startTime = Date.now();
  const pollInterval = 1500; // Check every 1.5 seconds
  let lastCheckedCount = 0;

  // Get initial message count to detect new messages
  try {
    const initialResponse = await api.get(`/sessions/${sessionId}/messages`);
    if (initialResponse.success && initialResponse.data) {
      const messages = Array.isArray(initialResponse.data) ? initialResponse.data : initialResponse.data.messages || [];
      lastCheckedCount = messages.length;
    }
  } catch (error) {
    // Continue with 0 count
  }

  while (Date.now() - startTime < timeoutMs) {
    try {
      const response = await api.get(`/sessions/${sessionId}/messages`);

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

function displayToolMessage(message) {
  const toolType = message.metadata?.tool_type || 'unknown';
  const toolIcon = '•';
  
  console.log(chalk.gray(`${toolIcon} ${toolType}`));
  console.log(chalk.dim('├─ ') + chalk.gray(message.content));
}

function displayHostMessage(message) {
  console.log(chalk.cyan('Host: ') + chalk.whiteBright(message.content));
  
  // After host message, show status and prompt
  showStatusAndPrompt();
}

function showStatusAndPrompt() {
  const stateIcon = '💤';
  const stateColor = chalk.green;
  
  console.log(chalk.gray('────────────────────'));
  console.log(`${stateIcon} Session: ${stateColor('idle')}`);
  console.log(chalk.gray('────────────────────'));
  process.stdout.write(chalk.cyanBright('User: '));
}

function clearStatusAndPrompt() {
  // Clear the last four lines: empty line, top dash line, status line, bottom dash line, user prompt
  process.stdout.write('\x1b[4A\x1b[2K\x1b[0G\x1b[1B\x1b[2K\x1b[0G\x1b[1B\x1b[2K\x1b[0G\x1b[1B\x1b[2K\x1b[0G');
}

function startAnimatedIndicator() {
  const frames = [
    chalk.gray('⣾'),
    chalk.blue('⣽'),
    chalk.cyan('⣻'),
    chalk.green('⢿'),
    chalk.yellow('⡿'),
    chalk.red('⣟'),
    chalk.magenta('⣯'),
    chalk.gray('⣷')
  ];
  
  let frameIndex = 0;
  
  const interval = setInterval(() => {
    process.stdout.write('\r' + frames[frameIndex % frames.length]);
    frameIndex++;
  }, 150); // Change frame every 150ms
  
  return interval;
}

async function monitorForResponses(sessionId, userMessageTime, clearStatus = null) {
  let lastMessageCount = 0;
  let statusCleared = false;

  // Get initial message count
  try {
    const initialResponse = await api.get(`/sessions/${sessionId}/messages`);
    if (initialResponse.success) {
      lastMessageCount = initialResponse.data.length;
    }
  } catch (error) {
    console.log(chalk.red('❌ Error getting initial message count'));
    return;
  }

  while (true) {
    try {
      const response = await api.get(`/sessions/${sessionId}/messages`);
      if (response.success && response.data.length > lastMessageCount) {
        const newMessages = response.data.slice(lastMessageCount);
        
        for (const message of newMessages) {
          if (message.role === 'host') {
            // Clear status on first host message if not already cleared
            if (!statusCleared && clearStatus) {
              clearStatus();
              statusCleared = true;
            }
            
            // Check if this is a tool execution message or final response
            const metadata = message.metadata;
            if (metadata && metadata.type === 'tool_execution') {
              displayToolMessage(message);
            } else {
              displayHostMessage(message);
              // Final response received - exit monitoring
              return;
            }
          }
        }
        
        lastMessageCount = response.data.length;
      }
      
      await new Promise(resolve => setTimeout(resolve, 1500)); // Check every 1.5 seconds
    } catch (error) {
      console.log(chalk.red('❌ Error monitoring responses:'), error.message);
      break;
    }
  }
}

async function showSessionStatus(sessionId) {
  try {
    const statusResponse = await api.get(`/sessions/${sessionId}`);
    if (statusResponse.success) {
      console.log();
      console.log(chalk.blue('📊 Session Status:'));
      console.log(chalk.gray('  ID:'), statusResponse.data.id);
      console.log(chalk.gray('  Name:'), statusResponse.data.name || 'Unnamed');
      console.log(chalk.gray('  State:'), getStateDisplay(statusResponse.data.state));
      console.log(chalk.gray('  Created:'), new Date(statusResponse.data.created_at).toLocaleString());
      console.log(chalk.gray('  Updated:'), new Date(statusResponse.data.updated_at).toLocaleString());
      console.log();
    } else {
      console.log(chalk.red('❌ Failed to get session status:'), statusResponse.error);
    }
  } catch (error) {
    console.log(chalk.red('❌ Error getting session status:'), error.message);
  }
}

function showHelp() {
  console.log();
  console.log(chalk.blue('🤖 Available Commands:'));
  console.log(chalk.gray('  /help       '), 'Show this help message');
  console.log(chalk.gray('  /status     '), 'Show session status');
  console.log(chalk.gray('  /timeout <s>'), 'Change session timeout (1-3600 seconds)');
  console.log(chalk.gray('  /name <name>'), 'Change session name (alphanumeric and hyphens)');
  console.log(chalk.gray('  /quit       '), 'End the session');
  console.log();
}

async function handleTimeoutCommand(sessionId, timeoutSeconds) {
  if (timeoutSeconds >= 1 && timeoutSeconds <= 3600) {
    try {
      const updateResponse = await api.put(`/sessions/${sessionId}`, {
        timeout_seconds: timeoutSeconds
      });
      if (updateResponse.success) {
        console.log(chalk.green('✅ Session timeout updated to'), `${timeoutSeconds} seconds`);
      } else {
        console.log(chalk.red('❌ Failed to update timeout:'), updateResponse.error || 'Unknown error');
      }
    } catch (error) {
      console.log(chalk.red('❌ Failed to update timeout:'), error.message);
    }
  } else {
    console.log(chalk.red('Invalid timeout value. Must be between 1 and 3600 seconds (1 hour).'));
  }
}

async function handleNameCommand(sessionId, newName) {
  const cleanName = newName.replace(/^["']|["']$/g, ''); // Remove surrounding quotes if present
  if (cleanName.length > 0 && cleanName.length <= 100 && /^[a-zA-Z0-9-]+$/.test(cleanName)) {
    try {
      const updateResponse = await api.put(`/sessions/${sessionId}`, {
        name: cleanName
      });
      if (updateResponse.success) {
        console.log(chalk.green('✅ Session name updated to:'), `"${cleanName}"`);
      } else {
        console.log(chalk.red('❌ Failed to update name:'), updateResponse.error || 'Unknown error');
      }
    } catch (error) {
      console.log(chalk.red('❌ Failed to update name:'), error.message);
    }
  } else {
    console.log(chalk.red('Invalid session name'));
    console.log(chalk.red('Error:'), 'Session name must contain only alphanumeric characters and hyphens');
    console.log(chalk.gray('Examples:'), 'my-session, data-analysis, project1, test-run');
  }
}

async function chatLoop(sessionId) {
  const readline = require('readline');
  let currentState = 'idle';
  let isProcessing = false;

  // Set up non-blocking input
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: chalk.cyanBright('User: ')
  });


  // Handle user input
  rl.on('line', async (input) => {
    const userInput = input.trim();
    
    // Clear the last four lines: empty line, top dash line, status line, bottom dash line, user prompt
    process.stdout.write('\x1b[4A\x1b[2K\x1b[0G\x1b[1B\x1b[2K\x1b[0G\x1b[1B\x1b[2K\x1b[0G\x1b[1B\x1b[2K\x1b[0G');
    
    if (!userInput) {
      rl.prompt();
      return;
    }

    // Handle quit command
    if (userInput.toLowerCase() === '/quit' || userInput.toLowerCase() === '/q' || userInput.toLowerCase() === 'exit') {
      console.log(chalk.blue('👋 Ending session. Goodbye!'));
      cleanup();
      return;
    }

    // Handle status command
    if (userInput === '/status') {
      await showSessionStatus(sessionId);
      rl.prompt();
      return;
    }

    // Handle help command
    if (userInput === '/help') {
      showHelp();
      rl.prompt();
      return;
    }

    // Handle timeout commands
    const timeoutMatch = userInput.match(/^(?:\/t|\/timeout|timeout)\s+(\d+)$/);
    if (timeoutMatch) {
      await handleTimeoutCommand(sessionId, parseInt(timeoutMatch[1], 10));
      rl.prompt();
      return;
    }

    // Handle name commands
    const nameMatch = userInput.match(/^(?:\/n|\/name|name)\s+(.+)$/);
    if (nameMatch) {
      await handleNameCommand(sessionId, nameMatch[1]);
      rl.prompt();
      return;
    }

    // Send message to session
    await sendMessage(sessionId, userInput);
    rl.prompt();
  });

  function cleanup() {
    rl.close();
    
    // Close session on exit
    api.post(`/sessions/${sessionId}/close`).catch(() => {
      // Ignore cleanup errors
    });
    
    process.exit(0);
  }

  // Handle cleanup on exit
  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);

  async function sendMessage(sessionId, userInput) {
    isProcessing = true;
    currentState = 'waiting';
    
    // Show the user message
    console.log(chalk.green('User:'), userInput);
    console.log();
    
    // Show status and prompt after user message
    showStatusAndPrompt();

    try {
      const sendResponse = await api.post(`/sessions/${sessionId}/messages`, {
        content: userInput,
        role: 'user'
      });

      if (!sendResponse.success) {
        console.log(chalk.red('❌ Failed to send message:'), sendResponse.error);
        currentState = 'error';
        isProcessing = false;
        return;
      }

      currentState = 'busy';
      
      // Start monitoring for responses with status clearing callback
      await monitorForResponses(sessionId, Date.now(), clearStatusAndPrompt);

    } catch (error) {
      console.log(chalk.red('❌ Error sending message:'), error.message);
      currentState = 'error';
    }
    
    isProcessing = false;
    currentState = 'idle';
  }

  // Initialize the interface
  showStatusAndPrompt();

  // Keep the process alive
  return new Promise((resolve, reject) => {
    rl.on('close', resolve);
  });
}

async function sessionPublishCommand(sessionId, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('📢 Publishing Raworc Session'));
  console.log(chalk.gray('Session:'), sessionId);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);

  // Show publishing permissions (same logic as remix)
  const data = options.data === undefined ? true : (options.data === 'true' || options.data === true);
  const code = options.code === undefined ? true : (options.code === 'true' || options.code === true);
  const secrets = options.secrets === undefined ? true : (options.secrets === 'true' || options.secrets === true);
  
  console.log();
  console.log(chalk.yellow('📋 Remix Permissions:'));
  console.log(chalk.gray('  Data:'), data ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log(chalk.gray('  Code:'), code ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log(chalk.gray('  Secrets:'), secrets ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log();

  try {
    const spinner = ora('Publishing session...').start();

    const publishPayload = {
      data: data,
      code: code,
      secrets: secrets
    };

    const response = await api.post(`/sessions/${sessionId}/publish`, publishPayload);

    if (!response.success) {
      spinner.fail('Failed to publish session');
      console.error(chalk.red('Error:'), response.error);
      process.exit(1);
    }

    spinner.succeed(`Session published: ${sessionId}`);
    
    console.log();
    console.log(chalk.green('🎉 Session is now publicly accessible!'));
    console.log();
    console.log(chalk.blue('📋 Public Access:'));
    console.log(chalk.gray('  • View:'), `raworc api published/sessions/${sessionId}`);
    console.log(chalk.gray('  • List all:'), 'raworc api published/sessions');
    console.log(chalk.gray('  • Remix:'), `raworc session remix ${sessionId}`);
    console.log();

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionUnpublishCommand(sessionId, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🔒 Unpublishing Raworc Session'));
  console.log(chalk.gray('Session:'), sessionId);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);
  console.log();

  try {
    const spinner = ora('Unpublishing session...').start();

    const response = await api.post(`/sessions/${sessionId}/unpublish`);

    if (!response.success) {
      spinner.fail('Failed to unpublish session');
      console.error(chalk.red('Error:'), response.error);
      process.exit(1);
    }

    spinner.succeed(`Session unpublished: ${sessionId}`);
    
    console.log();
    console.log(chalk.green('🔒 Session is now private again'));

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}

async function sessionCloseCommand(sessionId, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🛑 Closing Raworc Session'));
  console.log(chalk.gray('Session:'), sessionId);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);
  console.log();

  try {
    const spinner = ora('Closing session...').start();

    // Get session details first to show current state
    const sessionResponse = await api.get(`/sessions/${sessionId}`);
    
    if (!sessionResponse.success) {
      spinner.fail('Failed to fetch session details');
      console.error(chalk.red('Error:'), sessionResponse.error || 'Session does not exist');
      process.exit(1);
    }

    const session = sessionResponse.data;
    console.log(chalk.gray('Current state:'), session.state);

    // Check if session is already closed
    if (session.state === SESSION_STATE_CLOSED) {
      spinner.succeed('Session is already closed');
      console.log(chalk.yellow('💡 Session was already in closed state'));
      return;
    }

    // Close the session
    const closeResponse = await api.post(`/sessions/${sessionId}/close`);

    if (!closeResponse.success) {
      spinner.fail('Failed to close session');
      console.error(chalk.red('Error:'), closeResponse.error);
      process.exit(1);
    }

    spinner.succeed(`Session closed: ${sessionId}`);
    
    console.log();
    console.log(chalk.green('🛑 Session has been closed and resources cleaned up'));
    console.log();
    console.log(chalk.blue('💡 Session Operations:'));
    console.log(chalk.gray('  • Restore:'), `raworc session restore ${sessionId}`);
    console.log(chalk.gray('  • Remix:'), `raworc session remix ${sessionId}`);
    console.log();

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}