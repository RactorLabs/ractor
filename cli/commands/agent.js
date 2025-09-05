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
  AGENT_STATE_IDLE,
  AGENT_STATE_SLEPT,
  AGENT_STATE_BUSY,
  MESSAGE_ROLE_USER,
  MESSAGE_ROLE_AGENT
} = require('../lib/constants');

module.exports = (program) => {
  const agentCmd = program
    .command('agent')
    .description('Agent management and interactive agents')
    .action(() => {
      agentCmd.help();
    });

  // Default agent create [name]
  agentCmd
    .command('create [name]')
    .description('Create a new interactive agent (name is optional)')
    .option('-S, --secrets <secrets>', 'JSON string of secrets (key-value pairs) for new agents')
    .option('-i, --instructions <text>', 'Direct instructions text')
    .option('-if, --instructions-file <file>', 'Path to instructions file')
    .option('-s, --setup <text>', 'Direct setup script text')
    .option('-sf, --setup-file <file>', 'Path to setup script file')
    .option('-p, --prompt <text>', 'Prompt to send after agent creation')
    .option('-t, --timeout <seconds>', 'Agent timeout in seconds (default: 60)')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent create                    # Create agent with auto-generated name\n' +
      '  $ raworc agent create my-agent          # Create agent with specific name\n' +
      '  $ raworc agent create -S \'{"DB_URL":"postgres://..."}\' # Create with user secrets\n' +
      '  $ raworc agent create my-agent -p "Hello" # Create with name and initial prompt\n' +
      '  $ raworc agent create -t 120            # Create with 2 minute timeout\n')
    .action(async (name, options) => {
      await agentCreateCommand(name, options);
    });

  // Wake subcommand
  agentCmd
    .command('wake <agent-name>')
    .description('Wake an existing agent by name')
    .option('-p, --prompt <text>', 'Prompt to send after waking')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent wake abc123           # Wake by name\n' +
      '  $ raworc agent wake my-agent       # Wake by name\n' +
      '  $ raworc agent wake my-agent -p "Continue work" # Wake with prompt\n')
    .action(async (agentName, options) => {
      await agentWakeCommand(agentName, options);
    });

  // Remix subcommand  
  agentCmd
    .command('remix <agent-name>')
    .description('Create a new agent remixing an existing agent')
    .option('-n, --name [name]', 'Name for the new agent (generates random name if not provided)')
    .option('-c, --code <boolean>', 'Include code files (default: true)')
    .option('-s, --secrets <boolean>', 'Include secrets (default: true)')
    .option('-p, --prompt <text>', 'Prompt to send after creation')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent remix abc123             # Remix with random name\n' +
      '  $ raworc agent remix my-agent         # Remix with random name\n' +
      '  $ raworc agent remix my-agent -n "new-name" # Remix with specific name\n' +
      '  $ raworc agent remix my-agent -s false # Remix without secrets\n' +
      '  $ raworc agent remix my-agent --code false # Copy only secrets\n')
    .action(async (agentName, options) => {
      await agentRemixCommand(agentName, options);
    });

  // Publish subcommand
  agentCmd
    .command('publish <agent-name>')
    .description('Publish a agent for public access')
    .option('-c, --code <boolean>', 'Allow code remix (default: true)')
    .option('-s, --secrets <boolean>', 'Allow secrets remix (default: true)')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent publish abc123           # Publish with all permissions\n' +
      '  $ raworc agent publish my-agent       # Publish by name\n' +
      '  $ raworc agent publish abc123 --secrets false # Publish without secrets remix\n' +
      '  $ raworc agent publish abc123 --secrets false # Only allow code remix\n')
    .action(async (agentName, options) => {
      await agentPublishCommand(agentName, options);
    });

  // Unpublish subcommand
  agentCmd
    .command('unpublish <agent-name>')
    .description('Remove agent from public access')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent unpublish abc123         # Unpublish by name\n' +
      '  $ raworc agent unpublish my-agent     # Unpublish by name\n')
    .action(async (agentName, options) => {
      await agentUnpublishCommand(agentName, options);
    });

  // Open subcommand
  agentCmd
    .command('open <agent-name>')
    .description('Show content links for an agent (private and public if published)')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent open abc123              # Show links by name\n' +
      '  $ raworc agent open my-agent          # Show links by name\n')
    .action(async (agentName, options) => {
      await agentOpenCommand(agentName, options);
    });

  // Sleep subcommand
  agentCmd
    .command('sleep <agent-name>')
    .description('Sleep an active agent')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc agent sleep abc123            # Sleep by name\n' +
      '  $ raworc agent sleep my-agent        # Sleep by name\n')
    .action(async (agentName, options) => {
      await agentSleepCommand(agentName, options);
    });
};

