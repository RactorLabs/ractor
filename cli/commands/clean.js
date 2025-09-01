const chalk = require('chalk');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');
const docker = require('../lib/docker');

module.exports = (program) => {
  program
    .command('clean')
    .description('Clean containers and optionally Docker images')
    .option('-y, --yes', 'Confirm without prompting')
    .option('-a, --all', 'Also clean up Docker images (preserves volumes)')
    .action(async (options) => {
      try {
        console.log(chalk.blue('🧹 Container Cleanup'));
        console.log();

        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          console.error(chalk.red('❌ Docker is not available'));
          process.exit(1);
        }

        let containersToClean = [];
        let imagesToClean = [];

        // Get running containers
        const listSpinner = ora('Finding running containers...').start();
        
        try {
          // Find all raworc containers (running and stopped)
          const containerResult = await docker.execDocker([
            'ps', '-a', '--filter', 'name=raworc', '--format', '{{.Names}}\t{{.Status}}\t{{.Image}}'
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
            console.log(chalk.green('✅ No raworc containers found'));
            
            if (options.all) {
              // Still check for images to clean
              const imageSpinner = ora('Finding raworc images...').start();
              const imageResult = await docker.execDocker([
                'images', '--filter', 'reference=raworc*', '--filter', 'reference=*/raworc*', 
                '--format', '{{.Repository}}:{{.Tag}}\t{{.Size}}'
              ], { silent: true });

              if (imageResult.stdout && imageResult.stdout.trim()) {
                const images = imageResult.stdout.trim().split('\n').map(line => {
                  const [name, size] = line.split('\t');
                  return { name, size };
                });
                imagesToClean = images;
              }
              imageSpinner.stop();

              if (imagesToClean.length === 0) {
                console.log(chalk.green('✅ No raworc images found'));
                return;
              }
            } else {
              return;
            }
          } else {
            console.log(chalk.yellow(`Found ${containersToClean.length} raworc container(s):`));
            containersToClean.forEach((container, i) => {
              console.log(`  ${i + 1}. ${container.name} (${container.status})`);
            });

            if (options.all) {
              // Also get images
              const imageSpinner = ora('Finding raworc images...').start();
              const imageResult = await docker.execDocker([
                'images', '--filter', 'reference=raworc*', '--filter', 'reference=*/raworc*',
                '--format', '{{.Repository}}:{{.Tag}}\t{{.Size}}'
              ], { silent: true });

              if (imageResult.stdout && imageResult.stdout.trim()) {
                const images = imageResult.stdout.trim().split('\n').map(line => {
                  const [name, size] = line.split('\t');
                  return { name, size };
                });
                imagesToClean = images;
              }
              imageSpinner.stop();

              if (imagesToClean.length > 0) {
                console.log();
                console.log(chalk.yellow(`Found ${imagesToClean.length} raworc image(s):`));
                imagesToClean.forEach((image, i) => {
                  console.log(`  ${i + 1}. ${image.name} (${image.size})`);
                });
              }
            }
          }

          console.log();

          // Confirm unless --yes flag is provided
          if (!options.yes) {
            const readline = require('readline');
            const rl = readline.createInterface({
              input: process.stdin,
              output: process.stdout
            });

            let confirmMessage = '';
            if (containersToClean.length > 0 && imagesToClean.length > 0) {
              confirmMessage = `Clean up ${containersToClean.length} containers and ${imagesToClean.length} images? [y/N]: `;
            } else if (containersToClean.length > 0) {
              confirmMessage = `Clean up ${containersToClean.length} containers? [y/N]: `;
            } else if (imagesToClean.length > 0) {
              confirmMessage = `Clean up ${imagesToClean.length} images? [y/N]: `;
            }

            const answer = await new Promise(resolve => {
              rl.question(chalk.yellow(confirmMessage), resolve);
            });
            
            rl.close();

            if (!answer.match(/^[Yy]$/)) {
              console.log(chalk.blue('Operation cancelled'));
              return;
            }
          }

          console.log();
          let totalCleaned = 0;
          let totalFailed = 0;

          // Clean up containers
          if (containersToClean.length > 0) {
            const containerSpinner = ora(`Cleaning up ${containersToClean.length} containers...`).start();

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
            console.log(chalk.green(`✅ Cleaned up ${totalCleaned} containers`));
          }

          // Clean up images if --all flag is provided
          if (options.all && imagesToClean.length > 0) {
            const imageSpinner = ora(`Cleaning up ${imagesToClean.length} images...`).start();

            let imagesCleaned = 0;
            let imagesFailed = 0;

            for (const image of imagesToClean) {
              try {
                const removeResult = await docker.execDocker(['rmi', image.name], { silent: true });
                
                if (removeResult.code === 0) {
                  imagesCleaned++;
                } else {
                  imagesFailed++;
                  console.log(chalk.yellow(`Warning: Failed to remove image ${image.name}`));
                }
              } catch (error) {
                imagesFailed++;
                console.log(chalk.yellow(`Warning: Failed to clean image ${image.name}: ${error.message}`));
              }
            }

            imageSpinner.stop();
            console.log(chalk.green(`✅ Cleaned up ${imagesCleaned} images`));
            
            if (imagesFailed > 0) {
              console.log(chalk.yellow(`⚠️ Failed to clean ${imagesFailed} images`));
            }
          }

          if (totalFailed > 0) {
            console.log(chalk.yellow(`⚠️ Failed to clean ${totalFailed} containers`));
          }

          console.log();
          if (totalCleaned > 0 || (options.all && imagesToClean.length > 0)) {
            console.log(chalk.green('🎉 Cleanup completed!'));
          }

          if (!options.all && containersToClean.length > 0) {
            console.log();
            console.log(chalk.blue('💡 To also clean up Docker images:'));
            console.log(chalk.gray('  raworc cleanup --all'));
          }

          console.log();
          console.log(chalk.gray('Note: Docker volumes are preserved and not cleaned up'));

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