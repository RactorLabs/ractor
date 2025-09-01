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

  // Login subcommand (user/pass authentication)
  authCmd
    .command('login')
    .description('Authenticate with user and password')
    .option('-s, --server <url>', 'Server URL', 'http://localhost:9000')
    .option('-u, --user <user>', 'User for authentication')
    .option('-p, --pass <pass>', 'Password for authentication')
    .action(async (options) => {
      await loginCommand(options);
    });

  // Use subcommand (token authentication)
  authCmd
    .command('use')
    .description('Authenticate using an existing JWT token')
    .option('-s, --server <url>', 'Server URL', 'http://localhost:9000')
    .requiredOption('-t, --token <token>', 'JWT token for authentication')
    .action(async (options) => {
      await useTokenCommand(options);
    });

  // Logout subcommand
  authCmd
    .command('logout')
    .description('Clear authentication credentials')
    .action(async () => {
      await logoutCommand();
    });

  // Token creation subcommand
  authCmd
    .command('token')
    .description('Create token for a principal')
    .requiredOption('--principal <name>', 'Principal name')
    .option('--type <type>', 'Principal type (User or Operator)', 'User')
    .action(async (options) => {
      await createTokenCommand(options);
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
    console.log('  raworc auth login --user admin --pass admin');
    console.log('  raworc auth use --token <your-jwt-token>');
    return;
  }

  console.log(chalk.gray('User:'), authData.user?.user || authData.user || 'Unknown');
  
  // Test if authentication is still valid
  const spinner = ora('Checking authentication...').start();
  const response = await api.checkAuth();
  
  if (response.success) {
    spinner.succeed('Authentication valid');
    console.log(chalk.green('Status: Authenticated'));
    
    if (authData.user) {
      console.log();
      console.log(chalk.blue('User Details:'));
      console.log(chalk.gray('  User:'), authData.user.user || authData.user);
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
    console.log('  raworc auth login');
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
    // User/pass authentication only
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
    
    const response = await api.login({
      user: user,
      pass: pass
    });
    
    if (response.success) {
      spinner.succeed('Authentication successful');
    } else {
      spinner.fail('Authentication failed');
    }

    if (response.success) {
      console.log();
      console.log(chalk.green('✅ Successfully authenticated'));
      
      const authData = config.getAuth();
      if (authData?.user) {
        console.log(chalk.gray('Welcome,'), chalk.white(authData.user.user || authData.user));
        console.log(chalk.gray('Role:'), authData.user.role || 'Unknown');
      }
      
      console.log();
      console.log(chalk.cyan('Next steps:'));
      console.log('  • Check health: ' + chalk.white('raworc api health'));
      console.log('  • List sessions: ' + chalk.white('raworc api sessions'));
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
        console.log(chalk.gray('User:'), chalk.white(authData.user.user || authData.user));
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

async function createTokenCommand(options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc auth login') + ' to authenticate first');
    process.exit(1);
  }

  const spinner = ora('Creating token...').start();
  
  try {
    const api = require('../lib/api');
    const response = await api.post('/auth/token', {
      principal: options.principal,
      principal_type: options.type
    });

    if (response.success) {
      spinner.succeed('Token created successfully');
      console.log();
      console.log(chalk.green('🎟️ Token Details'));
      console.log(chalk.gray('Principal:'), options.principal);
      console.log(chalk.gray('Type:'), options.type);
      console.log(chalk.gray('Token:'), response.data.token);
      console.log(chalk.gray('Expires:'), response.data.expires_at);
      console.log();
      console.log(chalk.yellow('💡 Use this token:'));
      console.log(`  raworc auth use --token ${response.data.token}`);
    } else {
      spinner.fail('Token creation failed');
      console.error(chalk.red('Error:'), response.error?.message || 'Unknown error');
      process.exit(1);
    }
  } catch (error) {
    spinner.fail('Token creation failed');
    console.error(chalk.red('Error:'), error.message);
    process.exit(1);
  }
}