const chalk = require('chalk');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');
const docker = require('../lib/docker');

module.exports = (program) => {
  program
    .command('clean')
    .description('Clean all session containers (preserves core services and volumes)')
    .action(async (options) => {
      try {
        console.log(chalk.blue('🧹 Session Container Cleanup'));
        console.log();

        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          console.error(chalk.red('❌ Docker is not available'));
          process.exit(1);
        }

        let containersToClean = [];

        // Get running containers
        const listSpinner = ora('Finding session containers...').start();
        
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

          listSpinner.stop();

          if (containersToClean.length === 0) {
            console.log(chalk.green('✅ No session containers found'));
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
            const containerSpinner = ora(`Cleaning up ${containersToClean.length} session containers...`).start();

            for (const container of containersToClean) {
              try {
                // Stop and remove container
                await docker.execDocker(['stop', container.name], { silent: true });
                const removeResult = await docker.execDocker(['rm', container.name], { silent: true });
                
                if (removeResult.code === 0) {
                  totalCleaned++;
                } else {
                  totalFailed++;
                  console.log(chalk.yellow(`Warning: Failed to remove container ${container.name}`));
                }
              } catch (error) {
                totalFailed++;
                console.log(chalk.yellow(`Warning: Failed to clean container ${container.name}: ${error.message}`));
              }
            }

            containerSpinner.stop();
            console.log(chalk.green(`✅ Cleaned up ${totalCleaned} session containers`));
          }

          // No image cleanup - sessions only

          if (totalFailed > 0) {
            console.log(chalk.yellow(`⚠️ Failed to clean ${totalFailed} session containers`));
          }

          console.log();
          if (totalCleaned > 0) {
            console.log(chalk.green('🎉 Session cleanup completed!'));
          }

          console.log();
          console.log(chalk.gray('Note: Core services and volumes are preserved'));

        } catch (error) {
          listSpinner.stop();
          throw error;
        }

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};