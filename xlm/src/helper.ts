import { Address, nativeToScVal, xdr, rpc as SorobanRpc, TransactionBuilder, contract } from '@stellar/stellar-sdk';
import { Client as SacClient } from 'sac-sdk'

export const createContractClient = async (contractId: string, networkPassphrase: string, rpcUrl: string): Promise<contract.Client> => {
  return contract.Client.from({
    contractId,
    networkPassphrase,
    rpcUrl
  })
}

export const createSACClient = async (contractId: string, networkPassphrase: string, rpcUrl: string): Promise<SacClient> => {
  return new SacClient({
    contractId,
    rpcUrl,
    networkPassphrase,
  })
}

/**
 * Convert a string to a Symbol ScVal
 */
export const stringToSymbol = (val: string) => {
  return nativeToScVal(val, { type: "symbol" });
};

/**
 * Convert a number to a u64 ScVal with 2 decimal precision
 */
export const numberToU64 = (val: number) => {
  const num = parseInt((val * 100).toFixed(0));
  return nativeToScVal(num, { type: "u64" });
};

/**
 * Convert a number to an i128 ScVal with 2 decimal precision
 */
export const numberToI128 = (val: number) => {
  const num = parseInt((val * 100).toFixed(0));
  return nativeToScVal(num, { type: "i128" });
};

/**
 * Convert a Stellar address to ScVal
 */
export function addressToScVal(addressStr: string) {
  // Validate and convert the address
  const address = Address.fromString(addressStr);
  // Convert to ScVal
  return nativeToScVal(address);
}

/**
 * Convert a string to an i128 ScVal
 * This is useful for handling large numbers that exceed JavaScript's number precision
 */
export function i128ToScVal(value: string) {
  return nativeToScVal(value, { type: "i128" });
}

/**
 * Convert a string to a u128 ScVal
 * This is useful for handling large numbers that exceed JavaScript's number precision
 */
export function u128ToScVal(value: string) {
  return nativeToScVal(value, { type: "u128" });
}

/**
 * Convert a boolean to an ScVal
 */
export function boolToScVal(value: boolean) {
  return xdr.ScVal.scvBool(value);
}

/**
 * Convert a number to a u32 ScVal
 */
export function u32ToScVal(value: number) {
  return xdr.ScVal.scvU32(value);
}

/**
 * Configuration for transaction submission
 */
export interface SubmitTransactionConfig {
  server: SorobanRpc.Server;
  networkPassphrase: string;
  maxRetries?: number;
  pollingIntervalMs?: number;
}

/**
 * Submit a transaction and wait for its completion
 */
export async function submitTransaction(
  signedXdr: string,
  config: SubmitTransactionConfig
): Promise<{
  status: 'SUCCESS' | 'FAILED' | 'TIMEOUT',
  hash?: string,
  resultXdr?: string,
  resultMetaXdr?: string,
  response?: any
}> {
  const {
    server,
    networkPassphrase,
    maxRetries = 10,
    pollingIntervalMs = 2000
  } = config;

  try {
    // Reconstruct and submit the transaction
    const signedTx = TransactionBuilder.fromXDR(signedXdr, networkPassphrase);
    const submittedTx = await server.sendTransaction(signedTx);
    console.log('Transaction submitted:', submittedTx.hash);

    // Wait for transaction completion
    let retries = 0;
    
    while (retries < maxRetries) {
      try {
        const tx = await server.getTransaction(submittedTx.hash);
        
        if (tx.status === "SUCCESS") {
          return {
            status: 'SUCCESS',
            hash: submittedTx.hash,
            resultXdr: typeof tx.resultXdr === 'string' ? tx.resultXdr : undefined,
            resultMetaXdr: typeof tx.resultMetaXdr === 'string' ? tx.resultMetaXdr : undefined,
            response: tx
          };
        } else if (tx.status === "FAILED") {
          return {
            status: 'FAILED',
            hash: submittedTx.hash,
            resultXdr: typeof tx.resultXdr === 'string' ? tx.resultXdr : undefined,
            resultMetaXdr: typeof tx.resultMetaXdr === 'string' ? tx.resultMetaXdr : undefined,
            response: tx
          };
        }
        
        await new Promise(resolve => setTimeout(resolve, pollingIntervalMs));
        retries++;
        
      } catch (error: any) {
        console.error('Error checking transaction status:', error.message);
        
        // Check for XDR parsing error (Bad union switch)
        if (error.message.includes('Bad union switch')) {
          // Even though we can't parse the response, the transaction might still be processing
          // We should return a TIMEOUT status to avoid blocking indefinitely
          await new Promise(resolve => setTimeout(resolve, pollingIntervalMs));
          retries++;
          
          // After some retries, just return the transaction hash for manual checking
          if (retries >= maxRetries / 2) {
            return {
              status: 'TIMEOUT',
              hash: submittedTx.hash,
              response: { message: "Transaction submitted but status check failed due to XDR parsing error" }
            };
          }
          continue;
        }
        
        if (error.message.includes("404") || error.message.includes("NOT_FOUND")) {
          await new Promise(resolve => setTimeout(resolve, pollingIntervalMs));
          retries++;
          continue;
        }
        throw error;
      }
    }
    
    return {
      status: 'TIMEOUT',
      hash: submittedTx.hash
    };
    
  } catch (error: any) {
    console.error('Submit transaction error:', error);
    throw new Error(`Transaction submission failed: ${error.message}`);
  }
}
