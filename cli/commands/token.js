const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');
const display = require('../lib/display');

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
  // Show command box with token creation info
  display.showCommandBox(`${display.icons.token} Create Token`, {
    operation: `Create token for ${options.principal} (${options.type})`
  });

  // Check authentication
  const authData = config.getAuth();
  if (!authData) {
    display.error('Authentication required');
    console.log('Run: ' + chalk.white('raworc login') + ' to authenticate first');
    process.exit(1);
  }

  display.info('Creating token...');
  
  try {
    const api = require('../lib/api');
    const response = await api.post('/auth/token', {
      principal: options.principal,
      type: options.type
    });

    if (response.success) {
      display.success('Token created successfully');
      console.log();
      console.log(chalk.green(display.icons.token + ' Token Details'));
      console.log(chalk.gray('Principal:'), options.principal);
      console.log(chalk.gray('Type:'), options.type);
      console.log(chalk.gray('Token:'), response.data.token);
      console.log(chalk.gray('Expires:'), response.data.expires_at);
      console.log();
      display.info('Use this token:');
      console.log(`  raworc auth -t ${response.data.token}`);
    } else {
      display.error('Token creation failed');
      console.error(chalk.gray('Error:'), response.error?.message || 'Unknown error');
      process.exit(1);
    }
  } catch (error) {
    display.error('Token creation failed');
    console.error(chalk.gray('Error:'), error.message);
    process.exit(1);
  }
}