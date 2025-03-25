export interface ServerConfig {
  network: string;
  networkPassphrase: string;
  rpcUrl: string;
  contractId: string;
  port: number;
  host: string;
  adminKey?: string;
  rateLimitWindow?: string;
  rateLimitMax?: number;
}

export function loadConfig(): ServerConfig {
  return {
    network: process.env.NETWORK || 'testnet',
    networkPassphrase: process.env.NETWORK_PASSPHRASE || 'Test SDF Network ; September 2015',
    rpcUrl: process.env.RPC_URL || 'https://soroban-testnet.stellar.org',
    contractId: process.env.CONTRACT_ID || '',
    port: parseInt(process.env.PORT || '3000', 10),
    host: process.env.HOST || 'localhost',
    adminKey: process.env.ADMIN_KEY,
    rateLimitWindow: process.env.RATE_LIMIT_WINDOW,
    rateLimitMax: process.env.RATE_LIMIT_MAX ? parseInt(process.env.RATE_LIMIT_MAX, 10) : undefined
  };
} 