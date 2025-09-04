const chalk = require('chalk');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('stop')
    .description('Stop Raworc services using direct Docker container management')
    .argument('[components...]', 'Components to stop (server, operator, mysql)', [])
    .option('-c, --cleanup', 'Clean up agent containers after stopping')
    .action(async (components, options) => {
      try {
        // Show command box with stop info
        const operation = components.length > 0 ? 
          `Stop components: ${components.join(', ')}` : 
          'Stop all Raworc services';
        display.showCommandBox(`${display.icons.stop} Stop Services`, {
          operation: operation
        });
        
        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          display.error('Docker is not available.');
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
        display.info('Stopping services...');
        try {
          await docker.stop(services, options.cleanup);
          
          if (services.length > 0) {
            display.success(`Stopped services: ${services.join(', ')}`);
          } else {
            display.success('All Raworc services stopped');
          }
          
        } catch (error) {
          display.error(`Failed to stop services: ${error.message}`);
          process.exit(1);
        }

        // Note: Cleanup is now handled by the stop script if --cleanup was passed
        
        if (!options.cleanup && !services.length) {
          console.log();
          display.info('Tip: Use --cleanup to also remove agent containers');
        }

        // Show final status
        const status = await docker.status();
        if (status && status.trim()) {
          console.log();
          console.log(chalk.blue('Remaining services:'));
          console.log(status);
        }

      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};