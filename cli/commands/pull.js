const chalk = require('chalk');
const ora = require('ora');
const { execSync } = require('child_process');
const docker = require('../lib/docker');

module.exports = (program) => {
  program
    .command('pull')
    .description('Pull latest CLI version and Docker images from registries')
    .option('-c, --cli-only', 'Only update the CLI, skip Docker images')
    .option('-i, --images-only', 'Only pull Docker images, skip CLI update')
    .action(async (options) => {
      try {
        console.log(chalk.blue('📦 Pulling latest versions...'));
        
        // Check Docker availability if we need to pull images
        if (!options.cliOnly) {
          const dockerAvailable = await docker.checkDocker();
          if (!dockerAvailable) {
            console.error(chalk.red('❌ Docker is not available. Use --cli-only to update CLI only.'));
            process.exit(1);
          }
        }

        // Update CLI unless images-only is specified
        if (!options.imagesOnly) {
          const cliSpinner = ora('Updating CLI to latest version...').start();
          try {
            // Update the CLI package globally
            execSync('npm update -g @raworc/cli', { 
              stdio: options.verbose ? 'inherit' : 'pipe',
              encoding: 'utf8'
            });
            cliSpinner.succeed('CLI updated to latest version');
          } catch (error) {
            cliSpinner.fail('Failed to update CLI');
            console.error(chalk.red('Error updating CLI:'), error.message);
            console.log(chalk.yellow('💡 Try running: npm install -g @raworc/cli@latest'));
          }
        }

        // Pull Docker images unless cli-only is specified
        if (!options.cliOnly) {
          const imageSpinner = ora('Pulling latest Docker images...').start();
          try {
            await docker.pull();
            imageSpinner.succeed('Docker images pulled successfully');
          } catch (error) {
            imageSpinner.fail('Failed to pull some Docker images');
            console.error(chalk.red('Error pulling images:'), error.message);
          }
        }

        console.log();
        console.log(chalk.green('✅ Pull completed!'));
        
        // Show next steps
        console.log();
        console.log(chalk.cyan('Next steps:'));
        console.log('  • Start services: ' + chalk.white('raworc start'));
        console.log('  • Check version: ' + chalk.white('raworc --version'));
        
      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};