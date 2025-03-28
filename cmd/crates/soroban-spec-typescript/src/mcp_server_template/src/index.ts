#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { Contract, nativeToScVal, xdr, TransactionBuilder, rpc as SorobanRpc, Keypair, Address, BASE_FEE } from '@stellar/stellar-sdk';
import { z } from 'zod';
import { config as dotenvConfig } from 'dotenv';
import { 
  addressToScVal, 
  i128ToScVal, 
  u128ToScVal, 
  stringToSymbol, 
  numberToU64, 
  numberToI128, 
  boolToScVal, 
  u32ToScVal,
  submitTransaction,
  createSACClient,
  createContractClient
} from './helper.js';

// Load environment variables
dotenvConfig();

// Configuration
const config = {
  network: process.env.NETWORK || 'testnet',
  networkPassphrase: process.env.NETWORK_PASSPHRASE || 'Test SDF Network ; September 2015',
  rpcUrl: process.env.RPC_URL || 'https://soroban-testnet.stellar.org',
  contractId: process.env.CONTRACT_ID || '',
};

// Validate required environment variables
if (!config.contractId) {
  throw new Error('CONTRACT_ID environment variable is required');
}

const server = new SorobanRpc.Server(config.rpcUrl);

// Initialize Stellar connection
const contract = new Contract(config.contractId);

// Create MCP server instance
const mcpServer = new McpServer({
  name: "INSERT_NAME_HERE",
  version: "1.0.0",
  capabilities: {
    resources: {},
    tools: {},
  },
});

// Register contract methods as tools
// This will be populated by the generator based on contract spec
INSERT_TOOLS_HERE

async function main() {
  const transport = new StdioServerTransport();
  await mcpServer.connect(transport);
  console.error("Soroban MCP Server running on stdio");
}

main().catch((error) => {
  console.error("Fatal error in main():", error);
  process.exit(1); 
});