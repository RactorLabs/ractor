const chalk = require('chalk');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

module.exports = (program) => {
  program
    .command('token')
    .description('Create authentication token for a principal')
    .requiredOption('-p, --principal <name>', 'Principal name')
    .option('-t, --type <type>', 'Principal type (User or Operator)', 'User')
    .action(async (options) => {
      await createTokenCommand(options);
    });
};

async function createTokenCommand(options) {
  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    console.log(chalk.red('❌ Authentication required'));
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  const spinner = ora('Creating token...').start();
  
  try {
    const api = require('../lib/api');
    const response = await api.post('/auth/token', {
      principal: options.principal,
      type: options.type
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
      console.log(`  raworc auth -t ${response.data.token}`);
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