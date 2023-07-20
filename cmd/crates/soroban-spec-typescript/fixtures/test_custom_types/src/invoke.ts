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
import { Options, ResponseTypes } from './method-options.js'

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

// defined this way so typeahead shows full union, not named alias
let someRpcResponse: Simulation | SendTx | GetTx
type SomeRpcResponse = typeof someRpcResponse

type InvokeArgs<R extends ResponseTypes, T = string> = Options<R> & {
  method: string,
  args?: any[],
  parseResultXdr?: (xdr: string) => T,
}

/**
 * Invoke a method on the test_custom_types contract.
 *
 * Uses Freighter to determine the current user and if necessary sign the transaction.
 *
 * @returns {T}, by default, the parsed XDR from either the simulation or the full transaction. If `simulateOnly` or `fullRpcResponse` are true, returns either the full simulation or the result of sending/getting the transaction to/from the ledger.
 */
export async function invoke<R extends ResponseTypes = undefined, T = string>(args: InvokeArgs<R, T>): Promise<R extends undefined ? T : R extends "simulated" ? Simulation : R extends "full" ? SomeRpcResponse : T>;
export async function invoke<R extends ResponseTypes, T = string>({
  method,
  args = [],
  fee = 100,
  responseType,
  parseResultXdr,
  secondsToWait = 10,
}: InvokeArgs<R, T>): Promise<T | string | SomeRpcResponse> {
  const freighterAccount = await getAccount()

  // use a placeholder null account if not yet connected to Freighter so that view calls can still work
  const account = freighterAccount ?? new SorobanClient.Account('GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF', '0')

  const contract = new SorobanClient.Contract(CONTRACT_ID)

  let tx = new SorobanClient.TransactionBuilder(account, {
    fee: fee.toString(10),
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(SorobanClient.TimeoutInfinite)
    .build()

  const simulated = await Server.simulateTransaction(tx)

  if (responseType === 'simulated') return simulated

  // is it possible for `auths` to be present but empty? Probably not, but let's be safe.
  const auths = simulated.results?.[0]?.auth
  let authsCount =  auths?.length ?? 0;

  const writeLength = SorobanClient.xdr.SorobanTransactionData.fromXDR(simulated.transactionData, 'base64').resources().footprint().readWrite().length

  const parse = parseResultXdr ?? (xdr => xdr)

  // if VIEW ˅˅˅˅
  if (authsCount === 0 && writeLength === 0) {
    if (responseType === 'full') return simulated

    const { results } = simulated
    if (!results || results[0] === undefined) {
      if (simulated.error) {
        throw new Error(simulated.error as unknown as string)
      }
      throw new Error(`Invalid response from simulateTransaction:\n{simulated}`)
    }
    return parse(results[0].xdr)
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

    if (!freighterAccount) {
      throw new Error('Not connected to Freighter')
    }

    tx = await signTx(
      SorobanClient.assembleTransaction(tx, NETWORK_PASSPHRASE, simulated) as Tx
    );
  }

  const raw = await sendTx(tx, secondsToWait);

  if (responseType === 'full') return raw

  // if `sendTx` awaited the inclusion of the tx in the ledger, it used
  // `getTransaction`, which has a `resultXdr` field
  if ('resultXdr' in raw) return parse(raw.resultXdr)

  // otherwise, it returned the result of `sendTransaction`
  if ('errorResultXdr' in raw) return parse(raw.errorResultXdr)

  // if neither of these are present, something went wrong
  console.error("Don't know how to parse result! Returning full RPC response.")
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
