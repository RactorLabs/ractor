const axios = require('axios');
const config = require('../config/config');
const chalk = require('chalk');

class ApiClient {
  constructor() {
    this.baseURL = null;
    this.token = null;
    this.timeout = 30000;
    this.updateConfig();
  }

  updateConfig() {
    const configData = config.getConfig();
    const authData = config.getAuth();
    
    this.baseURL = configData.server;
    this.timeout = configData.timeout || 30000;
    this.token = authData?.token || null;
  }

  // Create axios instance with current config
  createClient() {
    const clientConfig = {
      baseURL: this.baseURL,
      timeout: this.timeout,
      headers: {
        'Content-Type': 'application/json',
      }
    };

    if (this.token) {
      clientConfig.headers.Authorization = `Bearer ${this.token}`;
    }

    return axios.create(clientConfig);
  }

  // Make API request
  async request(method, endpoint, data = null, options = {}) {
    this.updateConfig(); // Refresh config before each request
    
    const client = this.createClient();
    
    // Debug logging for PUT requests
    if (method === 'PUT' && data) {
      console.log('DEBUG: PUT request data:', JSON.stringify(data, null, 2));
    }
    
    // Ensure endpoint starts with /api/v0
    if (!endpoint.startsWith('/api/v0')) {
      endpoint = `/api/v0${endpoint.startsWith('/') ? '' : '/'}${endpoint}`;
    }
    
    try {
      const config = {
        method,
        url: endpoint,
        ...options
      };

      if (data) {
        if (method.toLowerCase() === 'get') {
          config.params = data;
        } else {
          config.data = data;
        }
      }

      const response = await client(config);
      return {
        success: true,
        data: response.data,
        status: response.status,
        headers: response.headers
      };
    } catch (error) {
      if (error.response) {
        // Server responded with error status
        return {
          success: false,
          error: error.response.data?.error || error.response.statusText,
          status: error.response.status,
          data: error.response.data
        };
      } else if (error.request) {
        // Request made but no response received
        return {
          success: false,
          error: 'No response from server. Is Raworc running?',
          status: 0
        };
      } else {
        // Something else happened
        return {
          success: false,
          error: error.message,
          status: 0
        };
      }
    }
  }

  // Convenience methods
  async get(endpoint, params = null, options = {}) {
    return this.request('GET', endpoint, params, options);
  }

  async post(endpoint, data = null, options = {}) {
    return this.request('POST', endpoint, data, options);
  }

  async put(endpoint, data = null, options = {}) {
    return this.request('PUT', endpoint, data, options);
  }

  async delete(endpoint, data = null, options = {}) {
    return this.request('DELETE', endpoint, data, options);
  }

  async patch(endpoint, data = null, options = {}) {
    return this.request('PATCH', endpoint, data, options);
  }

  // Authentication methods
  async loginAndSave(credentials) {
    const response = await this.post(`/operators/${credentials.user}/login`, { pass: credentials.pass });
    
    if (response.success) {
      // Structure the user data consistently 
      const userData = {
        user: response.data.user,
        role: response.data.role || 'Unknown',
        type: 'Operator'
      };
      
      const authData = {
        token: response.data.token,
        user: userData,
        expires: response.data.expires,
        server: this.baseURL
      };
      
      config.saveAuth(authData);
      this.token = response.data.token;
    }
    
    return response;
  }

  async login(credentials) {
    // Just return the login response without saving auth data
    const response = await this.post(`/operators/${credentials.user}/login`, { pass: credentials.pass });
    return response;
  }

  async loginWithToken(token, server = null) {
    if (server) {
      config.saveConfig({ server });
      this.baseURL = server;
    }
    
    // Test token by making a request to /auth
    // We need to temporarily save the old token and use the new one
    const oldToken = this.token;
    this.token = token;
    
    // Create a special client for this request without updateConfig()
    const client = this.createClient();
    const endpoint = '/api/v0/auth';
    
    let response;
    try {
      const axiosResponse = await client.get(endpoint);
      response = {
        success: true,
        data: axiosResponse.data,
        status: axiosResponse.status,
        headers: axiosResponse.headers
      };
    } catch (error) {
      if (error.response) {
        response = {
          success: false,
          error: error.response.data?.error || error.response.statusText,
          status: error.response.status,
          data: error.response.data
        };
      } else if (error.request) {
        response = {
          success: false,
          error: 'No response from server. Is Raworc running?',
          status: 0
        };
      } else {
        response = {
          success: false,
          error: error.message,
          status: 0
        };
      }
    }
    
    if (response.success) {
      // Structure the user data to match what auth status expects
      const userData = {
        user: response.data.user,
        role: response.data.type === 'Operator' ? 'admin' : 'user',
        type: response.data.type
      };
      
      const authData = {
        token,
        user: userData,
        server: this.baseURL
      };
      
      config.saveAuth(authData);
      // Keep the new token
      this.token = token;
    } else {
      // Restore the old token on failure
      this.token = oldToken;
    }
    
    return response;
  }

  // Check authentication status
  async checkAuth() {
    if (!this.token) {
      return {
        success: false,
        error: 'Not authenticated'
      };
    }

    const response = await this.get('/auth');
    return response;
  }

  // Logout
  logout() {
    config.clearAuth();
    this.token = null;
  }

  // Health check
  async health() {
    // Don't use authentication for health check
    const tempToken = this.token;
    this.token = null;
    
    const response = await this.get('/version');
    
    // Restore token
    this.token = tempToken;
    
    return response;
  }

  // Format and display response
  static formatResponse(response, options = {}) {
    if (!response.success) {
      console.error(chalk.red('Error:'), response.error);
      if (response.status) {
        console.error(chalk.gray(`Status: ${response.status}`));
      }
      return;
    }

    if (options.headers) {
      console.log(chalk.blue('Response Headers:'));
      Object.entries(response.headers || {}).forEach(([key, value]) => {
        console.log(chalk.gray(`  ${key}: ${value}`));
      });
      console.log();
    }

    if (options.pretty && typeof response.data === 'object') {
      console.log(JSON.stringify(response.data, null, 2));
    } else if (response.data) {
      console.log(response.data);
    }

    if (options.status) {
      console.log(chalk.green(`Status: ${response.status}`));
    }
  }
}

module.exports = new ApiClient();