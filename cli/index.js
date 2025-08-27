#!/usr/bin/env node

const { Command } = require('commander');
const chalk = require('chalk');
const pkg = require('./package.json');

// Import commands
const startCommand = require('./commands/start');
const stopCommand = require('./commands/stop');
const resetCommand = require('./commands/reset');
const cleanupCommand = require('./commands/cleanup');
const sessionCommand = require('./commands/session');
const authCommand = require('./commands/auth');
const apiCommand = require('./commands/api');
const pullCommand = require('./commands/pull');

const program = new Command();

program
  .name('raworc')
  .description('Universal AI Agent Runtime - CLI for managing AI agents')
  .version(pkg.version);

// Configure commands
startCommand(program);
stopCommand(program);
resetCommand(program);
cleanupCommand(program);
sessionCommand(program);
authCommand(program);
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