// Generate random agent name client-side
function generateRandomAgentName() {
  const adjectives = [
    'swift', 'bold', 'keen', 'wise', 'calm', 'brave', 'quick', 'smart',
    'bright', 'sharp', 'clear', 'cool', 'warm', 'soft', 'hard', 'fast',
    'slow', 'deep', 'light', 'dark', 'rich', 'pure', 'fresh', 'clean'
  ];

  const nouns = [
    'falcon', 'tiger', 'wolf', 'bear', 'eagle', 'lion', 'fox', 'hawk',
    'shark', 'whale', 'raven', 'robin', 'swift', 'storm', 'river', 'ocean',
    'mountain', 'forest', 'desert', 'valley', 'cloud', 'star', 'moon', 'sun'
  ];

  const adjective = adjectives[Math.floor(Math.random() * adjectives.length)];
  const noun = nouns[Math.floor(Math.random() * nouns.length)];
  const number = Math.floor(Math.random() * 900) + 100; // 100-999

  return `${adjective}-${noun}-${number}`;
}

async function agentCreateCommand(name, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }


  let agentName = null;

  try {
    // Create a new agent

    // Prepare agent creation payload
    const agentPayload = {};

    // Add secrets if provided
    if (options.secrets) {
      try {
        agentPayload.secrets = JSON.parse(options.secrets);
      } catch (error) {
        console.error(chalk.red('✗ Error:'), 'Secrets must be valid JSON');
        process.exit(1);
      }
    }

    // Note: ANTHROPIC_API_KEY is now generated automatically by the system
    // Users can still provide their own API key in secrets for custom code if needed

    // Add instructions if provided
    if (options.instructions) {
      agentPayload.instructions = options.instructions;
    } else if (options.instructionsFile) {
      try {
        const fs = require('fs');
        agentPayload.instructions = fs.readFileSync(options.instructionsFile, 'utf8');
      } catch (error) {
        console.error(chalk.red('✗ Error:'), error.message);
        process.exit(1);
      }
    }

    // Add setup if provided
    if (options.setup) {
      agentPayload.setup = options.setup;
    } else if (options.setupFile) {
      try {
        const fs = require('fs');
        agentPayload.setup = fs.readFileSync(options.setupFile, 'utf8');
      } catch (error) {
        console.error(chalk.red('✗ Error:'), error.message);
        process.exit(1);
      }
    }

    // Add prompt if provided
    if (options.prompt) {
      agentPayload.prompt = options.prompt;
    }

    // Add name if provided (positional parameter), generate random name if not
    if (name) {
      // Validate name format
      if (name.length === 0 || name.length > 100) {
        console.error(chalk.red('✗ Error:'), 'Agent name must be 1-100 characters long');
        process.exit(1);
      }
      if (!/^[a-zA-Z0-9-]+$/.test(name)) {
        console.error(chalk.red('✗ Error:'), 'Agent name must contain only alphanumeric characters and hyphens');
        console.log(chalk.gray('Examples:'), 'my-agent, data-analysis, project1, test-run');
        process.exit(1);
      }
      agentPayload.name = name;
    } else {
      // Generate random name client-side
      agentPayload.name = generateRandomAgentName();
    }

    // Add timeout if provided
    if (options.timeout) {
      const timeoutSeconds = parseInt(options.timeout);
      if (isNaN(timeoutSeconds) || timeoutSeconds <= 0) {
        console.error(chalk.red('✗ Error:'), 'Timeout must be a positive number in seconds');
        process.exit(1);
      }
      agentPayload.timeout_seconds = timeoutSeconds;
    }

    const createResponse = await api.post('/agents', agentPayload);

    if (!createResponse.success) {
      console.error(chalk.red('✗ Error:'), createResponse.error);

      if (createResponse.status === 400) {
        console.log();
        console.log(chalk.yellow('ℹ') + ' Check if your agent parameters are valid');
      }

      process.exit(1);
    }

    agentName = createResponse.data.name;

    await startInteractiveAgent(agentName, options);

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function agentWakeCommand(agentName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }


  try {

    // Get agent details first
    const agentResponse = await api.get(`/agents/${agentName}`);

    if (!agentResponse.success) {
      console.error(chalk.red('✗ Error:'), agentResponse.error || 'Agent does not exist');
      process.exit(1);
    }

    const agent = agentResponse.data;

    // Update agentName to actual name for consistent display
    agentName = agent.name;

    // Handle different agent states
    if (agent.state === AGENT_STATE_SLEPT) {
      const wakePayload = {};
      if (options.prompt) {
        wakePayload.prompt = options.prompt;
      }

      const wakeResponse = await api.post(`/agents/${agentName}/wake`, wakePayload);

      if (!wakeResponse.success) {
        console.error(chalk.red('✗ Error:'), wakeResponse.error);
        process.exit(1);
      }

    } else if (agent.state === AGENT_STATE_IDLE) {

      // If prompt provided for already-running agent, send it as a message
      if (options.prompt) {
        console.log(chalk.blue('Sending prompt to running agent:'), options.prompt);
        try {
          const messageResponse = await api.post(`/agents/${agentName}/messages`, {
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
    } else if (agent.state === AGENT_STATE_BUSY) {
      console.log(chalk.yellow('ℹ') + ' Agent is currently processing. You can observe ongoing activity.');
      console.log();
    } else {
      process.exit(1);
    }

    await startInteractiveAgent(agentName, { ...options, isRestore: true, agentState: agent.state });

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function agentRemixCommand(sourceAgentName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }


  try {
    // Get agent details first
    const agentResponse = await api.get(`/agents/${sourceAgentName}`);

    if (!agentResponse.success) {
      console.error(chalk.red('✗ Error:'), agentResponse.error || 'Agent does not exist');
      process.exit(1);
    }

    const sourceAgent = agentResponse.data;
    // Update agentName to actual name for consistent display
    sourceAgentName = sourceAgent.name;

    // Prepare remix payload
    const remixPayload = {};

    if (options.code !== undefined) {
      remixPayload.code = options.code === 'true' || options.code === true;
    }

    if (options.secrets !== undefined) {
      remixPayload.secrets = options.secrets === 'true' || options.secrets === true;
    }

    // Content is always included by default
    remixPayload.content = true;

    // Add prompt if provided
    if (options.prompt) {
      remixPayload.prompt = options.prompt;
    }

    // Add name if provided, or generate random name
    if (options.name) {
      // Validate name format
      if (options.name.length === 0 || options.name.length > 100) {
        console.error(chalk.red('✗ Error:'), 'Agent name must be 1-100 characters long');
        process.exit(1);
      }
      if (!/^[a-zA-Z0-9-]+$/.test(options.name)) {
        console.error(chalk.red('✗ Error:'), 'Agent name must contain only alphanumeric characters and hyphens');
        console.log(chalk.gray('Examples:'), 'my-agent, data-analysis, project1, test-run');
        process.exit(1);
      }
      remixPayload.name = options.name;
    } else {
      // Generate random name client-side
      remixPayload.name = generateRandomAgentName();
    }

    // Create remix agent
    const remixResponse = await api.post(`/agents/${sourceAgentName}/remix`, remixPayload);

    if (!remixResponse.success) {
      console.error(chalk.red('✗ Error:'), remixResponse.error);
      process.exit(1);
    }

    const agentName = remixResponse.data.name;
    const newAgent = remixResponse.data;

    // Show detailed remix success info
    if (newAgent.name) {
      // Success feedback is provided by the command box display
    }


    await startInteractiveAgent(agentName, { ...options, sourceAgentName: sourceAgentName });

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function showAgentBox(agentName, mode, user, source = null) {
  // Create descriptive title based on mode
  const modeIcons = {
    'New': `${display.icons.agent} Agent Start`,
    'Restore': `${display.icons.agent} Agent Restore`, 
    'Remix': `${display.icons.agent} Agent Remix`
  };
  
  const title = modeIcons[mode] || `${display.icons.agent} Agent`;
  const commands = '/help (for commands)';
  
  // Build base lines
  const lines = [
    `Agent: ${agentName}`,
    source ? `Source: ${source}` : null,
    `User: ${user}`,
    `Commands: ${commands}`
  ].filter(line => line !== null);
  
  // Try to get Content URL from agent info
  try {
    const agentResponse = await api.get(`/agents/${agentName}`);
    if (agentResponse.success && agentResponse.data && agentResponse.data.content_port) {
      // Extract hostname from server URL instead of hardcoding localhost
      const serverUrl = config.getServerUrl();
      const serverHostname = new URL(serverUrl).hostname;
      const contentUrl = `http://${serverHostname}:${agentResponse.data.content_port}/`;
      lines.splice(-1, 0, `Content: ${contentUrl}`); // Insert before Commands line
    }
  } catch (error) {
    // Continue without Content URL if API call fails
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

async function startInteractiveAgent(agentName, options) {
  // Get user info and determine mode
  const authData = config.getAuth();
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  const user = userName + userType;
  
  let mode = 'New';
  let source = null;
  
  if (options.isRestore) {
    mode = 'Restore';
  } else if (options.sourceAgentName) {
    mode = 'Remix';
    source = options.sourceAgentName;
  }
  
  await showAgentBox(agentName, mode, user, source);

  // Show recent conversation history for restored agents
  if (options.isRestore) {
    try {
      const messagesResponse = await api.get(`/agents/${agentName}/messages`);

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
            // Agent messages: show content without "> " prefix, just normal text
            const lines = content.split('\n');
            lines.forEach(line => {
              console.log(line);
            });
          }
        });

        console.log();
        console.log(chalk.gray('─'.repeat(getTerminalWidth())));

      } else {
      }
    } catch (error) {
      console.log(chalk.yellow('Warning: Could not load conversation history'));
    }
  }

  console.log();

  // Handle prompt if provided (for any agent type)
  if (options.prompt) {
    console.log(chalk.green('> ') + chalk.white(options.prompt));

    // Create comprehensive prompt manager
    const promptManager = createPromptManager(agentName);
    await promptManager.show();
    promptManager.startMonitoring();
    
    try {
      // Send the prompt message to the API
      const sendTime = Date.now();
      const sendResponse = await api.post(`/agents/${agentName}/messages`, {
        content: options.prompt,
        role: 'user'
      });
      
      if (!sendResponse.success) {
        console.log(chalk.yellow('Warning: Failed to send prompt:'), sendResponse.error);
        promptManager.stopMonitoring();
        return;
      }
      
      // Wait for all agent responses to the prompt (tool calls + final response)
      await waitForAllAgentResponses(agentName, sendTime, 60000, {
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

  // If connecting to a busy agent, start monitoring for ongoing activity
  if (options.agentState === AGENT_STATE_BUSY) {
    console.log(chalk.blue('ℹ') + ' Monitoring ongoing agent activity...');
    console.log();

    // Start monitoring without a user message time (will show any new messages)
    const monitoringPromise = monitorForResponses(agentName, 0);

    // Start chat loop concurrently so user can still interact
    const chatPromise = chatLoop(agentName, options);

    // Wait for either to complete (though monitoring should complete when agent finishes)
    await Promise.race([monitoringPromise, chatPromise]);
  } else {
    // Start synchronous chat loop - don't skip initial prompt, let chatLoop show the correct state
    await chatLoop(agentName, { ...options, skipInitialPrompt: false });
  }
}

async function waitForAgentResponse(agentName, userMessageTime, timeoutMs = 60000) {
  const startTime = Date.now();
  const pollInterval = 1500; // Check every 1.5 seconds
  let lastCheckedCount = 0;

  // Get initial message count to detect new messages
  try {
    const initialResponse = await api.get(`/agents/${agentName}/messages`);
    if (initialResponse.success && initialResponse.data) {
      const messages = Array.isArray(initialResponse.data) ? initialResponse.data : initialResponse.data.messages || [];
      lastCheckedCount = messages.length;
    }
  } catch (error) {
    // Continue with 0 count
  }

  while (Date.now() - startTime < timeoutMs) {
    try {
      const response = await api.get(`/agents/${agentName}/messages`);

      if (response.success && response.data) {
        const messages = Array.isArray(response.data) ? response.data : response.data.messages || [];

        // Check if we have new messages
        if (messages.length > lastCheckedCount) {
          // Look for the newest agent message that was created after our user message
          for (let i = messages.length - 1; i >= 0; i--) {
            const message = messages[i];
            if (message.role === MESSAGE_ROLE_AGENT) {
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

  throw new Error('Timeout waiting for agent response');
}

// Shared function to display agent messages (tool calls + final response)
function displayAgentMessage(message, options = {}) {
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

async function waitForAllAgentResponses(agentName, userMessageTime, timeoutMs = 60000, promptOptions = {}) {
  const startTime = Date.now();
  const pollInterval = 1500; // Check every 1.5 seconds
  let lastMessageCount = 0;
  let foundFinalResponse = false;

  // Get initial message count
  try {
    const initialResponse = await api.get(`/agents/${agentName}/messages`);
    if (initialResponse.success && initialResponse.data) {
      const messages = Array.isArray(initialResponse.data) ? initialResponse.data : initialResponse.data.messages || [];
      lastMessageCount = messages.length;
    }
  } catch (error) {
    // Continue with 0 count
  }

  while (Date.now() - startTime < timeoutMs && !foundFinalResponse) {
    try {
      const response = await api.get(`/agents/${agentName}/messages`);

      if (response.success && response.data) {
        const messages = Array.isArray(response.data) ? response.data : response.data.messages || [];
        
        // Process any new messages
        if (messages.length > lastMessageCount) {
          const newMessages = messages.slice(lastMessageCount);
          
          for (const message of newMessages) {
            if (message.role === MESSAGE_ROLE_AGENT) {
              const messageTime = new Date(message.created_at).getTime();
              if (messageTime > userMessageTime) {
                const result = displayAgentMessage(message, {
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
function createPromptManager(agentName, userInput = '') {
  let currentState = 'init';
  let currentUserInput = userInput;
  let promptVisible = false;
  let stateMonitorInterval = null;
  let dotAnimationInterval = null;
  let isRestoringFromClosed = false;
  
  const updateState = async () => {
    try {
      const agentResponse = await api.get(`/agents/${agentName}`);
      if (agentResponse.success) {
        const newState = agentResponse.data.state;
        
        // Special handling for agents being restored from slept state
        if (isRestoringFromClosed) {
          // Stay in 'init' until server confirms agent is ready (idle or busy)
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
    
    // Monitor agent state changes every 2 seconds
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

// Get terminal width with fallback
function getTerminalWidth() {
  return process.stdout.columns || 80;
}

function showPrompt(state = 'init') {
  const stateIcons = {
    'init': '◯',      // empty circle - initializing
    'idle': '●',      // solid circle - ready
    'busy': '◐',      // half circle - working
    'slept': '◻',    // empty square - slept/slept
    'deleted': '◼'    // filled square - deleted
  };

  const stateLabels = {
    'init': 'initializing',
    'idle': 'idle',
    'busy': 'working',
    'slept': 'slept',
    'deleted': 'deleted'
  };
  
  const stateColors = {
    'init': chalk.blue,
    'idle': chalk.green,
    'busy': chalk.yellow,
    'slept': chalk.cyan,     // brighter than gray
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
  console.log(chalk.gray('─'.repeat(getTerminalWidth())));
  process.stdout.write(chalk.cyanBright('> '));
}

function showPromptWithInput(state = 'init', userInput = '') {
  const stateIcons = {
    'init': '◯',      // empty circle - initializing
    'idle': '●',      // solid circle - ready
    'busy': '◐',      // half circle - working
    'slept': '◻',    // empty square - slept/slept
    'deleted': '◼'    // filled square - deleted
  };

  const stateLabels = {
    'init': 'initializing',
    'idle': 'idle',
    'busy': 'working',
    'slept': 'slept',
    'deleted': 'deleted'
  };
  
  const stateColors = {
    'init': chalk.blue,
    'idle': chalk.green,
    'busy': chalk.yellow,
    'slept': chalk.cyan,     // brighter than gray
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
  console.log(chalk.gray('─'.repeat(getTerminalWidth())));
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

async function monitorForResponses(agentName, userMessageTime, getCurrentState, updateState, getPromptVisible, setPromptVisible) {
  let lastMessageCount = 0;

  try {
    const initialResponse = await api.get(`/agents/${agentName}/messages`);
    if (initialResponse.success) {
      lastMessageCount = initialResponse.data.length;
    }
  } catch (error) {
    return;
  }

  while (true) {
    try {
      const response = await api.get(`/agents/${agentName}/messages`);
      if (response.success && response.data.length > lastMessageCount) {
        const newMessages = response.data.slice(lastMessageCount);

        for (const message of newMessages) {
          if (message.role === 'agent') {
            const result = displayAgentMessage(message, {
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

async function chatLoop(agentName, options = {}) {
  const readline = require('readline');
  // For restored agents from slept state, start with 'init' and wait for server to confirm ready
  // For new agents, start with 'init' 
  // For other cases, use current agent state
  let currentAgentState = 'init';
  let currentUserInput = '';
  let promptVisible = false; // Track if prompt is currently displayed
  let isRestoringFromClosed = options.isRestore && options.agentState === AGENT_STATE_SLEPT;

  // Function to fetch and update agent state
  async function updateAgentState() {
    try {
      const agentResponse = await api.get(`/agents/${agentName}`);
      if (agentResponse.success) {
        const newState = agentResponse.data.state;
        
        // Special handling for agents being restored from slept state
        if (isRestoringFromClosed) {
          // Stay in 'init' until server confirms agent is ready (idle or busy)
          if (newState === 'idle' || newState === 'busy') {
            isRestoringFromClosed = false; // Clear the flag
            currentAgentState = newState;
          }
          // Otherwise keep showing 'init' state
        } else {
          // Normal state transitions
          if (newState !== currentAgentState) {
            currentAgentState = newState;
          }
        }
        
        // Only redraw if prompt is currently visible
        if (promptVisible) {
          clearPrompt();
          showPromptWithInput(currentAgentState, currentUserInput);
        }
        
        return currentAgentState;
      }
    } catch (error) {
      // Keep current state if API fails
    }
    return currentAgentState;
  }

  // Get initial agent state
  await updateAgentState();

  // Enable keypress events
  readline.emitKeypressEvents(process.stdin);
  if (process.stdin.setRawMode) {
    process.stdin.setRawMode(true);
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  // Monitor agent state changes every 2 seconds
  const stateMonitorInterval = setInterval(updateAgentState, 2000);
  
  // Animation interval for dots (every 500ms)
  const dotAnimationInterval = setInterval(() => {
    if (promptVisible && (currentAgentState === 'init' || currentAgentState === 'busy')) {
      clearPrompt();
      showPromptWithInput(currentAgentState, currentUserInput);
    }
  }, 500);

  // Only show initial prompt if not skipping it
  if (!options.skipInitialPrompt) {
    showPrompt(currentAgentState);
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
      showPrompt(currentAgentState);
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

    // Handle detach command - exit interactive mode without closing agent
    if (userInput.toLowerCase() === '/detach' || userInput.toLowerCase() === '/d') {
      clearPrompt();
      promptVisible = false;
      console.log(chalk.green('◊ Detached from agent. Agent continues running.'));
      console.log(chalk.gray('Reconnect with: ') + chalk.white(`raworc agent wake ${agentName}`));
      process.exit(0);
    }

    // Handle commands - don't return early, just set a flag
    let shouldSendMessage = true;
    
    // Handle status command
    if (userInput === '/status') {
      clearPrompt(); // Short command, no enter to clear
      promptVisible = false;
      await showAgentStatus(agentName);
      shouldSendMessage = false;
    }
    // Handle help command
    else if (userInput === '/help' || userInput === '/h') {
      clearPrompt(); // Short command, no enter to clear
      promptVisible = false;
      showHelp();
      shouldSendMessage = false;
    }
    // Handle sleep command
    else if (userInput === '/sleep' || userInput === '/s') {
      clearPrompt();
      promptVisible = false;
      await handleSleepCommand(agentName);
      shouldSendMessage = false;
    }
    // Handle wake command
    else if (userInput === '/wake' || userInput === '/w') {
      clearPrompt();
      promptVisible = false;
      await handleWakeCommand(agentName);
      shouldSendMessage = false;
    }
    // Handle open command
    else if (userInput === '/open' || userInput === '/o') {
      clearPrompt();
      promptVisible = false;
      await handleOpenCommand(agentName);
      shouldSendMessage = false;
    }
    // Handle publish command
    else if (userInput === '/publish' || userInput === '/p') {
      clearPrompt();
      promptVisible = false;
      await handlePublishCommand(agentName);
      shouldSendMessage = false;
    }
    // Handle timeout commands
    else {
      const timeoutMatch = userInput.match(/^(?:\/t|\/timeout|timeout)\s+(\d+)$/);
      if (timeoutMatch) {
        clearPrompt(); // Short command, no enter to clear
        promptVisible = false;
        await handleTimeoutCommand(agentName, parseInt(timeoutMatch[1], 10));
        shouldSendMessage = false;
      }
    }
    
    // Show prompt after command execution or send message
    if (!shouldSendMessage) {
      showPrompt(currentAgentState);
      promptVisible = true;
    } else {
      // Send message to agent - clear with enter since it's regular input
      clearPrompt();
      promptVisible = false;
      await sendMessage(agentName, userInput);
    }
  });

  async function cleanup() {
    clearInterval(stateMonitorInterval);
    clearInterval(dotAnimationInterval);
    rl.close();
    
    // Close the agent on the server silently
    try {
      await api.post(`/agents/${agentName}/sleep`);
    } catch (error) {
      // Ignore all errors during cleanup
    }
    
    console.log();
    console.log(chalk.cyan('Goodbye! 👋'));
    
    process.exit(0);
  }

  process.on('SIGINT', () => cleanup());
  process.on('SIGTERM', () => cleanup());

  async function sendMessage(agentName, userInput) {
    console.log(chalk.green('> ') + chalk.white(userInput));
    
    // Show prompt with current actual state
    showPrompt(currentAgentState);
    promptVisible = true;

    try {
      const sendResponse = await api.post(`/agents/${agentName}/messages`, {
        content: userInput,
        role: 'user'
      });

      if (!sendResponse.success) {
        clearPrompt();
        promptVisible = false;
        console.log(chalk.red('✗ Failed to send message:'), sendResponse.error);
        // Update state from server after error
        await updateAgentState();
        showPrompt(currentAgentState);
        promptVisible = true;
        return;
      }

      await monitorForResponses(agentName, Date.now(), () => currentAgentState, updateAgentState, () => promptVisible, (visible) => { promptVisible = visible; });

    } catch (error) {
      clearPrompt();
      promptVisible = false;
      console.log(chalk.red('✗ Error sending message:'), error.message);
      // Update state from server after error
      await updateAgentState();
      showPrompt(currentAgentState);
      promptVisible = true;
    }
  }

  return new Promise((resolve) => {
    rl.on('close', resolve);
  });
}

async function showAgentStatus(agentName) {
  try {
    const statusResponse = await api.get(`/agents/${agentName}`);
    if (statusResponse.success) {
      console.log();
      console.log(chalk.blue('ℹ') + ' Agent Status:');
      console.log(chalk.gray('  Name:'), statusResponse.data.name || 'Unnamed');
      console.log(chalk.gray('  State:'), getStateDisplay(statusResponse.data.state));
      console.log(chalk.gray('  Created:'), new Date(statusResponse.data.created_at).toLocaleString());
      console.log(chalk.gray('  Updated:'), new Date(statusResponse.data.updated_at).toLocaleString());
      console.log();
    } else {
      console.log(chalk.red('✗ Failed to get agent status:'), statusResponse.error);
    }
  } catch (error) {
    console.log(chalk.red('✗ Error getting agent status:'), error.message);
  }
}

function showHelp() {
  console.log(chalk.blue('ℹ') + ' Available Commands:');
  console.log(chalk.gray('  /help, /h   '), 'Show this help message');
  console.log(chalk.gray('  /status     '), 'Show agent status');
  console.log(chalk.gray('  /timeout <s>'), 'Change agent timeout (1-3600 seconds)');
  console.log(chalk.gray('  /name <name>'), 'Change agent name (alphanumeric and hyphens)');
  console.log(chalk.gray('  /sleep, /s  '), 'Sleep the agent');
  console.log(chalk.gray('  /wake, /w   '), 'Wake the agent');
  console.log(chalk.gray('  /open, /o   '), 'Show agent content URLs');
  console.log(chalk.gray('  /publish, /p'), 'Publish agent to public directory');
  console.log(chalk.gray('  /detach, /d '), 'Detach from agent (keeps agent running)');
  console.log(chalk.gray('  /quit, /q   '), 'End the agent');
}

async function handleTimeoutCommand(agentName, timeoutSeconds) {
  if (timeoutSeconds >= 1 && timeoutSeconds <= 3600) {
    try {
      const updateResponse = await api.put(`/agents/${agentName}`, {
        timeout_seconds: timeoutSeconds
      });
      if (updateResponse.success) {
        console.log(chalk.green('✓') + ` Agent timeout updated to ${timeoutSeconds} seconds`);
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

async function handleSleepCommand(agentName) {
  try {
    const sleepResponse = await api.post(`/agents/${agentName}/sleep`);
    if (sleepResponse.success) {
      console.log(chalk.green('✓') + ` Agent ${agentName} put to sleep`);
    } else {
      console.log(chalk.red('✗ Failed to sleep agent:'), sleepResponse.error || 'Unknown error');
    }
  } catch (error) {
    console.log(chalk.red('✗ Failed to sleep agent:'), error.message);
  }
}

async function handleWakeCommand(agentName) {
  try {
    const wakeResponse = await api.post(`/agents/${agentName}/wake`, {});
    if (wakeResponse.success) {
      console.log(chalk.green('✓') + ` Agent ${agentName} woken up`);
    } else {
      console.log(chalk.red('✗ Failed to wake agent:'), wakeResponse.error || 'Unknown error');
    }
  } catch (error) {
    console.log(chalk.red('✗ Failed to wake agent:'), error.message);
  }
}

async function handleOpenCommand(agentName) {
  try {
    const agentResponse = await api.get(`/agents/${agentName}`);
    if (agentResponse.success) {
      const agent = agentResponse.data;
      const configData = config.getConfig();
      const serverUrl = configData.server || 'http://localhost:9000';
      const serverUrlObj = new URL(serverUrl);
      const serverHost = serverUrlObj.hostname;

      console.log(chalk.blue('ℹ') + ` Agent Content URLs:`);
      
      if (agent.content_port) {
        console.log(chalk.gray('  Private:  ') + chalk.blue(`http://${serverHost}:${agent.content_port}/`));
      } else {
        console.log(chalk.gray('  Private:  ') + chalk.yellow('Not available'));
      }
      
      if (agent.is_published) {
        console.log(chalk.gray('  Public:   ') + chalk.blue(`http://${serverHost}:8000/${agent.name}/`));
      } else {
        console.log(chalk.gray('  Public:   ') + chalk.yellow('Not published'));
      }
      
      if (agent.state) {
        console.log(chalk.gray('  Status:   ') + getStateDisplay(agent.state));
      }
    } else {
      console.log(chalk.red('✗ Failed to get agent info:'), agentResponse.error || 'Unknown error');
    }
  } catch (error) {
    console.log(chalk.red('✗ Failed to get agent info:'), error.message);
  }
}

async function handlePublishCommand(agentName) {
  try {
    const publishResponse = await api.post(`/agents/${agentName}/publish`, {
      content: true
    });
    if (publishResponse.success) {
      console.log(chalk.green('✓') + ` Agent ${agentName} published to public directory`);
      
      // Show the public URL
      const configData = config.getConfig();
      const serverUrl = configData.server || 'http://localhost:9000';
      const serverUrlObj = new URL(serverUrl);
      const serverHost = serverUrlObj.hostname;
      
      console.log(chalk.gray('  Public URL: ') + chalk.blue(`http://${serverHost}:8000/${agentName}/`));
    } else {
      console.log(chalk.red('✗ Failed to publish agent:'), publishResponse.error || 'Unknown error');
    }
  } catch (error) {
    console.log(chalk.red('✗ Failed to publish agent:'), error.message);
  }
}


async function agentPublishCommand(agentName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('📢 Publishing Raworc Agent'));
  console.log(chalk.gray('Agent:'), agentName);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);

  // Show publishing permissions (same logic as remix)
  const code = options.code === undefined ? true : (options.code === 'true' || options.code === true);
  const secrets = options.secrets === undefined ? true : (options.secrets === 'true' || options.secrets === true);
  // Content is always allowed by default
  const content = true;

  console.log();
  console.log(chalk.yellow('📋 Remix Permissions:'));
  console.log(chalk.gray('  Code:'), code ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log(chalk.gray('  Secrets:'), secrets ? chalk.green('✓ Allowed') : chalk.red('✗ Blocked'));
  console.log(chalk.gray('  Content:'), chalk.green('✓ Allowed'));
  console.log();

  try {

    const publishPayload = {
      code: code,
      secrets: secrets,
      content: content
    };

    const response = await api.post(`/agents/${agentName}/publish`, publishPayload);

    if (!response.success) {
      console.error(chalk.red('✗ Error:'), response.error);
      process.exit(1);
    }

    console.log(chalk.green('✓') + ` Agent published: ${agentName}`);

    console.log();
    console.log(chalk.green('✓') + ' Agent is now publicly accessible!');
    console.log();
    console.log(chalk.blue('📋 Public Access:'));
    console.log(chalk.gray('  • View:'), `raworc api published/agents/${agentName}`);
    console.log(chalk.gray('  • List all:'), 'raworc api published/agents');
    console.log(chalk.gray('  • Remix:'), `raworc agent remix ${agentName}`);
    console.log();

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function agentUnpublishCommand(agentName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🔒 Unpublishing Raworc Agent'));
  console.log(chalk.gray('Agent:'), agentName);
  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);
  console.log();

  try {

    const response = await api.post(`/agents/${agentName}/unpublish`);

    if (!response.success) {
      console.error(chalk.red('✗ Error:'), response.error);
      process.exit(1);
    }

    console.log(chalk.green('✓') + ` Agent unpublished: ${agentName}`);

    console.log();
    console.log(chalk.green('✓') + ' Agent is now private again');

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function agentSleepCommand(agentName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  display.showCommandBox(`${display.icons.stop} Agent Close`, {
    agent: agentName,
    operation: 'Close and cleanup resources'
  });

  try {
    // Get agent details first (to show current state)
    const agentResponse = await api.get(`/agents/${agentName}`);

    if (!agentResponse.success) {
      console.error(chalk.red('✗ Error:'), agentResponse.error || 'Agent does not exist');
      process.exit(1);
    }

    const agent = agentResponse.data;
    // Update agentName to actual name for consistent display
    agentName = agent.name;
console.log(chalk.gray('Current state:'), getStateDisplay(agent.state));

    // Check if agent is already slept
    if (agent.state === AGENT_STATE_SLEPT) {
      display.info('Agent was already in slept state');
      return;
    }

    // Close the agent
    const sleepResponse = await api.post(`/agents/${agentName}/sleep`);

    if (!sleepResponse.success) {
      display.error('Sleep failed: ' + sleepResponse.error);
      process.exit(1);
    }

    display.success(`Agent slept: ${agentName}`);

    console.log();
    display.success('Agent has been put to sleep and resources cleaned up');
    console.log();
    display.info('Agent Operations:');
    console.log(chalk.gray('  • Restore:'), `raworc agent wake ${agentName}`);
    console.log(chalk.gray('  • Remix:'), `raworc agent remix ${agentName}`);
    console.log();

  } catch (error) {
    console.error(chalk.red('✗ Error:'), error.message);
    process.exit(1);
  }
}

async function agentOpenCommand(agentName, options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('✗ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  // Get server configuration
  const configData = config.getConfig();
  const serverUrl = configData.server || 'http://localhost:9000';

  display.showCommandBox(`${display.icons.agent} Agent Links`, {
    agent: agentName,
    operation: 'Show content access links'
  });

  try {
    // Get agent details
    const agentResponse = await api.get(`/agents/${agentName}`);

    if (!agentResponse.success) {
      console.error(chalk.red('✗ Error:'), agentResponse.error || 'Agent does not exist');
      process.exit(1);
    }

    const agent = agentResponse.data;
    
    // Extract hostname from server URL for building content URLs
    const serverUrlObj = new URL(serverUrl);
    const serverHost = serverUrlObj.hostname;
    
    console.log();
    console.log(chalk.bold('Agent Information:'));
    console.log(`  Name: ${chalk.cyan(agent.name)}`);
    console.log(`  State: ${getStateDisplay(agent.state)}`);
    console.log(`  Created: ${new Date(agent.created_at).toLocaleString()}`);
    
    console.log();
    console.log(chalk.bold('Content Access:'));
    
    // Private content link (always available if agent has content_port)
    if (agent.content_port) {
      console.log(chalk.gray('  • Private Content:'));
      console.log(`    ${chalk.blue(`http://${serverHost}:${agent.content_port}/`)}`);
      console.log(chalk.gray('    (Direct access to agent\'s content directory)'));
    } else {
      console.log(chalk.gray('  • Private Content: Not available (agent may be sleeping)'));
    }
    
    console.log();
    
    // Public content link (only if published)
    if (agent.is_published) {
      console.log(chalk.gray('  • Public Content:'));
      console.log(`    ${chalk.blue(`http://${serverHost}:8000/${agent.name}/`)}`);
      console.log(chalk.gray('    (Published content available to everyone)'));
      
      if (agent.published_at) {
        console.log(`    Published: ${new Date(agent.published_at).toLocaleString()}`);
      }
    } else {
      console.log(chalk.gray('  • Public Content: Not published'));
      console.log(`    To publish: ${chalk.white(`raworc agent publish ${agent.name}`)}`);
    }
    
    console.log();
    
    // Additional info based on state
    if (agent.state === 'slept') {
      console.log(chalk.yellow('ℹ Agent is sleeping. To access private content:'));
      console.log(`  ${chalk.white(`raworc agent wake ${agent.name}`)}`);
    } else if (agent.state === 'init') {
      console.log(chalk.yellow('ℹ Agent is initializing. Content may not be ready yet.'));
    }

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
    'slept': '◻',    // empty square - slept/slept
    'deleted': '◼'    // filled square - deleted
  };

  const stateColors = {
    'init': chalk.blue,
    'idle': chalk.green,
    'busy': chalk.yellow, 
    'slept': chalk.cyan,     // brighter than gray
    'deleted': chalk.magenta
  };
  
  const icon = stateIcons[state] || '◯';
  const color = stateColors[state] || chalk.gray;
  return color(`${icon} ${state}`);
}