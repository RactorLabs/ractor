const chalk = require('chalk');
const inquirer = require('inquirer');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');
const { 
  SESSION_STATE_IDLE, 
  SESSION_STATE_BUSY, 
  SESSION_STATE_CLOSED,
  MESSAGE_ROLE_USER,
  MESSAGE_ROLE_AGENT
} = require('../lib/constants');

module.exports = (program) => {
  program
    .command('session')
    .description('Start an interactive AI agent session')
    .option('--restore <session-id>', 'Restore an existing session by ID')
    .option('--remix <session-id>', 'Create a new session remixing an existing session')
    .option('--data <boolean>', 'Include data files in remix (default: true)')
    .option('--code <boolean>', 'Include code files in remix (default: true)')
    .option('--secrets <secrets>', 'JSON string of secrets (key-value pairs) or "false" to exclude in remix')
    .option('--instructions <file>', 'Path to instructions file')
    .option('--setup <file>', 'Path to setup script file')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc session                           # Start a new session\n' +
      '  $ raworc session --restore abc123         # Restore and continue existing session\n' +
      '  $ raworc session --remix abc123           # Create new session based on existing one\n' +
      '  $ raworc session --remix abc123 --data false # Remix without copying data files\n' +
      '  $ raworc session --remix abc123 --secrets false # Remix without copying secrets\n' +
      '  $ raworc session --secrets \'{"API_KEY":"sk-123"}\' # Create session with secrets\n' +
      '  $ raworc session --instructions ./inst.md # Create session with instructions\n' +
      '  $ raworc session --setup ./setup.sh       # Create session with setup script\n')
    .action(async (options) => {
      await sessionCommand(options);
    });
};

