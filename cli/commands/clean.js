const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('clean')
    .description('Clean all session containers (preserves core services and volumes)')
    .action(async (options) => {
      try {
        // Show command box with clean info
        display.showCommandBox(`${display.icons.clean} Session Container Cleanup`, {
          operation: 'Clean all session containers'
        });

        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          display.error('Docker is not available');
          process.exit(1);
        }

        let containersToClean = [];

        // Get running containers
        display.info('Finding session containers...');
        
        try {
          // Find only raworc_session containers (running and stopped)
          const containerResult = await docker.execDocker([
            'ps', '-a', '--filter', 'name=raworc_session', '--format', '{{.Names}}\t{{.Status}}\t{{.Image}}'
          ], { silent: true });

          if (containerResult.stdout && containerResult.stdout.trim()) {
            const containers = containerResult.stdout.trim().split('\n').map(line => {
              const [name, status, image] = line.split('\t');
              return { name, status, image };
            });
            containersToClean = containers;
          }

          if (containersToClean.length === 0) {
            display.success('No session containers found');
            return;
          }

          console.log(chalk.yellow(`Found ${containersToClean.length} session container(s):`));
          containersToClean.forEach((container, i) => {
            console.log(`  ${i + 1}. ${container.name} (${container.status})`);
          });

          console.log();
          let totalCleaned = 0;
          let totalFailed = 0;

          // Clean up session containers
          if (containersToClean.length > 0) {
            display.info(`Cleaning up ${containersToClean.length} session containers...`);

            for (const container of containersToClean) {
              try {
                // Stop and remove container
                await docker.execDocker(['stop', container.name], { silent: true });
                const removeResult = await docker.execDocker(['rm', container.name], { silent: true });
                
                if (removeResult.code === 0) {
                  totalCleaned++;
                } else {
                  totalFailed++;
                  display.warning(`Failed to remove container ${container.name}`);
                }
              } catch (error) {
                totalFailed++;
                display.warning(`Failed to clean container ${container.name}: ${error.message}`);
              }
            }

            display.success(`Cleaned up ${totalCleaned} session containers`);
          }

          // No image cleanup - sessions only

          if (totalFailed > 0) {
            display.warning(`Failed to clean ${totalFailed} session containers`);
          }

          console.log();
          if (totalCleaned > 0) {
            display.success('Session cleanup completed!');
          }

          console.log();
          console.log(chalk.gray('Note: Core services and volumes are preserved'));

        } catch (error) {
          throw error;
        }

      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};