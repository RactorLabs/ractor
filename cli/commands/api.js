const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('api <endpoint>')
    .description('Execute API requests using saved authentication')
    .option('-m, --method <method>', 'HTTP method (GET, POST, PUT, DELETE, PATCH)', 'GET')
    .option('-b, --body <body>', 'JSON body for POST/PUT/PATCH requests')
    .option('-H, --headers', 'Show response headers')
    .option('-p, --pretty', 'Pretty print JSON responses', true)
    .option('-s, --status', 'Show response status')
    .action(async (endpoint, options) => {
      await apiCommand(endpoint, options);
    });
};

async function apiCommand(endpoint, options) {
  // Show command box with API info
  display.showCommandBox(`${display.icons.api} API Request`, {
    operation: `${options.method.toUpperCase()} ${endpoint}`
  });
  
  const authData = config.getAuth();
  
  if (!authData && endpoint !== 'health') {
    display.warning('This endpoint may require authentication');
    console.log(chalk.gray('Run:'), chalk.white('raworc login'), chalk.gray('to authenticate first'));
    console.log();
  }
  
  try {
    // Parse body if provided, or use default empty object for POST requests
    let body = null;
    if (options.body) {
      try {
        body = JSON.parse(options.body);
      } catch (error) {
        display.error('Invalid JSON body: ' + error.message);
        process.exit(1);
      }
    } else if (options.method.toUpperCase() === 'POST') {
      // Default to empty object for POST requests when no body specified
      body = {};
    }

    // Make the API request
    const response = await api.request(
      options.method.toUpperCase(),
      endpoint.startsWith('/') ? endpoint : `/${endpoint}`,
      body
    );

    console.log();
    
    // Display response
    if (!response.success) {
      display.error('Request failed');
      console.error(chalk.gray('Status:'), response.status || 'Unknown');
      console.error(chalk.gray('Error:'), response.error);
      
      // Provide helpful suggestions based on status code
      if (response.status === 401) {
        console.log();
        display.info('Authentication required:');
        console.log('   Run: ' + chalk.white('raworc login'));
      } else if (response.status === 403) {
        console.log();
        display.info('Access denied:');
        console.log('   Check if you have permission for this operation');
      } else if (response.status === 404) {
        console.log();
        display.info('Endpoint not found:');
        console.log('   Check the endpoint URL spelling');
        console.log('   Available endpoints: /version, /agents, /auth');
      } else if (response.status === 0) {
        console.log();
        display.info('Connection failed:');
        console.log('   Ensure Raworc is running: ' + chalk.white('raworc start'));
        console.log('   Check server URL: ' + chalk.white(config.getServerUrl()));
      }
      
      process.exit(1);
    }

    // Success response
    display.success('Request successful');
    
    if (options.status || response.status !== 200) {
      console.log(chalk.gray('Status:'), response.status);
    }

    if (options.headers && response.headers) {
      console.log();
      console.log(chalk.blue('Response Headers:'));
      Object.entries(response.headers).forEach(([key, value]) => {
        console.log(chalk.gray(`  ${key}:`), value);
      });
    }

    if (response.data !== undefined) {
      console.log();
      console.log(chalk.blue('Response Body:'));
      
      if (options.pretty && typeof response.data === 'object') {
        console.log(JSON.stringify(response.data, null, 2));
      } else if (typeof response.data === 'string') {
        console.log(response.data);
      } else {
        console.log(JSON.stringify(response.data));
      }
    }

    // Show helpful next steps for common endpoints
    if (endpoint === 'health' && response.success) {
      console.log();
      console.log(chalk.cyan('💡 Raworc is healthy! Try these commands:'));
      console.log('  • List agents: ' + chalk.white('raworc api agents'));
      console.log('  • Check auth: ' + chalk.white('raworc api auth'));
    } else if (endpoint === 'agents' && response.success) {
      console.log();
      display.info('Next steps:');
      console.log('  • Create agent: ' + chalk.white('raworc api agents -m POST'));
      console.log('  • Interactive agent: ' + chalk.white('raworc agent'));
    }

  } catch (error) {
    display.error('Error: ' + error.message);
    process.exit(1);
  }
}