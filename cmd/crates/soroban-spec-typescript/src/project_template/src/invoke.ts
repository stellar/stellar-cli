import freighter from "@stellar/freighter-api";
// working around ESM compatibility issues
const {
  isConnected,
  isAllowed,
  getUserInfo,
  signTransaction,
} = freighter;
import * as SorobanClient from 'soroban-client'
import type { Account, Memo, MemoType, Operation, Transaction } from 'soroban-client';
import { NETWORK_PASSPHRASE, CONTRACT_ID } from './constants.js'
import { Server } from './server.js'

export type Tx = Transaction<Memo<MemoType>, Operation[]>

/**
 * Get account details from the Soroban network for the publicKey currently
 * selected in Freighter. If not connected to Freighter, return null.
 */
async function getAccount(): Promise<Account | null> {
  if (!await isConnected() || !await isAllowed()) {
    return null
  }
  const { publicKey } = await getUserInfo()
  if (!publicKey) {
    return null
  }
  return await Server.getAccount(publicKey)
}

export class NotImplementedError extends Error { }

type Simulation = SorobanClient.SorobanRpc.SimulateTransactionResponse
type SendTx = SorobanClient.SorobanRpc.SendTransactionResponse
type GetTx = SorobanClient.SorobanRpc.GetTransactionResponse

export type InvokeArgs<T = any> = {
  method: string
  args?: any[]
  fee?: number
  simulateOnly?: boolean
  fullRpcResponse?: boolean
  secondsToWait?: number
  parseResultXdr?: (xdr: string) => T
}

/**
 * Invoke a method on the INSERT_CONTRACT_NAME_HERE contract.
 *
 * Uses Freighter to determine the current user and if necessary sign the transaction.
 *
 * @param {string} obj.method - The method to invoke.
 * @param {any[]} obj.args - The arguments to pass to the method.
 * @param {number} obj.fee - The fee to pay for the transaction.
 * @param {boolean} obj.simulateOnly – All invocations start with a simulation/preflight. If the simulation shows that the transaction requires auth/signing, then by default `invoke` will try to have the user sign the transaction with Freighter and send the signed transaction to the network. To prevent this signature step and inspect the results of the preflight, you can set `simulateOnly: true`. This implies `fullRpcResponse: true`, since it's assumed that you want to inspect the preflight data for a change method. That is, setting `simulateOnly: true` for a view method is not useful, since view methods do not require auth/signing and will return the simulation right away regardless.
 * @param {boolean} obj.fullRpcResponse – Whether to return the full RPC response. If false, will parse the returned XDR with `parseResultXdr` and return it.
 * @param {number} obj.secondsToWait – If the simulation shows that this invocation requires auth/signing, `invoke` will wait `secondsToWait` seconds for the transaction to complete before giving up and returning the incomplete {@link SorobanClient.SorobanRpc.GetTransactionResponse} results (or attempting to parse their probably-missing XDR with `parseResultXdr`, depending on `fullRpcResponse`). Set this to `0` to skip waiting altogether, which will return you {@link SorobanClient.SorobanRpc.SendTransactionResponse} more quickly, before the transaction has time to be included in the ledger.
 * @param {function} obj.parseResultXdr – If `fullRpcResponse` and `simulateOnly` are both `false` (the default), this function will be used to parse the XDR returned by either the simulation or the sent transaction. If not provided, the raw XDR will be returned; this can be inspected manually at https://laboratory.stellar.org/#xdr-viewer?network=futurenet
 * @returns T, by default, the parsed XDR from either the simulation or the full transaction. If `simulateOnly` or `fullRpcResponse` are true, returns either the full simulation or the result of sending/getting the transaction to/from the ledger.
 */
