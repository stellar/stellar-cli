import { Keypair, TransactionBuilder } from "soroban-client";

export const rpcUrl = "http://localhost:8000/soroban/rpc";
export const secretKey =
  "SC36BWNUOCZAO7DMEJNNKFV6BOTPJP7IG5PSHLUOLT6DZFRU3D3XGIXW";

const keypair = Keypair.fromSecret(secretKey);
export const publicKey = keypair.publicKey();
const networkPassphrase = "Standalone Network ; February 2017";

export const wallet = {
  isConnected: () => Promise.resolve(true),
  isAllowed: () => Promise.resolve(true),
  getUserInfo: () => Promise.resolve({ publicKey }),
  signTransaction: async (
    tx: string,
    _opts?: {
      network?: string;
      networkPassphrase?: string;
      accountToSign?: string;
    }
  ) => {
    const t = TransactionBuilder.fromXDR(tx, networkPassphrase);
    t.sign(keypair);
    return t.toXDR();
  },
};
