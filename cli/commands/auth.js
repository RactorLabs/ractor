const chalk = require('chalk');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

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
  console.log(chalk.blue('🔐 Authentication Status'));
  console.log();

  const authData = config.getAuth();
  const serverUrl = config.getServerUrl();
  
  console.log(chalk.gray('Server:'), serverUrl);
  
  if (!authData) {
    console.log(chalk.red('Status: Not authenticated'));
    console.log();
    console.log(chalk.yellow('💡 To authenticate:'));
    console.log('  raworc login --user admin --pass admin');
    console.log('  raworc auth --token <your-jwt-token>');
    return;
  }

  const userName = authData.user?.user || authData.user || 'Unknown';
  const userType = authData.user?.type ? ` (${authData.user.type})` : '';
  console.log(chalk.gray('User:'), userName + userType);
  
  // Test if authentication is still valid
  const spinner = ora('Checking authentication...').start();
  const response = await api.checkAuth();
  
  if (response.success) {
    spinner.succeed('Authentication valid');
    console.log(chalk.green('Status: Authenticated'));
    
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
    spinner.fail('Authentication expired or invalid');
    console.log(chalk.red('Status: Authentication expired'));
    console.log(chalk.gray('Error:'), response.error);
    console.log();
    console.log(chalk.yellow('💡 Please re-authenticate:'));
    console.log('  raworc login');
  }
}

async function useTokenCommand(options) {
  console.log(chalk.blue('🔑 Authenticating with token...'));
  console.log();

  // Update server URL if provided
  if (options.server && options.server !== config.getServerUrl()) {
    config.saveConfig({ server: options.server });
    console.log(chalk.gray('Server URL updated:'), options.server);
  }

  try {
    console.log(chalk.gray('Using token authentication...'));
    const spinner = ora('Validating token...').start();
    
    const response = await api.loginWithToken(options.token, options.server);
    
    if (response.success) {
      spinner.succeed('Token authentication successful');
      console.log();
      console.log(chalk.green('✅ Successfully authenticated'));
      
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
      console.log('  • List sessions: ' + chalk.white('raworc api sessions'));
      console.log('  • Start session: ' + chalk.white('raworc session'));
    } else {
      spinner.fail('Token authentication failed');
      console.log();
      console.error(chalk.red('❌ Authentication failed'));
      console.error(chalk.gray('Error:'), response.error);
      
      if (response.status === 401) {
        console.log();
        console.log(chalk.yellow('💡 Tips:'));
        console.log('  • Check that your token is valid and not expired');
        console.log('  • Ensure the token was created for this server');
      } else if (response.status === 0) {
        console.log();
        console.log(chalk.yellow('💡 Connection failed:'));
        console.log('  • Check server URL: ' + chalk.white(config.getServerUrl()));
        console.log('  • Ensure services are running: ' + chalk.white('raworc start'));
      }
      
      process.exit(1);
    }
  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}