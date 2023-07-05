import * as SorobanClient from 'soroban-client';
import type { Memo, MemoType, Operation, Transaction } from 'soroban-client';
export type Tx = Transaction<Memo<MemoType>, Operation[]>;
export type Simulation = NonNullable<SorobanClient.SorobanRpc.SimulateTransactionResponse['results']>[0];
export type TxResponse = SorobanClient.SorobanRpc.GetTransactionResponse;
export type InvokeArgs = {
    method: string;
    args?: any[];
    signAndSend?: boolean;
    fee?: number;
};
export declare class NotImplementedError extends Error {
}
/**
 * Invoke a method on the test_custom_types contract.
 *
 * Uses Freighter to determine the current user and if necessary sign the transaction.
 *
 * @param {string} obj.method - The method to invoke.
 * @param {any[]} obj.args - The arguments to pass to the method.
 * @param {boolean} obj.signAndSend - Whether to sign and send the transaction, or just simulate it. Unless the method requires authentication.
 * @param {number} obj.fee - The fee to pay for the transaction.
 * @returns The transaction response, or the simulation result if signing isn't required.
 */
export declare function invoke({ method, args, fee, signAndSend }: InvokeArgs): Promise<(TxResponse & {
    xdr: string;
}) | Simulation>;
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
export declare function sendTx(tx: Tx, secondsToWait?: number): Promise<TxResponse>;
