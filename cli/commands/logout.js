const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('logout')
    .description('Clear authentication credentials')
    .action(async () => {
      await logoutCommand();
    });
};

async function logoutCommand() {
  // Show command box with logout info
  display.showCommandBox(`${display.icons.user} Logout`, {
    operation: 'Clear authentication credentials'
  });

  const authData = config.getAuth();
  
  if (!authData) {
    display.warning('Not currently authenticated');
    return;
  }

  api.logout();
  display.success('Logged out successfully');
  console.log(chalk.gray('Authentication credentials cleared'));
}