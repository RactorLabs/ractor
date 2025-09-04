#!/usr/bin/env node

const { Command } = require('commander');
const chalk = require('chalk');
const pkg = require('./package.json');

// Import commands
const startCommand = require('./commands/start');
const stopCommand = require('./commands/stop');
const resetCommand = require('./commands/reset');
const cleanCommand = require('./commands/clean');
const agentCommand = require('./commands/agent');
const authCommand = require('./commands/auth');
const loginCommand = require('./commands/login');
const logoutCommand = require('./commands/logout');
const tokenCommand = require('./commands/token');
const apiCommand = require('./commands/api');
const pullCommand = require('./commands/pull');

const program = new Command();

program
  .name('raworc')
  .description('Remote Agentic Work Orchestrator - CLI for Computer use agents')
  .version(pkg.version, '-v, --version', 'output the version number');

// Configure commands
startCommand(program);
stopCommand(program);
resetCommand(program);
cleanCommand(program);
agentCommand(program);
authCommand(program);
loginCommand(program);
logoutCommand(program);
tokenCommand(program);
apiCommand(program);
pullCommand(program);

// Default help behavior
program.action(() => {
  program.help();
});

// Error handling
program.exitOverride();

try {
  program.parse();
} catch (err) {
  if (err.code === 'commander.help') {
    process.exit(0);
  }
  if (err.code === 'commander.version') {
    process.exit(0);
  }
  console.error(chalk.red('Error:'), err.message);
  process.exit(1);
}