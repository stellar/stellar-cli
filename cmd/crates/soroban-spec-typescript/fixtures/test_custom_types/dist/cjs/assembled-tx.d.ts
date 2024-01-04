import { Account, Address, Operation, SorobanRpc, xdr } from "stellar-sdk";
import type { Memo, MemoType, Transaction } from "stellar-sdk";
import type { ClassOptions, MethodOptions, Wallet, XDR_BASE64 } from "./method-options.js";
export type Tx = Transaction<Memo<MemoType>, Operation[]>;
export declare class ExpiredStateError extends Error {
}
export declare class NeedsMoreSignaturesError extends Error {
}
export declare class WalletDisconnectedError extends Error {
}
export declare class SendResultOnlyError extends Error {
}
export declare class SendFailedError extends Error {
}
export declare class NoUnsignedNonInvokerAuthEntriesError extends Error {
}
type SendTx = SorobanRpc.Api.SendTransactionResponse;
type GetTx = SorobanRpc.Api.GetTransactionResponse;
export type u32 = number;
export type i32 = number;
export type u64 = bigint;
export type i64 = bigint;
export type u128 = bigint;
export type i128 = bigint;
export type u256 = bigint;
export type i256 = bigint;
export type Option<T> = T | undefined;
export type Typepoint = bigint;
export type Duration = bigint;
export { Address };
export interface Error_ {
    message: string;
}
export interface Result<T, E extends Error_> {
    unwrap(): T;
    unwrapErr(): E;
    isOk(): boolean;
    isErr(): boolean;
}
export declare class Ok<T, E extends Error_ = Error_> implements Result<T, E> {
    readonly value: T;
    constructor(value: T);
    unwrapErr(): E;
    unwrap(): T;
    isOk(): boolean;
    isErr(): boolean;
}
export declare class Err<E extends Error_ = Error_> implements Result<any, E> {
    readonly error: E;
    constructor(error: E);
    unwrapErr(): E;
    unwrap(): never;
    isOk(): boolean;
    isErr(): boolean;
}
export declare const contractErrorPattern: RegExp;
type AssembledTransactionOptions<T = string> = MethodOptions & ClassOptions & {
    method: string;
    args?: any[];
    parseResultXdr: (xdr: string | xdr.ScVal | Err) => T;
};
export declare const NULL_ACCOUNT = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
export declare class AssembledTransaction<T> {
    options: AssembledTransactionOptions<T>;
    raw: Tx;
    private simulation?;
    private simulationResult?;
    private simulationTransactionData?;
    private server;
    toJSON(): string;
    static fromJSON<T>(options: Omit<AssembledTransactionOptions<T>, 'args'>, { tx, simulationResult, simulationTransactionData }: {
        tx: XDR_BASE64;
        simulationResult: {
            auth: XDR_BASE64[];
            retval: XDR_BASE64;
        };
        simulationTransactionData: XDR_BASE64;
    }): AssembledTransaction<T>;
    private constructor();
    static fromSimulation<T>(options: AssembledTransactionOptions<T>): Promise<AssembledTransaction<T>>;
    simulate: () => Promise<this>;
    get simulationData(): {
        result: SorobanRpc.Api.SimulateHostFunctionResult;
        transactionData: xdr.SorobanTransactionData;
    };
    get result(): T;
    parseError(errorMessage: string): Err | undefined;
    getWallet: () => Promise<Wallet>;
    getPublicKey: () => Promise<string | undefined>;
    /**
     * Get account details from the Soroban network for the publicKey currently
     * selected in user's wallet. If not connected to Freighter, use placeholder
     * null account.
     */
    getAccount: () => Promise<Account>;
    /**
     * Sign the transaction with the `wallet` (default Freighter), then send to
     * the network and return a `SentTransaction` that keeps track of all the
     * attempts to send and fetch the transaction from the network.
     */
    signAndSend: ({ secondsToWait, force }?: {
        /**
         * Wait `secondsToWait` seconds (default: 10) for both the transaction to SEND successfully (will keep trying if the server returns `TRY_AGAIN_LATER`), as well as for the transaction to COMPLETE (will keep checking if the server returns `PENDING`).
         */
        secondsToWait?: number | undefined;
        /**
         * If `true`, sign and send the transaction even if it is a read call.
         */
        force?: boolean | undefined;
    }) => Promise<SentTransaction<T>>;
    getStorageExpiration: () => Promise<number>;
    /**
     * Get a list of accounts, other than the invoker of the simulation, that
     * need to sign auth entries in this transaction.
     *
     * Soroban allows multiple people to sign a transaction. Someone needs to
     * sign the final transaction envelope; this person/account is called the
     * _invoker_, or _source_. Other accounts might need to sign individual auth
     * entries in the transaction, if they're not also the invoker.
     *
     * This function returns a list of accounts that need to sign auth entries,
     * assuming that the same invoker/source account will sign the final
     * transaction envelope as signed the initial simulation.
     *
     * One at a time, for each public key in this array, you will need to
     * serialize this transaction with `toJSON`, send to the owner of that key,
     * deserialize the transaction with `txFromJson`, and call
     * {@link signAuthEntries}. Then re-serialize and send to the next account
     * in this list.
     */
    needsNonInvokerSigningBy: ({ includeAlreadySigned, }?: {
        /**
         * Whether or not to include auth entries that have already been signed. Default: false
         */
        includeAlreadySigned?: boolean | undefined;
    }) => Promise<string[]>;
    preImageFor(entry: xdr.SorobanAuthorizationEntry, signatureExpirationLedger: number): xdr.HashIdPreimage;
    /**
     * If {@link needsNonInvokerSigningBy} returns a non-empty list, you can serialize
     * the transaction with `toJSON`, send it to the owner of one of the public keys
     * in the map, deserialize with `txFromJSON`, and call this method on their
     * machine. Internally, this will use `signAuthEntry` function from connected
     * `wallet` for each.
     *
     * Then, re-serialize the transaction and either send to the next
     * `needsNonInvokerSigningBy` owner, or send it back to the original account
     * who simulated the transaction so they can {@link sign} the transaction
     * envelope and {@link send} it to the network.
     *
     * Sending to all `needsNonInvokerSigningBy` owners in parallel is not currently
     * supported!
     */
    signAuthEntries: (expiration?: number | Promise<number>) => Promise<void>;
    get isReadCall(): boolean;
    hasRealInvoker: () => Promise<boolean>;
}
/**
 * A transaction that has been sent to the Soroban network. This happens in two steps:
 *
 * 1. `sendTransaction`: initial submission of the transaction to the network.
 *    This step can run into problems, and will be retried with exponential
 *    backoff if it does. See all attempts in `sendTransactionResponseAll` and the
 *    most recent attempt in `sendTransactionResponse`.
 * 2. `getTransaction`: once the transaction has been submitted to the network
 *    successfully, you need to wait for it to finalize to get the results of the
 *    transaction. This step can also run into problems, and will be retried with
 *    exponential backoff if it does. See all attempts in
 *    `getTransactionResponseAll` and the most recent attempt in
 *    `getTransactionResponse`.
 */
declare class SentTransaction<T> {
    options: AssembledTransactionOptions<T>;
    assembled: AssembledTransaction<T>;
    server: SorobanRpc.Server;
    signed: Tx;
    sendTransactionResponse?: SendTx;
    sendTransactionResponseAll?: SendTx[];
    getTransactionResponse?: GetTx;
    getTransactionResponseAll?: GetTx[];
    constructor(options: AssembledTransactionOptions<T>, assembled: AssembledTransaction<T>);
    static init: <T_1>(options: AssembledTransactionOptions<T_1>, assembled: AssembledTransaction<T_1>, secondsToWait?: number) => Promise<SentTransaction<T_1>>;
    private send;
    get result(): T;
}
