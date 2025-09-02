const chalk = require('chalk');
const ora = require('ora');
const docker = require('../lib/docker');

module.exports = (program) => {
  program
    .command('start')
    .description('Start Raworc services using direct Docker container management')
    .argument('[components...]', 'Components to start (server, operator, mysql)', [])
    .option('-r, --restart', 'Stop existing containers before starting (restart)')
    .action(async (components, options) => {
      try {
        console.log(chalk.blue('🚀 Starting Raworc services...'));
        
        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          console.error(chalk.red('❌ Docker is not available. Please install Docker first.'));
          process.exit(1);
        }

        // Check if images are available
        try {
          await docker.checkImages();
        } catch (error) {
          console.error(chalk.red('❌ Docker images not available:'), error.message);
          console.error(chalk.yellow('💡 Try running: raworc pull'));
          process.exit(1);
        }

        // Check for required environment variables when starting operator
        if (components.length === 0 || components.includes('operator')) {
          if (!process.env.ANTHROPIC_API_KEY) {
            console.error(chalk.red('❌ ANTHROPIC_API_KEY environment variable is required for the operator'));
            console.log('');
            console.log(chalk.yellow('💡 The operator needs an Anthropic API key to provide to session containers.'));
            console.log('   Set the environment variable and try again:');
            console.log('');
            console.log('   ' + chalk.white('export ANTHROPIC_API_KEY=sk-ant-api03-...'));
            console.log('   ' + chalk.white('raworc start'));
            process.exit(1);
          }
        }

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

        // Stop existing containers if restart requested
        if (options.restart) {
          const stopSpinner = ora('Stopping existing containers...').start();
          try {
            await docker.stop(services);
            stopSpinner.succeed('Existing containers stopped');
          } catch (error) {
            stopSpinner.warn('Some containers may not have been running');
          }
        }

        // Start services
        const startSpinner = ora('Starting services...').start();
        try {
          await docker.start(services, false);
          startSpinner.succeed('Services started successfully');
          
          // Show running services
          console.log();
          console.log(chalk.green('✅ Raworc is now running!'));
          
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
          console.log('  • Start session: ' + chalk.white('raworc session'));
          console.log();
          console.log(chalk.gray('API Server: http://localhost:9000'));
          console.log(chalk.gray('MySQL Port: 3307'));

        } catch (error) {
          startSpinner.fail(`Failed to start services: ${error.message}`);
          
          // Show troubleshooting tips
          console.log();
          console.log(chalk.yellow('💡 Troubleshooting tips:'));
          console.log('  • Check if ports 9000 and 3307 are available');
          console.log('  • Ensure Docker daemon is running');
          console.log('  • Try pulling latest images: ' + chalk.white('raworc pull'));
          console.log('  • Make sure Docker Hub is accessible');
          
          process.exit(1);
        }

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};