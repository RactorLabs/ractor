const { spawn } = require('child_process');
const path = require('path');
const chalk = require('chalk');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('setup')
    .description('Install host prerequisites (GPU drivers + NVIDIA Container Toolkit)')
    .option('--driver-only', 'Install only GPU drivers (Ubuntu/Debian)')
    .option('--toolkit-only', 'Install only NVIDIA Container Toolkit')
    .action(async (options) => {
      try {
        display.showCommandBox(`${display.icons.start} Host Setup`, {
          operation: 'Install GPU drivers and container toolkit',
        });

        const scriptPath = path.join(__dirname, '..', 'scripts', 'setup.sh');
        const args = [];
        if (options.driverOnly) args.push('--driver-only');
        if (options.toolkitOnly) args.push('--toolkit-only');

        console.log(chalk.yellow('Note: This may require sudo privileges.'));
        const shell = spawn('bash', [scriptPath, ...args], { stdio: 'inherit' });
        shell.on('close', (code) => {
          if (code === 0) {
            console.log(chalk.green('Setup completed. A reboot may be required for drivers.'));
          } else {
            console.error(chalk.red(`Setup exited with code ${code}`));
            process.exit(code || 1);
          }
        });
      } catch (err) {
        console.error(chalk.red('Error running setup:'), err.message);
        process.exit(1);
      }
    });
};

