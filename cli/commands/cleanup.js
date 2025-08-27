const chalk = require('chalk');
const ora = require('ora');
const api = require('../lib/api');
const config = require('../config/config');

module.exports = (program) => {
  program
    .command('cleanup')
    .description('Clean up all live sessions')
    .option('-y, --yes', 'Confirm without prompting')
    .action(async (options) => {
      try {
        // Check authentication
        const authData = config.getAuth();
        if (!authData) {
          console.log(chalk.red('❌ Authentication required'));
          console.log('Run: ' + chalk.white('raworc auth login') + ' to authenticate first');
          process.exit(1);
        }

        console.log(chalk.blue('🧹 Session Cleanup'));
        console.log();

        // Get all sessions
        const listSpinner = ora('Fetching all sessions...').start();
        
        const sessionsResponse = await api.get('/sessions');
        
        if (!sessionsResponse.success) {
          listSpinner.fail('Failed to fetch sessions');
          console.error(chalk.red('Error:'), sessionsResponse.error);
          process.exit(1);
        }

        const sessions = Array.isArray(sessionsResponse.data) ? sessionsResponse.data : sessionsResponse.data.sessions || [];
        listSpinner.stop();

        if (sessions.length === 0) {
          console.log(chalk.green('✅ No sessions to clean up'));
          return;
        }

        console.log(chalk.yellow(`Found ${sessions.length} session(s):`));
        sessions.forEach((session, i) => {
          const status = session.state || session.status || 'unknown';
          const space = session.space || 'unknown';
          console.log(`  ${i + 1}. ${session.id} (${status}) in space '${space}'`);
        });

        console.log();

        // Confirm unless --yes flag is provided
        if (!options.yes) {
          const readline = require('readline');
          const rl = readline.createInterface({
            input: process.stdin,
            output: process.stdout
          });

          const answer = await new Promise(resolve => {
            rl.question(chalk.yellow(`Delete all ${sessions.length} sessions? [y/N]: `), resolve);
          });
          
          rl.close();

          if (!answer.match(/^[Yy]$/)) {
            console.log(chalk.blue('Operation cancelled'));
            return;
          }
        }

        // Delete all sessions
        console.log();
        const cleanupSpinner = ora(`Deleting ${sessions.length} sessions...`).start();

        let deletedCount = 0;
        let failedCount = 0;

        for (const session of sessions) {
          try {
            const deleteResponse = await api.delete(`/sessions/${session.id}`);
            
            if (deleteResponse.success) {
              deletedCount++;
            } else {
              failedCount++;
              console.log(chalk.yellow(`Warning: Failed to delete session ${session.id}: ${deleteResponse.error}`));
            }
            
          } catch (error) {
            failedCount++;
            console.log(chalk.yellow(`Warning: Failed to delete session ${session.id}: ${error.message}`));
          }
        }

        cleanupSpinner.stop();

        console.log();
        if (deletedCount > 0) {
          console.log(chalk.green(`✅ Successfully deleted ${deletedCount} sessions`));
        }
        
        if (failedCount > 0) {
          console.log(chalk.yellow(`⚠️ Failed to delete ${failedCount} sessions`));
        }

        if (deletedCount === sessions.length) {
          console.log(chalk.green('🎉 All sessions cleaned up!'));
        }

        console.log();
        console.log(chalk.blue('Session containers may still be running.'));
        console.log(chalk.gray('To clean up containers: ' + chalk.white('raworc stop --cleanup')));

      } catch (error) {
        console.error(chalk.red('❌ Error:'), error.message);
        process.exit(1);
      }
    });
};