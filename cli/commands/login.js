const chalk = require('chalk');
const inquirer = require('inquirer');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

module.exports = (program) => {
  // Login command for operator login - generates token without saving
  program
    .command('login')
    .description('Generate operator authentication token')
    .option('-s, --server <url>', 'Server URL', 'http://localhost:9000')
    .option('-u, --user <user>', 'Operator username for authentication')
    .option('-p, --pass <pass>', 'Password for authentication')
    .action(async (options) => {
      await operatorLogin(options);
    });
};

async function operatorLogin(options) {
  console.log(chalk.blue('🔑 Operator Login - Generating Authentication Token'));
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
      console.log(chalk.gray('Enter your operator credentials:'));
      
      const answers = await inquirer.prompt([
        {
          type: 'input',
          name: 'user',
          message: 'Operator:',
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

    const spinner = ora('Generating authentication token...').start();
    
    const response = await api.login({
      user: user,
      pass: pass
    });
    
    if (response.success) {
      spinner.succeed('Token generated successfully');
      console.log();
      console.log(chalk.green('🎟️ Authentication Token Generated'));
      console.log(chalk.gray('User:'), response.data.user);
      console.log(chalk.gray('Role:'), response.data.role || 'Unknown');
      console.log(chalk.gray('Token:'), response.data.token);
      console.log(chalk.gray('Expires:'), response.data.expires_at);
      console.log();
      console.log(chalk.yellow('💡 Use this token to authenticate:'));
      console.log(`  raworc auth -t ${response.data.token}`);
    } else {
      spinner.fail('Token generation failed');
      console.log();
      console.error(chalk.red('❌ Token generation failed'));
      console.error(chalk.gray('Error:'), response.error);
      
      if (response.status === 401) {
        console.log();
        console.log(chalk.yellow('💡 Tips:'));
        console.log('  • Check your operator credentials');
        console.log('  • Ensure Raworc server is running: ' + chalk.white('raworc start'));
        console.log('  • Default operator credentials: admin/admin');
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