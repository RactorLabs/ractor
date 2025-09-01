const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');

module.exports = (program) => {
  program
    .command('logout')
    .description('Clear authentication credentials')
    .action(async () => {
      await logoutCommand();
    });
};

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