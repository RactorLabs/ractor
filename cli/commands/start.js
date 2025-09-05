const chalk = require('chalk');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('start')
    .description('Start Raworc services using direct Docker container management')
    .argument('[components...]', 'Components to start (server, operator, mysql)', [])
    .option('-r, --restart', 'Stop existing containers before starting (restart)')
    .action(async (components, options) => {
      try {
        // Show command box with start info
        const operation = components.length > 0 ? 
          `Start components: ${components.join(', ')}` : 
          'Start all Raworc services';
        display.showCommandBox(`${display.icons.start} Start Services`, {
          operation: operation
        });
        
        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          display.error('Docker is not available. Please install Docker first.');
          process.exit(1);
        }

        // Check if images are available
        try {
          await docker.checkImages();
        } catch (error) {
          display.error('Docker images not available: ' + error.message);
          display.info('Try running: raworc pull');
          process.exit(1);
        }

        // Note: Operator now uses Ollama. Optionally set OLLAMA_HOST or start the ollama component.

        // Map component names to service names
        const serviceMap = {
          'server': 'raworc_server',
          'operator': 'raworc_operator',
          'mysql': 'raworc_mysql',
          'ollama': 'raworc_ollama'
        };

        // Convert component names to service names
        const services = components.length > 0 
          ? components.map(comp => serviceMap[comp] || comp)
          : [];

        // Stop existing containers if restart requested
        if (options.restart) {
          display.info('Stopping existing containers...');
          try {
            await docker.stop(services);
            display.success('Existing containers stopped');
          } catch (error) {
            display.warning('Some containers may not have been running');
          }
        }

        // Start services
        display.info('Starting services...');
        try {
          await docker.start(services, false);
          display.success('Services started successfully');
          
          // Show running services
          console.log();
          display.success('Raworc is now running!');
          
          const status = await docker.status();
          if (status) {
            console.log();
            console.log(chalk.blue('Running services:'));
            console.log(status);
          }
          
          console.log();
          console.log(chalk.cyan('Next steps:'));
          console.log('  • Authenticate: ' + chalk.white('raworc login --user admin --pass admin'));
          console.log('  • Check health: ' + chalk.white('raworc api version'));
          console.log('  • Start agent: ' + chalk.white('raworc agent create'));
          console.log();
          console.log(chalk.gray('API Server: http://localhost:9000'));
          console.log(chalk.gray('MySQL Port: 3307'));

        } catch (error) {
          display.error(`Failed to start services: ${error.message}`);
          
          // Show troubleshooting tips
          console.log();
          display.info('Troubleshooting tips:');
          console.log('  • Check if ports 9000 and 3307 are available');
          console.log('  • Ensure Docker daemon is running');
          console.log('  • Try pulling latest images: ' + chalk.white('raworc pull'));
          console.log('  • Make sure Docker Hub is accessible');
          
          process.exit(1);
        }

      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
