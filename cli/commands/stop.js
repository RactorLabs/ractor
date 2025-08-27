const chalk = require('chalk');
const ora = require('ora');
const docker = require('../lib/docker');

module.exports = (program) => {
  program
    .command('stop')
    .description('Stop Raworc services using direct Docker container management')
    .argument('[components...]', 'Components to stop (server, operator, mysql)', [])
    .option('-c, --cleanup', 'Clean up session containers after stopping')
    .action(async (components, options) => {
      try {
        console.log(chalk.blue('🛑 Stopping Raworc services...'));
        
        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          console.error(chalk.red('❌ Docker is not available.'));
          process.exit(1);
        }

        // CLI manages containers directly - no additional checks needed

        // Map component names to service names
        const serviceMap = {
          'server': 'raworc_server',
          'operator': 'raworc_operator',
          'mysql': 'raworc_mysql'
        };

        // Convert component names to service names
        const services = components.length > 0 
          ? components.map(comp => serviceMap[comp] || comp)
          : [];

        // Stop services
        const stopSpinner = ora('Stopping services...').start();
        try {
          await docker.stop(services, options.cleanup);
          
          if (services.length > 0) {
            stopSpinner.succeed(`Stopped services: ${services.join(', ')}`);
          } else {
            stopSpinner.succeed('All Raworc services stopped');
          }
          
        } catch (error) {
          stopSpinner.fail(`Failed to stop services: ${error.message}`);
          process.exit(1);
        }

        // Note: Cleanup is now handled by the stop script if --cleanup was passed

        console.log();
        console.log(chalk.green('✅ Services stopped successfully'));
        
        if (!options.cleanup && !services.length) {
          console.log();
          console.log(chalk.yellow('💡 Tip: Use --cleanup to also remove session containers'));
        }

        // Show final status
        const status = await docker.status();
        if (status && status.trim()) {
          console.log();
          console.log(chalk.blue('Remaining services:'));
          console.log(status);
        }

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};