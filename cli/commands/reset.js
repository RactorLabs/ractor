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

        // Step 1: Stop and remove all raworc containers
        const resetSpinner = ora('Stopping and removing Raworc containers...').start();
        
        try {
          // Get all raworc containers
          const result = await execDocker(['ps', '-a', '-q', '--filter', 'name=raworc'], { silent: true });
          
          if (result.stdout.trim()) {
            const containerIds = result.stdout.trim().split('\n').filter(id => id);
            
            if (containerIds.length > 0) {
              await execDocker(['rm', '-f', ...containerIds], { silent: true });
              resetSpinner.succeed(`Removed ${containerIds.length} containers`);
            } else {
              resetSpinner.succeed('No containers to remove');
            }
          } else {
            resetSpinner.succeed('No containers to remove');
          }
        } catch (error) {
          resetSpinner.warn(`Container cleanup warning: ${error.message}`);
        }

        if (options.servicesOnly) {
          console.log();
          console.log(chalk.green('🎉 Services-only reset completed!'));
          return;
        }

        // Step 2: Remove volumes
        const volumeSpinner = ora('Removing Docker volumes...').start();
        try {
          const volumes = ['mysql_data', 'operator_data'];
          let removedCount = 0;
          
          for (const volume of volumes) {
            try {
              await execDocker(['volume', 'rm', volume], { silent: true });
              removedCount++;
            } catch (error) {
              // Volume might not exist or be in use
            }
          }
          
          volumeSpinner.succeed(`Removed ${removedCount} volumes`);
        } catch (error) {
          volumeSpinner.warn(`Volume cleanup warning: ${error.message}`);
        }

        // Step 3: Prune networks
        const networkSpinner = ora('Pruning unused networks...').start();
        try {
          await execDocker(['network', 'prune', '-f'], { silent: true });
          networkSpinner.succeed('Networks pruned');
        } catch (error) {
          networkSpinner.warn(`Network cleanup warning: ${error.message}`);
        }

        // Step 4: Prune build cache
        const cacheSpinner = ora('Pruning build cache...').start();
        try {
          await execDocker(['builder', 'prune', '-f'], { silent: true });
          cacheSpinner.succeed('Build cache pruned');
        } catch (error) {
          cacheSpinner.warn(`Build cache cleanup warning: ${error.message}`);
        }

        console.log();
        console.log(chalk.green('🎉 Reset completed!'));
        console.log();
        console.log(chalk.blue('Summary:'));
        console.log(chalk.green('  ✓ Docker containers, volumes, and networks cleaned up'));
        console.log(chalk.green('  ✓ Build cache cleaned'));
        console.log();
        console.log(chalk.cyan('To start Raworc again:'));
        console.log('  • Start services: ' + chalk.white('raworc start --pull'));

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};