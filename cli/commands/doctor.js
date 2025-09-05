const { spawn } = require('child_process');
const path = require('path');
const chalk = require('chalk');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('doctor')
    .description('Run environment diagnostics and show GPU/Docker status')
    .action(async () => {
      try {
        display.showCommandBox(`${display.icons.info} Doctor`, {
          operation: 'Check host readiness and GPU access',
        });

        const scriptPath = path.join(__dirname, '..', 'scripts', 'doctor.sh');
        const shell = spawn('bash', [scriptPath], { stdio: 'inherit' });
        shell.on('close', (code) => {
          if (code === 0) {
            console.log(chalk.green('Diagnostics completed.'));
          } else {
            console.error(chalk.red(`Diagnostics exited with code ${code}`));
            process.exit(code || 1);
          }
        });
      } catch (err) {
        console.error(chalk.red('Error running doctor:'), err.message);
        process.exit(1);
      }
    });
};

