const chalk = require('chalk');
const ora = require('ora');
const { spawn } = require('child_process');
const readline = require('readline');

// Execute Docker command directly
async function execDocker(args, options = {}) {
  return new Promise((resolve, reject) => {
    const docker = spawn('docker', args, {
      stdio: options.silent ? 'pipe' : 'inherit',
      ...options
    });

    let stdout = '';
    let stderr = '';

    if (options.silent) {
      docker.stdout.on('data', (data) => {
        stdout += data.toString();
      });

      docker.stderr.on('data', (data) => {
        stderr += data.toString();
      });
    }

    docker.on('exit', (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(new Error(`Docker command failed: ${stderr || 'Unknown error'}`));
      }
    });

    docker.on('error', reject);
  });
}

module.exports = (program) => {
  program
    .command('reset')
    .description('Clean up everything: stop services, remove containers, and prune Docker')
    .option('-y, --yes', 'Confirm without prompting (non-interactive)')
    .option('-s, --services-only', 'Only stop services, don\'t clean Docker resources')
    .action(async (options) => {
      try {
        console.log(chalk.blue('🧹 Raworc Reset - Complete Cleanup'));
        console.log();

        if (options.servicesOnly) {
          console.log(chalk.blue('Services-only mode: Will stop services but skip Docker cleanup'));
        } else {
          console.log(chalk.blue('Full reset mode: This will remove ALL Raworc-related Docker resources'));
        }

        console.log();
        console.log(chalk.yellow('⚠️  This will:'));
        console.log(chalk.yellow('  - Stop all Raworc services'));
        console.log(chalk.yellow('  - Remove ALL Raworc containers (session, server, operator, mysql)'));

        if (!options.servicesOnly) {
          console.log(chalk.yellow('  - Remove ALL Raworc images'));
          console.log(chalk.yellow('  - Remove ALL Docker volumes'));
          console.log(chalk.yellow('  - Remove unused Docker networks'));
          console.log(chalk.yellow('  - Clean up build cache'));
        }

        // Confirm with user unless --yes flag is provided
        if (!options.yes) {
          const rl = readline.createInterface({
            input: process.stdin,
            output: process.stdout
          });

          const answer = await new Promise(resolve => {
            rl.question(chalk.yellow('This is a destructive operation. Continue? [y/N]: '), resolve);
          });

          rl.close();

          if (!answer.match(/^[Yy]$/)) {
            console.log(chalk.blue('Operation cancelled'));
            return;
          }
        }

        console.log();
        console.log(chalk.blue('Starting reset process...'));

        // Step 1: Stop all running containers first
        console.log();
        const stopSpinner = ora('[1/8] Stopping Raworc services...').start();

        try {
          const runningResult = await execDocker(['ps', '-q', '--filter', 'name=raworc_'], { silent: true });

          if (runningResult.stdout.trim()) {
            const runningIds = runningResult.stdout.trim().split('\n').filter(id => id);
            if (runningIds.length > 0) {
              await execDocker(['stop', ...runningIds], { silent: true });
              stopSpinner.succeed(`Stopped ${runningIds.length} running containers`);
            } else {
              stopSpinner.succeed('No running containers found');
            }
          } else {
            stopSpinner.succeed('No running containers found');
          }
        } catch (error) {
          stopSpinner.warn(`Stop warning: ${error.message}`);
        }

        // Step 2: Remove ALL raworc containers (running and stopped)
        console.log();
        const removeSpinner = ora('[2/8] Removing ALL raworc containers...').start();

        try {
          const result = await execDocker(['ps', '-a', '-q', '--filter', 'name=raworc'], { silent: true });

          if (result.stdout.trim()) {
            const containerIds = result.stdout.trim().split('\n').filter(id => id);

            if (containerIds.length > 0) {
              await execDocker(['rm', '-f', ...containerIds], { silent: true });
              removeSpinner.succeed(`Removed ${containerIds.length} containers`);
            } else {
              removeSpinner.succeed('No containers to remove');
            }
          } else {
            removeSpinner.succeed('No containers to remove');
          }
        } catch (error) {
          removeSpinner.warn(`Container cleanup warning: ${error.message}`);
        }

        if (options.servicesOnly) {
          console.log();
          console.log(chalk.green('🎉 Services-only reset completed!'));
          return;
        }

        // Step 3: Remove ALL raworc images
        console.log();
        const imageSpinner = ora('[3/8] Removing ALL raworc images...').start();

        try {
          const imageResult = await execDocker(['images', '-q', '--filter', 'reference=raworc*', '--filter', 'reference=*/raworc*'], { silent: true });

          if (imageResult.stdout.trim()) {
            const imageIds = imageResult.stdout.trim().split('\n').filter(id => id);

            if (imageIds.length > 0) {
              await execDocker(['rmi', '-f', ...imageIds], { silent: true });
              imageSpinner.succeed(`Removed ${imageIds.length} images`);
            } else {
              imageSpinner.succeed('No raworc images found');
            }
          } else {
            imageSpinner.succeed('No raworc images found');
          }
        } catch (error) {
          imageSpinner.warn(`Image cleanup warning: ${error.message}`);
        }

        // Step 4: Prune unused networks
        console.log();
        const networkSpinner = ora('[4/8] Pruning unused networks...').start();
        try {
          await execDocker(['network', 'prune', '-f'], { silent: true });
          networkSpinner.succeed('Networks pruned');
        } catch (error) {
          networkSpinner.warn(`Network cleanup warning: ${error.message}`);
        }

        // Step 5: Remove ALL Docker volumes
        console.log();
        const volumeSpinner = ora('[5/8] Removing ALL Docker volumes...').start();
        try {
          const volumeResult = await execDocker(['volume', 'ls', '-q'], { silent: true });

          if (volumeResult.stdout.trim()) {
            const volumeNames = volumeResult.stdout.trim().split('\n').filter(name => name);
            let removedCount = 0;

            if (volumeNames.length > 0) {
              // Try to remove all volumes at once first
              try {
                await execDocker(['volume', 'rm', '-f', ...volumeNames], { silent: true });
                removedCount = volumeNames.length;
              } catch (error) {
                // If batch removal fails, try individual removal
                for (const volume of volumeNames) {
                  try {
                    await execDocker(['volume', 'rm', '-f', volume], { silent: true });
                    removedCount++;
                  } catch (individualError) {
                    // Volume may be in use, continue with others
                  }
                }
              }

              volumeSpinner.succeed(`Removed ${removedCount} of ${volumeNames.length} volumes`);
            } else {
              volumeSpinner.succeed('No volumes found');
            }
          } else {
            volumeSpinner.succeed('No volumes found');
          }
        } catch (error) {
          volumeSpinner.warn(`Volume cleanup warning: ${error.message}`);
        }

        // Step 6: Prune dangling images
        console.log();
        const danglingSpinner = ora('[6/8] Pruning dangling images...').start();
        try {
          await execDocker(['image', 'prune', '-f'], { silent: true });
          danglingSpinner.succeed('Dangling images pruned');
        } catch (error) {
          danglingSpinner.warn(`Image prune warning: ${error.message}`);
        }

        // Step 7: Prune build cache
        console.log();
        const cacheSpinner = ora('[7/8] Pruning build cache...').start();
        try {
          await execDocker(['builder', 'prune', '-f'], { silent: true });
          cacheSpinner.succeed('Build cache pruned');
        } catch (error) {
          cacheSpinner.warn(`Build cache cleanup warning: ${error.message}`);
        }

        // Step 8: Show final disk usage
        console.log();
        const diskSpinner = ora('[8/8] Checking Docker disk usage...').start();
        try {
          console.log(); // Add space before disk usage output
          await execDocker(['system', 'df'], { silent: false });
          diskSpinner.succeed('Disk usage displayed');
        } catch (error) {
          diskSpinner.warn(`Disk usage warning: ${error.message}`);
        }

        console.log();
        console.log(chalk.green('🎉 Reset completed!'));
        console.log();
        console.log(chalk.blue('Summary:'));
        console.log(chalk.green('  ✓ Docker containers, volumes, and networks cleaned up'));
        console.log(chalk.green('  ✓ Raworc images removed'));
        console.log(chalk.green('  ✓ Build cache cleaned (preserving some for faster rebuilds)'));
        console.log();
        console.log(chalk.cyan('To start Raworc again:'));
        console.log('  • ' + chalk.white('raworc start'));

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};
