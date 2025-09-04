const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('auth')
    .description('Authenticate using JWT token or show authentication status')
    .option('-s, --server <url>', 'Server URL', 'http://localhost:9000')
    .option('-t, --token <token>', 'JWT token for authentication')
    .action(async (options) => {
      if (options.token) {
        await useTokenCommand(options);
      } else {
        await showAuthStatus();
      }
    });
};

async function showAuthStatus() {
  // Show command box with auth info
  display.showCommandBox(`${display.icons.auth} Authentication Status`, {
    operation: 'Check authentication status'
  });

  const authData = config.getAuth();
  
  if (!authData) {
    display.error('Not authenticated');
    console.log();
    display.info('To authenticate:');
    console.log('  raworc login --user admin --pass admin');
    console.log('  raworc auth --token <your-jwt-token>');
    return;
  }

  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('Current User:'), userName + userType);
  
  // Test if authentication is still valid
  display.info('Checking authentication...');
  const response = await api.checkAuth();
  
  if (response.success) {
    display.success('Authentication valid');
    
    if (authData.user) {
      console.log();
      console.log(chalk.blue('User Details:'));
      const userName = authData.user.user || authData.user;
      const userType = authData.user?.type ? ` (${authData.user.type})` : '';
      console.log(chalk.gray('  User:'), userName + userType);
      console.log(chalk.gray('  Role:'), authData.user.role || 'Unknown');
      if (authData.expires) {
        console.log(chalk.gray('  Expires:'), new Date(authData.expires).toLocaleString());
      }
    }
  } else {
    display.error('Authentication expired or invalid');
    console.log(chalk.gray('Error:'), response.error);
    console.log();
    display.info('Please re-authenticate:');
    console.log('  raworc login');
  }
}

async function useTokenCommand(options) {
  // Show command box with token auth info
  display.showCommandBox(`${display.icons.token} Token Authentication`, {
    operation: 'Authenticate with JWT token'
  });

  // Update server URL if provided
  if (options.server && options.server !== config.getServerUrl()) {
    config.saveConfig({ server: options.server });
    console.log(chalk.gray('Server URL updated:'), options.server);
  }

  try {
    display.info('Using token authentication...');
    display.info('Validating token...');
    
    const response = await api.loginWithToken(options.token, options.server);
    
    if (response.success) {
      display.success('Token authentication successful');
      console.log();
      display.success('Successfully authenticated');
      
      const authData = config.getAuth();
      if (authData?.user) {
        const userName = authData.user.user || authData.user;
        const userType = authData.user?.type ? ` (${authData.user.type})` : '';
        console.log(chalk.gray('User:'), chalk.white(userName + userType));
        console.log(chalk.gray('Role:'), authData.user.role || 'Unknown');
      }
      
      console.log();
      console.log(chalk.cyan('Next steps:'));
      console.log('  • Check health: ' + chalk.white('raworc api health'));
      console.log('  • List agents: ' + chalk.white('raworc api agents'));
      console.log('  • Start agent: ' + chalk.white('raworc agent'));
    } else {
      display.error('Token authentication failed');
      console.log();
      display.error('Authentication failed');
      console.error(chalk.gray('Error:'), response.error);
      
      if (response.status === 401) {
        console.log();
        display.info('Tips:');
        console.log('  • Check that your token is valid and not expired');
        console.log('  • Ensure the token was created for this server');
      } else if (response.status === 0) {
        console.log();
        display.info('Connection failed:');
        console.log('  • Check server URL: ' + chalk.white(config.getServerUrl()));
        console.log('  • Ensure services are running: ' + chalk.white('raworc start'));
      }
      
      process.exit(1);
    }
  } catch (error) {
    display.error('Error: ' + error.message);
    process.exit(1);
  }
}