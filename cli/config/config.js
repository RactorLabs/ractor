const path = require('path');
const os = require('os');
const fs = require('fs-extra');

class Config {
  constructor() {
    this.configDir = path.join(os.homedir(), '.raworc');
    this.configFile = path.join(this.configDir, 'config.json');
    this.authFile = path.join(this.configDir, 'auth.json');
    
    // Ensure config directory exists
    fs.ensureDirSync(this.configDir);
  }

  // Get configuration
  getConfig() {
    try {
      if (fs.existsSync(this.configFile)) {
        return fs.readJsonSync(this.configFile);
      }
    } catch (error) {
      // Return default config if file doesn't exist or is corrupted
    }
    
    return {
      server: 'http://localhost:9000',
      timeout: 30000
    };
  }

  // Save configuration
  saveConfig(config) {
    const currentConfig = this.getConfig();
    const newConfig = { ...currentConfig, ...config };
    fs.writeJsonSync(this.configFile, newConfig, { spaces: 2 });
    return newConfig;
  }

  // Get authentication info
  getAuth() {
    try {
      if (fs.existsSync(this.authFile)) {
        return fs.readJsonSync(this.authFile);
      }
    } catch (error) {
      // Return empty auth if file doesn't exist or is corrupted
    }
    
    return null;
  }

  // Save authentication info
  saveAuth(auth) {
    fs.writeJsonSync(this.authFile, auth, { spaces: 2 });
    return auth;
  }

  // Clear authentication
  clearAuth() {
    if (fs.existsSync(this.authFile)) {
      fs.removeSync(this.authFile);
    }
  }

  // Get server URL
  getServerUrl() {
    const config = this.getConfig();
    return config.server;
  }

  // Get timeout
  getTimeout() {
    const config = this.getConfig();
    return config.timeout;
  }
}

module.exports = new Config();