async function sessionCommand(options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc auth login') + ' to authenticate first');
    process.exit(1);
  }

  console.log(chalk.blue('🤖 Starting Raworc AI Session'));
  if (options.remix) {
    console.log(chalk.gray('Mode:'), 'Remix');
    console.log(chalk.gray('Source:'), options.remix);
    
    // Show remix parameters if specified
    if (options.data !== undefined) {
      console.log(chalk.gray('Copy Data:'), options.data === 'true' || options.data === true ? 'Yes' : 'No');
    }
    if (options.code !== undefined) {
      console.log(chalk.gray('Copy Code:'), options.code === 'true' || options.code === true ? 'Yes' : 'No');
    }
    if (options.secrets !== undefined) {
      if (options.secrets === 'false') {
        console.log(chalk.gray('Copy Secrets:'), 'No');
      } else {
        console.log(chalk.gray('Copy Secrets:'), options.secrets === 'true' || options.secrets === true ? 'Yes' : 'No');
      }
    }
  } else if (options.restore) {
    console.log(chalk.gray('Mode:'), 'Restore');
    console.log(chalk.gray('Session:'), options.restore);
  } else {
    console.log(chalk.gray('Mode:'), 'New Session');
  }
  console.log(chalk.gray('User:'), authData.user?.username || 'Unknown');
  
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
  console.log();

  let sessionId = null;
  
  try {
    // Check if we're remixing an existing session
    if (options.remix) {
      const sourceSessionId = options.remix;
      const spinner = ora('Remixing session...').start();
      
      // Check if source session exists
      const sourceSessionResponse = await api.get(`/sessions/${sourceSessionId}`);
      
      if (!sourceSessionResponse.success) {
        spinner.fail('Source session not found');
        console.error(chalk.red('Error:'), sourceSessionResponse.error || 'Source session does not exist');
        process.exit(1);
      }
      
      // Prepare remix payload with selective parameters
      const remixPayload = {
        metadata: {
          remixed_from: sourceSessionId,
          remix_timestamp: new Date().toISOString()
        }
      };
      
      // Parse selective copy parameters
      if (options.data !== undefined) {
        remixPayload.data = options.data === 'true' || options.data === true;
      }
      
      if (options.code !== undefined) {
        remixPayload.code = options.code === 'true' || options.code === true;
      }
      
      if (options.secrets !== undefined) {
        if (options.secrets === 'false') {
          remixPayload.secrets = false;
        } else {
          // For remix, secrets parameter controls copying, not providing new secrets
          remixPayload.secrets = options.secrets === 'true' || options.secrets === true;
        }
      }
      
      // Create remix session
      const remixResponse = await api.post(`/sessions/${sourceSessionId}/remix`, remixPayload);
      
      if (!remixResponse.success) {
        spinner.fail('Failed to remix session');
        console.error(chalk.red('Error:'), remixResponse.error);
        process.exit(1);
      }
      
      sessionId = remixResponse.data.id;
      spinner.succeed(`Session remixed: ${sessionId}`);
      
    } else if (options.restore) {
      // Check if we're restoring an existing session
      sessionId = options.restore;
      const spinner = ora('Restoring session...').start();
      
      // Check if session exists
      const sessionResponse = await api.get(`/sessions/${sessionId}`);
      
      if (!sessionResponse.success) {
        spinner.fail('Session not found');
        console.error(chalk.red('Error:'), sessionResponse.error || 'Session does not exist');
        process.exit(1);
      }
      
      const session = sessionResponse.data;
      
      // Show session info
      console.log(chalk.gray('Session state:'), session.state);
      
      // If session is closed or idle, restore it
      if (session.state === SESSION_STATE_CLOSED || session.state === SESSION_STATE_IDLE) {
        const restoreResponse = await api.post(`/sessions/${sessionId}/restore`);
        
        if (!restoreResponse.success) {
          spinner.fail('Failed to restore session');
          console.error(chalk.red('Error:'), restoreResponse.error);
          process.exit(1);
        }
        
        spinner.succeed(`Session restored: ${sessionId}`);
        
        // Brief delay to allow container initialization
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        // Wait for session to become idle (ready for messages)
        let attempts = 0;
        while (attempts < 15) { // Wait up to 15 seconds
          await new Promise(resolve => setTimeout(resolve, 1000));
          const statusCheck = await api.get(`/sessions/${sessionId}`);
          if (statusCheck.success && (statusCheck.data.state === SESSION_STATE_IDLE || statusCheck.data.state === SESSION_STATE_BUSY)) {
            break;
          }
          attempts++;
        }
        
      } else if (session.state === SESSION_STATE_IDLE) {
        spinner.succeed(`Session already ready: ${sessionId}`);
      } else if (session.state === SESSION_STATE_BUSY) {
        spinner.succeed(`Session is being restored: ${sessionId}`);
        
        // Wait for session to become idle (ready for messages)
        let attempts = 0;
        while (attempts < 15) { // Wait up to 15 seconds
          await new Promise(resolve => setTimeout(resolve, 1000));
          const statusCheck = await api.get(`/sessions/${sessionId}`);
          if (statusCheck.success && (statusCheck.data.state === SESSION_STATE_IDLE || statusCheck.data.state === SESSION_STATE_BUSY)) {
            break;
          }
          attempts++;
        }
      } else {
        spinner.fail(`Cannot restore session in state: ${session.state}`);
        process.exit(1);
      }
      
      // Get and display recent messages if any
      const messagesResponse = await api.get(`/sessions/${sessionId}/messages?limit=10`);
      if (messagesResponse.success && messagesResponse.data) {
        const messages = Array.isArray(messagesResponse.data) ? messagesResponse.data : messagesResponse.data.messages || [];
        
        if (messages.length > 0) {
          console.log();
          console.log(chalk.gray('--- Recent messages ---'));
          
          // Show last few messages for context
          const recentMessages = messages.slice(-5);
          recentMessages.forEach(msg => {
            if (msg.role === MESSAGE_ROLE_USER) {
              console.log(chalk.gray('You:'), msg.content.substring(0, 80) + (msg.content.length > 80 ? '...' : ''));
            } else if (msg.role === MESSAGE_ROLE_AGENT) {
              console.log(chalk.cyan('Agent:'), msg.content.substring(0, 80) + (msg.content.length > 80 ? '...' : ''));
            }
          });
          console.log(chalk.gray('--- End of history ---'));
        }
      }
      
    } else {
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
      
      // Add instructions if provided
      if (options.instructions) {
        try {
          const fs = require('fs');
          sessionPayload.instructions = fs.readFileSync(options.instructions, 'utf8');
        } catch (error) {
          spinner.fail('Failed to read instructions file');
          console.error(chalk.red('Error:'), error.message);
          process.exit(1);
        }
      }
      
      // Add setup script if provided
      if (options.setup) {
        try {
          const fs = require('fs');
          sessionPayload.setup = fs.readFileSync(options.setup, 'utf8');
        } catch (error) {
          spinner.fail('Failed to read setup script');
          console.error(chalk.red('Error:'), error.message);
          process.exit(1);
        }
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
    }
    
    console.log();
    console.log(chalk.green('✅ Session active! Type your messages below.'));
    console.log(chalk.gray('Commands: /status, /quit'));
    console.log(chalk.gray('Session ID:'), sessionId);
    console.log();
    
    // Start synchronous chat loop
    await chatLoop(sessionId);
    
  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    
    // Clean up session if it was created
    if (sessionId) {
      try {
        await api.post(`/sessions/${sessionId}/close`);
      } catch (cleanupError) {
        // Ignore cleanup errors
      }
    }
    
    process.exit(1);
  }
}

