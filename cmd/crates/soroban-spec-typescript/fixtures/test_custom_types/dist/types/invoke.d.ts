import * as SorobanClient from 'soroban-client';
import type { Memo, MemoType, Operation, Transaction } from 'soroban-client';
import { Options, ResponseTypes } from './method-options.js';
export type Tx = Transaction<Memo<MemoType>, Operation[]>;
export declare class NotImplementedError extends Error {
}
type Simulation = SorobanClient.SorobanRpc.SimulateTransactionResponse;
type SendTx = SorobanClient.SorobanRpc.SendTransactionResponse;
type GetTx = SorobanClient.SorobanRpc.GetTransactionResponse;
declare let someRpcResponse: Simulation | SendTx | GetTx;
type SomeRpcResponse = typeof someRpcResponse;
type InvokeArgs<R extends ResponseTypes, T = string> = Options<R> & {
    method: string;
    args?: any[];
    parseResultXdr?: (xdr: string) => T;
};
/**
 * Invoke a method on the test_custom_types contract.
 *
 * Uses Freighter to determine the current user and if necessary sign the transaction.
 *
 * @returns {T}, by default, the parsed XDR from either the simulation or the full transaction. If `simulateOnly` or `fullRpcResponse` are true, returns either the full simulation or the result of sending/getting the transaction to/from the ledger.
 */
export declare function invoke<R extends ResponseTypes = undefined, T = string>(args: InvokeArgs<R, T>): Promise<R extends undefined ? T : R extends "simulated" ? Simulation : R extends "full" ? SomeRpcResponse : T>;
/**
 * Sign a transaction with Freighter and return the fully-reconstructed
 * transaction ready to send with {@link sendTx}.
 *
 * If you need to construct a transaction yourself rather than using `invoke`
 * or one of the exported contract methods, you may want to use this function
 * to sign the transaction with Freighter.
 */
export declare function signTx(tx: Tx): Promise<Tx>;
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
export declare function sendTx(tx: Tx, secondsToWait: number): Promise<SendTx | GetTx>;
export {};
