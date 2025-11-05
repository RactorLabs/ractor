const chalk = require('chalk');
const config = require('../config/config');

// Flat icon mappings to replace emojis
const icons = {
  // Status icons
  success: '✓',      // replaces ✅
  error: '✗',        // replaces ❌ ✗
  warning: '⚠',      // replaces ⚠️
  info: 'ℹ',         // replaces ℹ
  
  // Action icons  
  start: '▶',        // replaces 🚀
  stop: '◼',         // replaces 🛑
  clean: '◯',        // replaces 🎉
  pull: '⇣',         // replaces download
  reset: '⟲',        // replaces 🔄
  
  // Communication icons
  api: '◈',          // replaces 🌐
  sandbox: '◊',        // replaces sandbox box
  history: '≡',      // replaces 📜
  chat: '◉',         // replaces 💬
  
  // Auth icons
  auth: '◎',         // auth related
  token: '⬟',        // token operations
  user: '◐'          // user operations
};

// Create a standardized command box
function showCommandBox(title, info = {}) {
  const authData = config.getAuth();
  const configData = config.getConfig();
  
  // Build info lines based on what's provided
  const lines = [title];
  
  if (info.api !== false) {
    lines.push(`API: ${configData.api || 'http://localhost:9000'}`);
  }
  
  if (info.user !== false) {
    const userName = authData?.user?.user || 'Not authenticated';
    const userType = authData?.user?.type ? ` (${authData.user.type})` : '';
    lines.push(`User: ${userName}${userType}`);
  }
  
  if (info.sandbox) {
    lines.push(`Sandbox: ${info.sandbox}`);
  }
  
  if (info.operation) {
    lines.push(`Operation: ${info.operation}`);
  }
  
  if (info.target) {
    lines.push(`Target: ${info.target}`);
  }
  
  // Calculate box width
  const maxWidth = Math.max(...lines.map(line => line.length));
  const boxWidth = maxWidth + 4; // Add padding (2 spaces + 2 borders)
  
  // Create box
  console.log();
  console.log('┌' + '─'.repeat(boxWidth - 2) + '┐');
  
  lines.forEach(line => {
    const padding = ' '.repeat(boxWidth - line.length - 4); // Account for │ space content space │
    console.log(`│ ${line}${padding} │`);
  });
  
  console.log('└' + '─'.repeat(boxWidth - 2) + '┘');
  console.log();
}

// Status display helpers
function success(message) {
  console.log(chalk.green(icons.success) + ' ' + message);
}

function error(message) {
  console.log(chalk.red(icons.error) + ' ' + message);
}

function warning(message) {
  console.log(chalk.yellow(icons.warning) + ' ' + message);
}

function info(message) {
  console.log(chalk.blue(icons.info) + ' ' + message);
}

function status(type, message) {
  const colorMap = {
    success: chalk.green,
    error: chalk.red,
    warning: chalk.yellow,
    info: chalk.blue
  };
  
  const iconMap = {
    success: icons.success,
    error: icons.error,
    warning: icons.warning,
    info: icons.info
  };
  
  const color = colorMap[type] || chalk.gray;
  const icon = iconMap[type] || icons.info;
  
  console.log(color(icon) + ' ' + message);
}

module.exports = {
  icons,
  showCommandBox,
  success,
  error,
  warning,
  info,
  status
};