async function waitForAgentResponse(sessionId, userMessageTime, timeoutMs = 60000) {
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
          // Look for the newest agent message
          for (let i = messages.length - 1; i >= 0; i--) {
            const message = messages[i];
            if (message.role === MESSAGE_ROLE_AGENT) {
              return message;
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

async function chatLoop(sessionId) {
  try {
    while (true) {
      // Get user input
      const answers = await inquirer.prompt([
        {
          type: 'input',
          name: 'message',
          message: 'You:',
          prefix: '', // Remove default prefix
          validate: (input) => {
            if (!input.trim()) {
              return 'Please enter a message';
            }
            return true;
          }
        }
      ]);
      
      const message = answers.message.trim();
      
      // Handle special commands
      if (message === '/quit' || message === '/q' || message === '/exit') {
        console.log(chalk.yellow('👋 Ending session...'));
        break;
      }
      
      if (message === '/status') {
        console.log();
        console.log(chalk.blue('📊 Session Status'));
        console.log(chalk.gray('Session ID:'), sessionId);
        console.log(chalk.gray('Server:'), config.getServerUrl());
        console.log();
        continue;
      }
      
      // Send message to agent
      const userMessageTime = Date.now();
      const sendResponse = await api.post(`/sessions/${sessionId}/messages`, {
        role: MESSAGE_ROLE_USER,
        content: message
      });
      
      if (!sendResponse.success) {
        console.error(chalk.red('❌ Failed to send message:'), sendResponse.error);
        
        if (sendResponse.status === 404) {
          console.log(chalk.red('❌ Session may have expired. Please start a new session.'));
          break;
        }
        continue;
      }
      
      // Wait for agent response
      const spinner = ora('Waiting for agent response...').start();
      
      try {
        const agentMessage = await waitForAgentResponse(sessionId, userMessageTime);
        spinner.stop();
        
        // Display agent response
        console.log(chalk.cyan('Agent:'), agentMessage.content);
        console.log();
        
      } catch (error) {
        spinner.fail('Agent response timeout');
        console.log(chalk.yellow('⚠️ No response received within 60 seconds'));
        console.log();
      }
    }
    
  } finally {
    // Close session for later restore
    try {
      const spinner = ora('Closing session...').start();
      await api.post(`/sessions/${sessionId}/close`);
      spinner.succeed('Session closed (can be restored later)');
    } catch (error) {
      console.log(chalk.yellow('⚠️ Failed to close session'));
    }
  }
}

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
  console.log();
  console.log(chalk.yellow('👋 Goodbye!'));
  process.exit(0);
});