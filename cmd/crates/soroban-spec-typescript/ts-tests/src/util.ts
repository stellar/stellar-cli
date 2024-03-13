import { spawnSync } from "node:child_process";
import { Address, Keypair } from "@stellar/stellar-sdk";
import { basicNodeSigner } from "@stellar/stellar-sdk/lib/contract_client/index.js";

const rootKeypair = Keypair.fromSecret(spawnSync("./soroban", ["keys", "show", "root"], { shell: true, encoding: "utf8" }).stdout.trim());

export const root = {
  keypair: rootKeypair,
  address: Address.fromString(rootKeypair.publicKey()),
}

export const rpcUrl = process.env.SOROBAN_RPC_URL ?? "http://localhost:8000/";
export const networkPassphrase = process.env.SOROBAN_NETWORK_PASSPHRASE ?? "Standalone Network ; February 2017";

export const signer = basicNodeSigner(root.keypair, networkPassphrase);
