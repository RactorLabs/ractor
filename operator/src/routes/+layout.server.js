export function load() {
  return {
    hostName: process.env.TSBX_HOST_NAME || 'TaskSandbox',
    hostUrl: (process.env.TSBX_HOST_URL || 'http://localhost').replace(/\/$/, '')
  };
}
