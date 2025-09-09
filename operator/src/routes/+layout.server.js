export function load() {
  return {
    hostName: process.env.RAWORC_HOST_NAME || 'Raworc',
    hostUrl: (process.env.RAWORC_HOST_URL || 'http://localhost').replace(/\/$/, '')
  };
}
