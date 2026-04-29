import os from 'node:os';

export function load() {
  const ifaces = os.networkInterfaces();
  const ips: { name: string; address: string }[] = [];
  for (const [name, addrs] of Object.entries(ifaces) as [string, os.NetworkInterfaceInfo[]][]) {
    for (const addr of addrs) {
      if (addr.family === 'IPv4' && !addr.internal) {
        ips.push({ name, address: addr.address });
      }
    }
  }
  return { ips };
}
