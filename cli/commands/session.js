const chalk = require('chalk');
const inquirer = require('inquirer');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

module.exports = (program) => {
  program
    .command('session')
    .description('Start an interactive AI agent session')
    .option('-s, --space <space>', 'Space name for the session', 'default')
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
  console.log(chalk.gray('Space:'), options.space);
  console.log(chalk.gray('User:'), authData.user?.username || 'Unknown');
  console.log();

  let sessionId = null;
  
  try {
    // Create a new session
    const spinner = ora('Creating session...').start();
    
    const createResponse = await api.post('/sessions', {
      space: options.space
    });
    
    if (!createResponse.success) {
      spinner.fail('Failed to create session');
      console.error(chalk.red('Error:'), createResponse.error);
      
      if (createResponse.status === 404) {
        console.log();
        console.log(chalk.yellow('💡 Space may not exist. Available commands:'));
        console.log('  • List spaces: ' + chalk.white('raworc api spaces'));
      }
      
      process.exit(1);
    }
    
    sessionId = createResponse.data.id;
    spinner.succeed(`Session created: ${sessionId}`);
    
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
            if (message.role === 'agent') {
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
        console.log(chalk.gray('Space:'), options?.space || 'default');
        console.log();
        continue;
      }
      
      // Send message to agent
      const userMessageTime = Date.now();
      const sendResponse = await api.post(`/sessions/${sessionId}/messages`, {
        role: 'user',
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