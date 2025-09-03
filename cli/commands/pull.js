const chalk = require('chalk');
const { execSync } = require('child_process');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('pull')
    .description('Pull latest CLI version and Docker images from registries')
    .option('-c, --cli-only', 'Only update the CLI, skip Docker images')
    .option('-i, --images-only', 'Only pull Docker images, skip CLI update')
    .action(async (options) => {
      try {
        // Show command box with pull info
        const operation = options.cliOnly ? 'Update CLI only' : 
          options.imagesOnly ? 'Pull Docker images only' : 
          'Update CLI and pull Docker images';
        display.showCommandBox(`${display.icons.pull} Pull Latest`, {
          operation: operation
        });
        
        // Check Docker availability if we need to pull images
        if (!options.cliOnly) {
          const dockerAvailable = await docker.checkDocker();
          if (!dockerAvailable) {
            display.error('Docker is not available. Use --cli-only to update CLI only.');
            process.exit(1);
          }
        }

        // Update CLI unless images-only is specified
        if (!options.imagesOnly) {
          display.info('Updating CLI to latest version...');
          try {
            // Update the CLI package globally
            execSync('npm update -g @raworc/cli', { 
              stdio: options.verbose ? 'inherit' : 'pipe',
              encoding: 'utf8'
            });
            display.success('CLI updated to latest version');
          } catch (error) {
            display.error('Failed to update CLI');
            console.error(chalk.gray('Error updating CLI:'), error.message);
            display.info('Try running: npm install -g @raworc/cli@latest');
          }
        }

        // Pull Docker images unless cli-only is specified
        if (!options.cliOnly) {
          display.info('Pulling latest Docker images...');
          try {
            await docker.pull();
            display.success('Docker images pulled successfully');
          } catch (error) {
            display.error('Failed to pull some Docker images');
            console.error(chalk.gray('Error pulling images:'), error.message);
          }
        }

        console.log();
        display.success('Pull completed!');
        
        // Show next steps
        console.log();
        console.log(chalk.cyan('Next steps:'));
        console.log('  • Start services: ' + chalk.white('raworc start'));
        console.log('  • Check version: ' + chalk.white('raworc --version'));
        
      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};