export async function invoke<T = any>(args: InvokeArgs & { fullRpcResponse?: undefined | false, simulateOnly?: undefined | false }): Promise<T>;
export async function invoke<T = any>(args: InvokeArgs & { simulateOnly: true }): Promise<Simulation>;
export async function invoke<T = any>(args: InvokeArgs & { fullRpcResponse: true }): Promise<Simulation | SendTx | GetTx>;
export async function invoke<T = any>({
  method,
  args = [],
  fee = 100,
  fullRpcResponse = false,
  simulateOnly = false,
  parseResultXdr = xdr => xdr,
  secondsToWait = 10,
}: InvokeArgs): Promise<T | Simulation | SendTx | GetTx> {
  const freighterAccount = await getAccount()

  // use a placeholder account if not yet connected to Freighter so that view calls can still work
  const account = freighterAccount ?? new SorobanClient.Account('GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', '0')

  const contract = new SorobanClient.Contract(CONTRACT_ID)

  let tx = new SorobanClient.TransactionBuilder(account, {
    fee: fee.toString(10),
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(SorobanClient.TimeoutInfinite)
    .build()

  const simulated = await Server.simulateTransaction(tx)

  if (simulateOnly) return simulated

  // is it possible for `auths` to be present but empty? Probably not, but let's be safe.
  const auths = simulated.results?.[0]?.auth
  let authsCount =  auths?.length ?? 0;

  // if VIEW ˅˅˅˅
  if (authsCount === 0) {
    if (fullRpcResponse) return simulated

    const { results } = simulated
    if (!results || results[0] === undefined) {
      if (simulated.error) {
        throw new Error(simulated.error as unknown as string)
      }
      throw new Error(`Invalid response from simulateTransaction:\n{simulated}`)
    }
    return parseResultXdr(results[0].xdr)
  }

  // ^^^^ else, is CHANGE method ˅˅˅˅
  if (authsCount > 1) {
    throw new NotImplementedError("Multiple auths not yet supported")
  }
  if (authsCount === 1) {
    // TODO: figure out how to fix with new SorobanClient
    // const auth = SorobanClient.xdr.SorobanAuthorizationEntry.fromXDR(auths![0]!, 'base64')
    // if (auth.addressWithNonce() !== undefined) {
    //   throw new NotImplementedError(
    //     `This transaction needs to be signed by ${auth.addressWithNonce()
    //     }; Not yet supported`
    //   )
    // }
  }
  if (!freighterAccount) {
    throw new Error('Not connected to Freighter')
  }

  tx = await signTx(
    SorobanClient.assembleTransaction(tx, NETWORK_PASSPHRASE, simulated) as Tx
  );

  const raw = await sendTx(tx, secondsToWait);

  if (fullRpcResponse) return raw

  // if `sendTx` awaited the inclusion of the tx in the ledger, it used
  // `getTransaction`, which has a `resultXdr` field
  if ('resultXdr' in raw) return parseResultXdr(raw.resultXdr)

  // otherwise, it returned the result of `sendTransaction`
  if ('errorResultXdr' in raw) return parseResultXdr(raw.errorResultXdr)

  // if neither of these are present, something went wrong
  console.log("Don't know how to parse result! Returning fullRpcResponse")
  return raw
}

/**
 * Sign a transaction with Freighter and return the fully-reconstructed
 * transaction ready to send with {@link sendTx}.
 *
 * If you need to construct a transaction yourself rather than using `invoke`
 * or one of the exported contract methods, you may want to use this function
 * to sign the transaction with Freighter.
 */
export async function signTx(tx: Tx): Promise<Tx> {
  const signed = await signTransaction(tx.toXDR(), {
    networkPassphrase: NETWORK_PASSPHRASE,
  })

  return SorobanClient.TransactionBuilder.fromXDR(
    signed,
    NETWORK_PASSPHRASE
  ) as Tx
}

/**
 * Send a transaction to the Soroban network.
 *
 * Wait `secondsToWait` seconds for the transaction to complete (default: 10).
 *
 * If you need to construct or sign a transaction yourself rather than using
 * `invoke` or one of the exported contract methods, you may want to use this
 * function for its timeout/`secondsToWait` logic, rather than implementing
 * your own.
 */
export async function sendTx(tx: Tx, secondsToWait: number): Promise<SendTx | GetTx> {
  const sendTransactionResponse = await Server.sendTransaction(tx);

  if (sendTransactionResponse.status !== "PENDING" || secondsToWait === 0) {
    return sendTransactionResponse;
  }

  let getTransactionResponse = await Server.getTransaction(sendTransactionResponse.hash);

  const waitUntil = new Date((Date.now() + secondsToWait * 1000)).valueOf()

  let waitTime = 1000;
  let exponentialFactor = 1.5

  while ((Date.now() < waitUntil) && getTransactionResponse.status === "NOT_FOUND") {
    // Wait a beat
    await new Promise(resolve => setTimeout(resolve, waitTime))
    /// Exponential backoff
    waitTime = waitTime * exponentialFactor;
    // See if the transaction is complete
    getTransactionResponse = await Server.getTransaction(sendTransactionResponse.hash)
  }

  if (getTransactionResponse.status === "NOT_FOUND") {
    console.log(
      `Waited ${secondsToWait} seconds for transaction to complete, but it did not. Returning anyway. Check the transaction status manually. Info: ${JSON.stringify(sendTransactionResponse, null, 2)}`
    )
  }

  return getTransactionResponse
}
