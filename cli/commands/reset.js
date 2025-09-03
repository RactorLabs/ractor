const chalk = require('chalk');
const { spawn } = require('child_process');
const readline = require('readline');
const display = require('../lib/display');

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
        // Show command box with reset info
        const operation = options.servicesOnly ? 
          'Stop services only (skip Docker cleanup)' : 
          'Complete Docker reset (DESTRUCTIVE)';
        display.showCommandBox(`${display.icons.reset} Complete Reset`, {
          operation: operation
        });

        if (options.servicesOnly) {
          display.info('Services-only mode: Will stop services but skip Docker cleanup');
        } else {
          display.warning('Full reset mode: This will remove EVERYTHING from Docker');
        }

        console.log();
        display.warning('This will:');
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
            display.info('Operation cancelled');
            return;
          }
        }

        console.log();
        display.info('Starting reset process...');

        // Step 1: Stop ALL running containers first
        console.log();
        display.info('[1/9] Stopping ALL running containers...');

        try {
          const runningResult = await execDocker(['ps', '-q'], { silent: true });

          if (runningResult.stdout.trim()) {
            const runningIds = runningResult.stdout.trim().split('\n').filter(id => id);
            if (runningIds.length > 0) {
              await execDocker(['stop', ...runningIds], { silent: true });
              display.success(`Stopped ${runningIds.length} running containers`);
            } else {
              display.success('No running containers found');
            }
          } else {
            display.success('No running containers found');
          }
        } catch (error) {
          display.warning(`Stop warning: ${error.message}`);
        }

        // Step 2: Remove ALL containers (running and stopped)
        console.log();
        display.info('[2/9] Removing ALL containers...');

        try {
          const result = await execDocker(['ps', '-a', '-q'], { silent: true });

          if (result.stdout.trim()) {
            const containerIds = result.stdout.trim().split('\n').filter(id => id);

            if (containerIds.length > 0) {
              await execDocker(['rm', '-f', ...containerIds], { silent: true });
              display.success(`Removed ${containerIds.length} containers`);
            } else {
              display.success('No containers to remove');
            }
          } else {
            display.success('No containers to remove');
          }
        } catch (error) {
          display.warning(`Container cleanup warning: ${error.message}`);
        }

        if (options.servicesOnly) {
          console.log();
          display.success('Services-only reset completed!');
          return;
        }

        // Step 3: Remove ALL Docker images
        console.log();
        display.info('[3/9] Removing ALL Docker images...');

        try {
          const imageResult = await execDocker(['images', '-q'], { silent: true });

          if (imageResult.stdout.trim()) {
            const imageIds = imageResult.stdout.trim().split('\n').filter(id => id);

            if (imageIds.length > 0) {
              await execDocker(['rmi', '-f', ...imageIds], { silent: true });
              display.success(`Removed ${imageIds.length} images`);
            } else {
              display.success('No images found');
            }
          } else {
            display.success('No images found');
          }
        } catch (error) {
          display.warning(`Image cleanup warning: ${error.message}`);
        }

        // Step 4: Remove ALL custom networks
        console.log();
        display.info('[4/9] Removing ALL custom networks...');
        try {
          // Get all custom networks (exclude default ones)
          const networkResult = await execDocker(['network', 'ls', '--filter', 'type=custom', '-q'], { silent: true });
          
          if (networkResult.stdout.trim()) {
            const networkIds = networkResult.stdout.trim().split('\n').filter(id => id);
            if (networkIds.length > 0) {
              await execDocker(['network', 'rm', ...networkIds], { silent: true });
              display.success(`Removed ${networkIds.length} custom networks`);
            } else {
              display.success('No custom networks found');
            }
          } else {
            // Fallback to prune
            await execDocker(['network', 'prune', '-f'], { silent: true });
            display.success('Networks pruned');
          }
        } catch (error) {
          display.warning(`Network cleanup warning: ${error.message}`);
        }

        // Step 5: Remove ALL Docker volumes
        console.log();
        display.info('[5/9] Removing ALL Docker volumes...');
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

              display.success(`Removed ${removedCount} of ${volumeNames.length} volumes`);
            } else {
              display.success('No volumes found');
            }
          } else {
            display.success('No volumes found');
          }
        } catch (error) {
          display.warning(`Volume cleanup warning: ${error.message}`);
        }

        // Step 6: Prune dangling images
        console.log();
        display.info('[6/9] Pruning system...');
        try {
          await execDocker(['system', 'prune', '-a', '-f', '--volumes'], { silent: true });
          display.success('System completely pruned');
        } catch (error) {
          display.warning(`Image prune warning: ${error.message}`);
        }

        // Step 7: Prune build cache
        console.log();
        display.info('[7/9] Clearing ALL build cache...');
        try {
          await execDocker(['builder', 'prune', '-a', '-f'], { silent: true });
          display.success('ALL build cache cleared');
        } catch (error) {
          display.warning(`Build cache cleanup warning: ${error.message}`);
        }

        // Step 8: Final system prune to catch anything missed
        console.log();
        display.info('[8/9] Final complete system prune...');
        try {
          await execDocker(['system', 'prune', '-a', '-f', '--volumes'], { silent: true });
          display.success('Final system prune completed');
        } catch (error) {
          display.warning(`Final prune warning: ${error.message}`);
        }

        // Step 9: Show final disk usage
        console.log();
        display.info('[9/9] Checking Docker disk usage...');
        try {
          console.log(); // Add space before disk usage output
          await execDocker(['system', 'df'], { silent: false });
          display.success('Disk usage displayed');
        } catch (error) {
          display.warning(`Disk usage warning: ${error.message}`);
        }

        console.log();
        display.success('Reset completed!');
        console.log();
        console.log(chalk.blue('Summary:'));
        console.log(chalk.green('  ' + display.icons.success + ' ALL containers stopped and removed'));
        console.log(chalk.green('  ' + display.icons.success + ' ALL Docker images removed'));
        console.log(chalk.green('  ' + display.icons.success + ' ALL Docker volumes removed'));
        console.log(chalk.green('  ' + display.icons.success + ' ALL custom networks removed'));
        console.log(chalk.green('  ' + display.icons.success + ' ALL build cache cleared'));
        console.log(chalk.green('  ' + display.icons.success + ' Docker system completely reset'));
        console.log();
        console.log(chalk.cyan('Docker has been completely reset to clean state'));
        console.log(chalk.cyan('To start Raworc again:'));
        console.log('  • ' + chalk.white('raworc start'));

      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
