declare let responseTypes: 'simulated' | 'full' | undefined;
export type ResponseTypes = typeof responseTypes;
export type XDR_BASE64 = string;
export interface Wallet {
    isConnected: () => Promise<boolean>;
    isAllowed: () => Promise<boolean>;
    getUserInfo: () => Promise<{
        publicKey?: string;
    }>;
    signTransaction: (tx: XDR_BASE64, opts?: {
        network?: string;
        networkPassphrase?: string;
        accountToSign?: string;
    }) => Promise<XDR_BASE64>;
    signAuthEntry: (entryXdr: XDR_BASE64, opts?: {
        accountToSign?: string;
    }) => Promise<XDR_BASE64>;
}
export type ClassOptions = {
    contractId: string;
    networkPassphrase: string;
    rpcUrl: string;
    errorTypes?: Record<number, {
        message: string;
    }>;
    /**
     * A Wallet interface, such as Freighter, that has the methods `isConnected`, `isAllowed`, `getUserInfo`, and `signTransaction`. If not provided, will attempt to import and use Freighter. Example:
     *
     * @example
     * ```ts
     * import freighter from "@stellar/freighter-api";
     * import { Contract } from "test_custom_types";
     * const contract = new Contract({
     *   …,
     *   wallet: freighter,
     * })
     * ```
     */
    wallet?: Wallet;
};
export type MethodOptions = {
    /**
     * The fee to pay for the transaction. Default: soroban-sdk's BASE_FEE ('100')
     */
    fee?: number;
};
export {};
