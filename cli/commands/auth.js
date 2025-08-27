const chalk = require('chalk');
const inquirer = require('inquirer');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

module.exports = (program) => {
  // Main auth command with subcommands
  const authCmd = program
    .command('auth')
    .description('Authentication management');

  // Status subcommand (default when no subcommand)
  authCmd
    .command('status', { isDefault: true })
    .description('Show authentication status')
    .action(async () => {
      await showAuthStatus();
    });

  // Login subcommand
  authCmd
    .command('login')
    .description('Authenticate with Raworc server')
    .option('-s, --server <url>', 'Server URL', 'http://localhost:9000')
    .option('-t, --token <token>', 'JWT token for direct authentication')
    .option('-u, --user <user>', 'User for credential authentication')
    .option('-p, --pass <pass>', 'Password for credential authentication')
    .action(async (options) => {
      await loginCommand(options);
    });

  // Logout subcommand
  authCmd
    .command('logout')
    .description('Clear authentication credentials')
    .action(async () => {
      await logoutCommand();
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
    console.log('  raworc auth:login --user admin --pass admin');
    console.log('  raworc auth:login --token <your-jwt-token>');
    return;
  }

  console.log(chalk.gray('User:'), authData.user?.username || 'Unknown');
  
  // Test if authentication is still valid
  const spinner = ora('Checking authentication...').start();
  const response = await api.checkAuth();
  
  if (response.success) {
    spinner.succeed('Authentication valid');
    console.log(chalk.green('Status: Authenticated'));
    
    if (authData.user) {
      console.log();
      console.log(chalk.blue('User Details:'));
      console.log(chalk.gray('  Username:'), authData.user.username);
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
    console.log('  raworc auth:login');
  }
}

async function loginCommand(options) {
  console.log(chalk.blue('🔑 Authenticating with Raworc...'));
  console.log();

  // Update server URL if provided
  if (options.server && options.server !== config.getServerUrl()) {
    config.saveConfig({ server: options.server });
    console.log(chalk.gray('Server URL updated:'), options.server);
  }

  try {
    let response;

    if (options.token) {
      // Token-based authentication
      console.log(chalk.gray('Using token authentication...'));
      const spinner = ora('Validating token...').start();
      
      response = await api.loginWithToken(options.token, options.server);
      
      if (response.success) {
        spinner.succeed('Token authentication successful');
      } else {
        spinner.fail('Token authentication failed');
      }

    } else {
      // User/pass authentication
      let user = options.user;
      let pass = options.pass;

      // Prompt for missing credentials
      if (!user || !pass) {
        console.log(chalk.gray('Enter your credentials:'));
        
        const answers = await inquirer.prompt([
          {
            type: 'input',
            name: 'user',
            message: 'User:',
            default: user || 'admin',
            when: !user
          },
          {
            type: 'password',
            name: 'pass',
            message: 'Password:',
            when: !pass
          }
        ]);

        user = user || answers.user;
        pass = pass || answers.pass;
      }

      const spinner = ora('Authenticating...').start();
      
      response = await api.login({
        user: user,
        pass: pass
      });
      
      if (response.success) {
        spinner.succeed('Authentication successful');
      } else {
        spinner.fail('Authentication failed');
      }
    }

    if (response.success) {
      console.log();
      console.log(chalk.green('✅ Successfully authenticated'));
      
      const authData = config.getAuth();
      if (authData?.user) {
        console.log(chalk.gray('Welcome,'), chalk.white(authData.user.username));
        console.log(chalk.gray('Role:'), authData.user.role || 'Unknown');
      }
      
      console.log();
      console.log(chalk.cyan('Next steps:'));
      console.log('  • Check health: ' + chalk.white('raworc api health'));
      console.log('  • List spaces: ' + chalk.white('raworc api spaces'));
      console.log('  • Start session: ' + chalk.white('raworc session'));

    } else {
      console.log();
      console.error(chalk.red('❌ Authentication failed'));
      console.error(chalk.gray('Error:'), response.error);
      
      if (response.status === 401) {
        console.log();
        console.log(chalk.yellow('💡 Tips:'));
        console.log('  • Check your user and password');
        console.log('  • Ensure Raworc server is running: ' + chalk.white('raworc start'));
        console.log('  • Default credentials: admin/admin');
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

async function logoutCommand() {
  const authData = config.getAuth();
  
  if (!authData) {
    console.log(chalk.yellow('⚠️ Not currently authenticated'));
    return;
  }

  api.logout();
  console.log(chalk.green('✅ Logged out successfully'));
  console.log(chalk.gray('Authentication credentials cleared'));
}