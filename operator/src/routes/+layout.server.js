export function load() {
  return {
    hostName: process.env.RACTOR_HOST_NAME || 'Ractor',
    hostUrl: (process.env.RACTOR_HOST_URL || 'http://localhost').replace(/\/$/, '')
  };
}
