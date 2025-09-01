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
          console.log(chalk.red('Full reset mode: This will remove EVERYTHING from Docker'));
        }

        console.log();
        console.log(chalk.red('⚠️  This will:'));
        console.log(chalk.red('  - Stop ALL running containers'));
        console.log(chalk.red('  - Remove ALL containers (running and stopped)'));

        if (!options.servicesOnly) {
          console.log(chalk.red('  - Remove ALL Docker images'));
          console.log(chalk.red('  - Remove ALL Docker volumes'));
          console.log(chalk.red('  - Remove ALL Docker networks'));
          console.log(chalk.red('  - Clear ALL build cache'));
          console.log(chalk.red('  - Completely reset Docker to clean state'));
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

        // Step 1: Stop ALL running containers first
        console.log();
        const stopSpinner = ora('[1/9] Stopping ALL running containers...').start();

        try {
          const runningResult = await execDocker(['ps', '-q'], { silent: true });

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

        // Step 2: Remove ALL containers (running and stopped)
        console.log();
        const removeSpinner = ora('[2/9] Removing ALL containers...').start();

        try {
          const result = await execDocker(['ps', '-a', '-q'], { silent: true });

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

        // Step 3: Remove ALL Docker images
        console.log();
        const imageSpinner = ora('[3/9] Removing ALL Docker images...').start();

        try {
          const imageResult = await execDocker(['images', '-q'], { silent: true });

          if (imageResult.stdout.trim()) {
            const imageIds = imageResult.stdout.trim().split('\n').filter(id => id);

            if (imageIds.length > 0) {
              await execDocker(['rmi', '-f', ...imageIds], { silent: true });
              imageSpinner.succeed(`Removed ${imageIds.length} images`);
            } else {
              imageSpinner.succeed('No images found');
            }
          } else {
            imageSpinner.succeed('No images found');
          }
        } catch (error) {
          imageSpinner.warn(`Image cleanup warning: ${error.message}`);
        }

        // Step 4: Remove ALL custom networks
        console.log();
        const networkSpinner = ora('[4/9] Removing ALL custom networks...').start();
        try {
          // Get all custom networks (exclude default ones)
          const networkResult = await execDocker(['network', 'ls', '--filter', 'type=custom', '-q'], { silent: true });
          
          if (networkResult.stdout.trim()) {
            const networkIds = networkResult.stdout.trim().split('\n').filter(id => id);
            if (networkIds.length > 0) {
              await execDocker(['network', 'rm', ...networkIds], { silent: true });
              networkSpinner.succeed(`Removed ${networkIds.length} custom networks`);
            } else {
              networkSpinner.succeed('No custom networks found');
            }
          } else {
            // Fallback to prune
            await execDocker(['network', 'prune', '-f'], { silent: true });
            networkSpinner.succeed('Networks pruned');
          }
        } catch (error) {
          networkSpinner.warn(`Network cleanup warning: ${error.message}`);
        }

        // Step 5: Remove ALL Docker volumes
        console.log();
        const volumeSpinner = ora('[5/9] Removing ALL Docker volumes...').start();
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
        const danglingSpinner = ora('[6/9] Pruning system...').start();
        try {
          await execDocker(['system', 'prune', '-a', '-f', '--volumes'], { silent: true });
          danglingSpinner.succeed('System completely pruned');
        } catch (error) {
          danglingSpinner.warn(`Image prune warning: ${error.message}`);
        }

        // Step 7: Prune build cache
        console.log();
        const cacheSpinner = ora('[7/9] Clearing ALL build cache...').start();
        try {
          await execDocker(['builder', 'prune', '-a', '-f'], { silent: true });
          cacheSpinner.succeed('ALL build cache cleared');
        } catch (error) {
          cacheSpinner.warn(`Build cache cleanup warning: ${error.message}`);
        }

        // Step 8: Final system prune to catch anything missed
        console.log();
        const systemSpinner = ora('[8/9] Final complete system prune...').start();
        try {
          await execDocker(['system', 'prune', '-a', '-f', '--volumes'], { silent: true });
          systemSpinner.succeed('Final system prune completed');
        } catch (error) {
          systemSpinner.warn(`Final prune warning: ${error.message}`);
        }

        // Step 9: Show final disk usage
        console.log();
        const diskSpinner = ora('[9/9] Checking Docker disk usage...').start();
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
        console.log(chalk.green('  ✓ ALL containers stopped and removed'));
        console.log(chalk.green('  ✓ ALL Docker images removed'));
        console.log(chalk.green('  ✓ ALL Docker volumes removed'));
        console.log(chalk.green('  ✓ ALL custom networks removed'));
        console.log(chalk.green('  ✓ ALL build cache cleared'));
        console.log(chalk.green('  ✓ Docker system completely reset'));
        console.log();
        console.log(chalk.cyan('Docker has been completely reset to clean state'));
        console.log(chalk.cyan('To start Raworc again:'));
        console.log('  • ' + chalk.white('raworc start'));

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};
