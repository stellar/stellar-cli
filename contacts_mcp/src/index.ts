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
  createSACClient
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
  name: "contacts-mcp-server",
  version: "1.0.0",
  capabilities: {
    resources: {},
    tools: {},
  },
});

// Register contract methods as tools
// This will be populated by the generator based on contract spec
mcpServer.tool(
  "add_contact",
  "Add a new contact to the user's contact list",
  {
    owner: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    nickname: z.string().describe("Stellar symbol/enum value - Converts to: xdr.ScVal.scvSymbol(i)"),
    address: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
  },
  async (params) => {
    try {
      // Get the contract client
      const client = await createContractClient(config.contractId, config.networkPassphrase, config.rpcUrl);

      let txXdr: string;
      const functionName = 'add_contact';
      const functionToCall = client[functionName];

      // For WASM contracts, we need to convert parameters to ScVal
      const orderedParams = ['owner', 'nickname', 'address'];
      const scValParams = orderedParams.map(paramName => {
        const value = params[paramName as keyof typeof params];
        if (value === undefined) {
          throw new Error(`Missing required parameter: ${paramName}`);
        }
        // Use appropriate conversion based on parameter type
        switch(paramName) {
          case 'owner':
            return addressToScVal(value as string);
          case 'nickname':
            return stringToSymbol(value as string);
          case 'address':
            return addressToScVal(value as string);
          default:
            return nativeToScVal(value);
        }
      });
      const result = await functionToCall(...scValParams);
      txXdr = result.toXDR();

      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" },
        ]
      }
      
    } catch (error: any) {
      return {
        content: [{ 
          type: "text", 
          text: `Error executing add_contact: ${error.message}${error.cause ? `\nCause: ${error.cause}` : ''}` 
        }]
      };
    }
  }
);
mcpServer.tool(
  "get_contact",
  "Get a specific contact by nickname",
  {
    owner: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    nickname: z.string().describe("Stellar symbol/enum value - Converts to: xdr.ScVal.scvSymbol(i)"),
  },
  async (params) => {
    try {
      // Get the contract client
      const client = await createContractClient(config.contractId, config.networkPassphrase, config.rpcUrl);

      let txXdr: string;
      const functionName = 'get_contact';
      const functionToCall = client[functionName];

      // For WASM contracts, we need to convert parameters to ScVal
      const orderedParams = ['owner', 'nickname'];
      const scValParams = orderedParams.map(paramName => {
        const value = params[paramName as keyof typeof params];
        if (value === undefined) {
          throw new Error(`Missing required parameter: ${paramName}`);
        }
        // Use appropriate conversion based on parameter type
        switch(paramName) {
          case 'owner':
            return addressToScVal(value as string);
          case 'nickname':
            return stringToSymbol(value as string);
          default:
            return nativeToScVal(value);
        }
      });
      const result = await functionToCall(...scValParams);
      txXdr = result.toXDR();

      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" },
        ]
      }
      
    } catch (error: any) {
      return {
        content: [{ 
          type: "text", 
          text: `Error executing get_contact: ${error.message}${error.cause ? `\nCause: ${error.cause}` : ''}` 
        }]
      };
    }
  }
);
mcpServer.tool(
  "edit_contact",
  "Edit an existing contact's address",
  {
    owner: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    nickname: z.string().describe("Stellar symbol/enum value - Converts to: xdr.ScVal.scvSymbol(i)"),
    new_address: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
  },
  async (params) => {
    try {
      // Get the contract client
      const client = await createContractClient(config.contractId, config.networkPassphrase, config.rpcUrl);

      let txXdr: string;
      const functionName = 'edit_contact';
      const functionToCall = client[functionName];

      // For WASM contracts, we need to convert parameters to ScVal
      const orderedParams = ['owner', 'nickname', 'new_address'];
      const scValParams = orderedParams.map(paramName => {
        const value = params[paramName as keyof typeof params];
        if (value === undefined) {
          throw new Error(`Missing required parameter: ${paramName}`);
        }
        // Use appropriate conversion based on parameter type
        switch(paramName) {
          case 'owner':
            return addressToScVal(value as string);
          case 'nickname':
            return stringToSymbol(value as string);
          case 'new_address':
            return addressToScVal(value as string);
          default:
            return nativeToScVal(value);
        }
      });
      const result = await functionToCall(...scValParams);
      txXdr = result.toXDR();

      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" },
        ]
      }
      
    } catch (error: any) {
      return {
        content: [{ 
          type: "text", 
          text: `Error executing edit_contact: ${error.message}${error.cause ? `\nCause: ${error.cause}` : ''}` 
        }]
      };
    }
  }
);
mcpServer.tool(
  "delete_contact",
  "Delete a contact",
  {
    owner: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    nickname: z.string().describe("Stellar symbol/enum value - Converts to: xdr.ScVal.scvSymbol(i)"),
  },
  async (params) => {
    try {
      // Get the contract client
      const client = await createContractClient(config.contractId, config.networkPassphrase, config.rpcUrl);

      let txXdr: string;
      const functionName = 'delete_contact';
      const functionToCall = client[functionName];

      // For WASM contracts, we need to convert parameters to ScVal
      const orderedParams = ['owner', 'nickname'];
      const scValParams = orderedParams.map(paramName => {
        const value = params[paramName as keyof typeof params];
        if (value === undefined) {
          throw new Error(`Missing required parameter: ${paramName}`);
        }
        // Use appropriate conversion based on parameter type
        switch(paramName) {
          case 'owner':
            return addressToScVal(value as string);
          case 'nickname':
            return stringToSymbol(value as string);
          default:
            return nativeToScVal(value);
        }
      });
      const result = await functionToCall(...scValParams);
      txXdr = result.toXDR();

      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" },
        ]
      }
      
    } catch (error: any) {
      return {
        content: [{ 
          type: "text", 
          text: `Error executing delete_contact: ${error.message}${error.cause ? `\nCause: ${error.cause}` : ''}` 
        }]
      };
    }
  }
);
mcpServer.tool(
  "list_contacts",
  "Get all contacts for an owner",
  {
    owner: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
  },
  async (params) => {
    try {
      // Get the contract client
      const client = await createContractClient(config.contractId, config.networkPassphrase, config.rpcUrl);

      let txXdr: string;
      const functionName = 'list_contacts';
      const functionToCall = client[functionName];

      // For WASM contracts, we need to convert parameters to ScVal
      const orderedParams = ['owner'];
      const scValParams = orderedParams.map(paramName => {
        const value = params[paramName as keyof typeof params];
        if (value === undefined) {
          throw new Error(`Missing required parameter: ${paramName}`);
        }
        // Use appropriate conversion based on parameter type
        switch(paramName) {
          case 'owner':
            return addressToScVal(value as string);
          default:
            return nativeToScVal(value);
        }
      });
      const result = await functionToCall(...scValParams);
      txXdr = result.toXDR();

      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" },
        ]
      }
      
    } catch (error: any) {
      return {
        content: [{ 
          type: "text", 
          text: `Error executing list_contacts: ${error.message}${error.cause ? `\nCause: ${error.cause}` : ''}` 
        }]
      };
    }
  }
);


async function main() {
  const transport = new StdioServerTransport();
  await mcpServer.connect(transport);
  console.error("Soroban MCP Server running on stdio");
}

main().catch((error) => {
  console.error("Fatal error in main():", error);
  process.exit(1); 
});