import { spawnSync } from "node:child_process";
import { Keypair, TransactionBuilder, hash } from "@stellar/stellar-sdk";
import { Address } from 'test-custom-types'

const rootKeypair = Keypair.fromSecret(spawnSync("./soroban", ["keys", "show", "root"], { shell: true, encoding: "utf8" }).stdout.trim());
const aliceKeypair = Keypair.fromSecret(spawnSync("./soroban", ["keys", "show", "alice"], { shell: true, encoding: "utf8" }).stdout.trim());
const bobKeypair = Keypair.fromSecret(spawnSync("./soroban", ["keys", "show", "bob"], { shell: true, encoding: "utf8" }).stdout.trim());

export const root = {
  keypair: rootKeypair,
  address: Address.fromString(rootKeypair.publicKey()),
}

export const alice = {
  keypair: aliceKeypair,
  address: Address.fromString(aliceKeypair.publicKey()),
}

export const bob = {
  keypair: bobKeypair,
  address: Address.fromString(bobKeypair.publicKey()),
}

function getKeypair(pk: string): Keypair {
  return Keypair.fromSecret({
    [root.keypair.publicKey()]: root.keypair.secret(),
    [alice.keypair.publicKey()]: alice.keypair.secret(),
    [bob.keypair.publicKey()]: bob.keypair.secret(),
  }[pk])
}

export const rpcUrl = process.env.SOROBAN_RPC_URL ?? "http://localhost:8000/";
export const networkPassphrase = process.env.SOROBAN_NETWORK_PASSPHRASE ?? "Standalone Network ; February 2017";

export class Wallet {
  constructor(private publicKey: string) {}
  isConnected = () => Promise.resolve(true)
  isAllowed = () => Promise.resolve(true)
  getUserInfo = () => Promise.resolve({ publicKey: this.publicKey })
  signTransaction = async (tx: string) => {
    const t = TransactionBuilder.fromXDR(tx, networkPassphrase);
    t.sign(getKeypair(this.publicKey));
    return t.toXDR();
  }
  signAuthEntry = async (
    entryXdr: string,
    opts?: {
      accountToSign?: string,
    }
  ): Promise<string> => {
    return getKeypair(opts?.accountToSign ?? this.publicKey)
      .sign(hash(Buffer.from(entryXdr, "base64")))
      .toString('base64')
  }
}

export const wallet = new Wallet(root.keypair.publicKey())
