export function load() {
  return {
    hostName: process.env.TSBX_HOST_NAME || 'TSBX',
    hostUrl: (process.env.TSBX_HOST_URL || 'http://localhost').replace(/\/$/, '')
  };
}
