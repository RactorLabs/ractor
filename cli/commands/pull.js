const chalk = require('chalk');
const { execSync } = require('child_process');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('pull [version]')
    .description('Pull CLI version and Docker images from registries')
    .option('-c, --cli-only', 'Only update the CLI, skip Docker images')
    .option('-i, --images-only', 'Only pull Docker images, skip CLI update')
    .addHelpText('after', '\n' +
      'Examples:\n' +
      '  $ raworc pull                    # Pull latest version\n' +
      '  $ raworc pull 0.4.0              # Pull specific version\n' +
      '  $ raworc pull latest             # Pull latest version (explicit)\n' +
      '  $ raworc pull 0.3.5 --cli-only  # Pull specific CLI version only\n')
    .action(async (version, options) => {
      try {
        // Default to latest if no version specified
        const targetVersion = version || 'latest';
        const versionDisplay = targetVersion === 'latest' ? 'latest' : `v${targetVersion}`;
        
        // Show command box with pull info
        const operation = options.cliOnly ? `Update CLI to ${versionDisplay}` : 
          options.imagesOnly ? `Pull Docker images (${versionDisplay})` : 
          `Update CLI and pull Docker images (${versionDisplay})`;
        display.showCommandBox(`${display.icons.pull} Pull ${versionDisplay}`, {
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
          const cliVersionTag = targetVersion === 'latest' ? 'latest' : targetVersion;
          display.info(`Updating CLI to ${versionDisplay}...`);
          try {
            // Install specific version of the CLI package globally
            const npmCommand = `npm install -g @raworc/cli@${cliVersionTag}`;
            execSync(npmCommand, { 
              stdio: options.verbose ? 'inherit' : 'pipe',
              encoding: 'utf8'
            });
            display.success(`CLI updated to ${versionDisplay}`);
          } catch (error) {
            display.error(`Failed to update CLI to ${versionDisplay}`);
            console.error(chalk.gray('Error updating CLI:'), error.message);
            display.info(`Try running: npm install -g @raworc/cli@${cliVersionTag}`);
          }
        }

        // Pull Docker images unless cli-only is specified
        if (!options.cliOnly) {
          const dockerVersionTag = targetVersion === 'latest' ? 'latest' : targetVersion;
          display.info(`Pulling Docker images (${versionDisplay})...`);
          try {
            await docker.pull(dockerVersionTag);
            display.success(`Docker images pulled successfully (${versionDisplay})`);
          } catch (error) {
            display.error(`Failed to pull some Docker images (${versionDisplay})`);
            console.error(chalk.gray('Error pulling images:'), error.message);
          }
        }

        console.log();
        display.success(`Pull completed! (${versionDisplay})`);
        
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