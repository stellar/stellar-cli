import * as SorobanClient from "soroban-client";
/**
 * Get account details from the Soroban network for the publicKey currently
 * selected in Freighter. If not connected to Freighter, return null.
 */
async function getAccount(wallet, server) {
    if (!(await wallet.isConnected()) || !(await wallet.isAllowed())) {
        return null;
    }
    const { publicKey } = await wallet.getUserInfo();
    if (!publicKey) {
        return null;
    }
    return await server.getAccount(publicKey);
}
export class NotImplementedError extends Error {
}
// defined this way so typeahead shows full union, not named alias
let someRpcResponse;
export async function invoke({ method, args = [], fee = 100, responseType, parseResultXdr, secondsToWait = 10, rpcUrl, networkPassphrase, contractId, wallet, }) {
    wallet = wallet ?? (await import("@stellar/freighter-api")).default;
    let parse = parseResultXdr;
    const server = new SorobanClient.Server(rpcUrl, {
        allowHttp: rpcUrl.startsWith("http://"),
    });
    const walletAccount = await getAccount(wallet, server);
    // use a placeholder null account if not yet connected to Freighter so that view calls can still work
    const account = walletAccount ??
        new SorobanClient.Account("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF", "0");
    const contract = new SorobanClient.Contract(contractId);
    let tx = new SorobanClient.TransactionBuilder(account, {
        fee: fee.toString(10),
        networkPassphrase,
    })
        .addOperation(contract.call(method, ...args))
        .setTimeout(SorobanClient.TimeoutInfinite)
        .build();
    const simulated = await server.simulateTransaction(tx);
    if (simulated.error)
        throw simulated.error;
    if (responseType === "simulated")
        return simulated;
    // is it possible for `auths` to be present but empty? Probably not, but let's be safe.
    let authsCount = simulated.result.auth?.length ?? 0;
    const writeLength = simulated.transactionData
        .build()
        .resources()
        .footprint()
        .readWrite().length;
    const isViewCall = authsCount === 0 && writeLength === 0;
    if (isViewCall) {
        if (responseType === "full")
            return simulated;
        const retval = simulated.result?.retval;
        if (!retval) {
            if (simulated.error) {
                throw new Error(simulated.error);
            }
            throw new Error(`Invalid response from simulateTransaction:\n{simulated}`);
        }
        return parseResultXdr(retval);
    }
    if (authsCount > 1) {
        throw new NotImplementedError("Multiple auths not yet supported");
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
    if (!walletAccount) {
        throw new Error("Not connected to Freighter");
    }
    tx = await signTx(wallet, SorobanClient.assembleTransaction(tx, networkPassphrase, simulated).build(), networkPassphrase);
    const raw = await sendTx(tx, secondsToWait, server);
    if (responseType === "full")
        return raw;
    // if `sendTx` awaited the inclusion of the tx in the ledger, it used
    // `getTransaction`, which has a `returnValue` field
    if ("returnValue" in raw)
        return parse(raw.returnValue);
    // otherwise, it returned the result of `sendTransaction`
    if ("errorResultXdr" in raw)
        return parse(raw.errorResultXdr);
    // if neither of these are present, something went wrong
    console.error("Don't know how to parse result! Returning full RPC response.");
    return raw;
}
/**
 * Sign a transaction with Freighter and return the fully-reconstructed
 * transaction ready to send with {@link sendTx}.
 *
 * If you need to construct a transaction yourself rather than using `invoke`
 * or one of the exported contract methods, you may want to use this function
 * to sign the transaction with Freighter.
 */
export async function signTx(wallet, tx, networkPassphrase) {
    const signed = await wallet.signTransaction(tx.toXDR(), {
        networkPassphrase,
    });
    return SorobanClient.TransactionBuilder.fromXDR(signed, networkPassphrase);
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
export async function sendTx(tx, secondsToWait, server) {
    const sendTransactionResponse = await server.sendTransaction(tx);
    if (sendTransactionResponse.status !== "PENDING" || secondsToWait === 0) {
        return sendTransactionResponse;
    }
    let getTransactionResponse = await server.getTransaction(sendTransactionResponse.hash);
    const waitUntil = new Date(Date.now() + secondsToWait * 1000).valueOf();
    let waitTime = 1000;
    let exponentialFactor = 1.5;
    while (Date.now() < waitUntil &&
        getTransactionResponse.status === "NOT_FOUND") {
        // Wait a beat
        await new Promise((resolve) => setTimeout(resolve, waitTime));
        /// Exponential backoff
        waitTime = waitTime * exponentialFactor;
        // See if the transaction is complete
        getTransactionResponse = await server.getTransaction(sendTransactionResponse.hash);
    }
    if (getTransactionResponse.status === "NOT_FOUND") {
        console.error(`Waited ${secondsToWait} seconds for transaction to complete, but it did not. Returning anyway. Check the transaction status manually. Info: ${JSON.stringify(sendTransactionResponse, null, 2)}`);
    }
    return getTransactionResponse;
}
