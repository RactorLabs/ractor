const chalk = require('chalk');
const api = require('../lib/api');
const config = require('../config/config');

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
  // Show authentication status first
  const authData = config.getAuth();
  const serverUrl = config.getServerUrl();
  
  console.log(chalk.blue('🌐 API Request'));
  console.log(chalk.gray('Server:'), serverUrl);
  console.log(chalk.gray('Authentication:'), authData ? chalk.green('✓ Authenticated') : chalk.red('✗ Not authenticated'));
  
  if (!authData && endpoint !== 'health') {
    console.log();
    console.log(chalk.yellow('⚠️ This endpoint may require authentication'));
    console.log('   Run: ' + chalk.white('raworc auth:login') + ' to authenticate first');
  }
  
  console.log();
  console.log(chalk.gray('Request:'), chalk.white(`${options.method.toUpperCase()} ${endpoint}`));
  
  try {
    // Parse body if provided, or use default empty object for POST requests
    let body = null;
    if (options.body) {
      try {
        body = JSON.parse(options.body);
      } catch (error) {
        console.error(chalk.red('❌ Invalid JSON body:'), error.message);
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
      console.error(chalk.red('❌ Request failed'));
      console.error(chalk.gray('Status:'), response.status || 'Unknown');
      console.error(chalk.gray('Error:'), response.error);
      
      // Provide helpful suggestions based on status code
      if (response.status === 401) {
        console.log();
        console.log(chalk.yellow('💡 Authentication required:'));
        console.log('   Run: ' + chalk.white('raworc auth:login'));
      } else if (response.status === 403) {
        console.log();
        console.log(chalk.yellow('💡 Access denied:'));
        console.log('   Check if you have permission for this operation');
      } else if (response.status === 404) {
        console.log();
        console.log(chalk.yellow('💡 Endpoint not found:'));
        console.log('   Check the endpoint URL spelling');
        console.log('   Available endpoints: /version, /sessions, /auth');
      } else if (response.status === 0) {
        console.log();
        console.log(chalk.yellow('💡 Connection failed:'));
        console.log('   Ensure Raworc is running: ' + chalk.white('raworc start'));
        console.log('   Check server URL: ' + chalk.white(serverUrl));
      }
      
      process.exit(1);
    }

    // Success response
    console.log(chalk.green('✅ Request successful'));
    
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
      console.log('  • List sessions: ' + chalk.white('raworc api sessions'));
      console.log('  • Check auth: ' + chalk.white('raworc api auth/me'));
    } else if (endpoint === 'sessions' && response.success) {
      console.log();
      console.log(chalk.cyan('💡 Next steps:'));
      console.log('  • Create session: ' + chalk.white('raworc api sessions -m POST'));
      console.log('  • Interactive session: ' + chalk.white('raworc session'));
    }

  } catch (error) {
    console.error(chalk.red('❌ Error:'), error.message);
    process.exit(1);
  }
}