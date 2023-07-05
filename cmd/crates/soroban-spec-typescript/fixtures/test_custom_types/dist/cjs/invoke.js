"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.sendTx = exports.signTx = exports.invoke = exports.NotImplementedError = void 0;
const freighter_api_1 = require("@stellar/freighter-api");
// working around ESM compatibility issues
const { isConnected, isAllowed, getUserInfo, signTransaction, } = freighter_api_1.default;
const SorobanClient = require("soroban-client");
const constants_js_1 = require("./constants.js");
const server_js_1 = require("./server.js");
/**
 * Get account details from the Soroban network for the publicKey currently
 * selected in Freighter. If not connected to Freighter, return null.
 */
async function getAccount() {
    if (!await isConnected() || !await isAllowed()) {
        return null;
    }
    const { publicKey } = await getUserInfo();
    if (!publicKey) {
        return null;
    }
    return await server_js_1.Server.getAccount(publicKey);
}
class NotImplementedError extends Error {
}
exports.NotImplementedError = NotImplementedError;
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
async function invoke({ method, args = [], fee = 100, signAndSend = false }) {
    const freighterAccount = await getAccount();
    // use a placeholder account if not yet connected to Freighter so that view calls can still work
    const account = freighterAccount ?? new SorobanClient.Account('GBZXP4PWQLOTBL3P6OE6DQ7QXNYDAZMWQG27V7ATM7P3TKSRDLQS4V7Q', '0');
    const contract = new SorobanClient.Contract(constants_js_1.CONTRACT_ID);
    let tx = new SorobanClient.TransactionBuilder(account, {
        fee: fee.toString(10),
        networkPassphrase: constants_js_1.NETWORK_PASSPHRASE,
    })
        .addOperation(contract.call(method, ...args))
        .setTimeout(SorobanClient.TimeoutInfinite)
        .build();
    const simulated = await server_js_1.Server.simulateTransaction(tx);
    if (!signAndSend) {
        const { results } = simulated;
        if (!results || results[0] === undefined) {
            if (simulated.error) {
                throw new Error(simulated.error);
            }
            throw new Error(`Invalid response from simulateTransaction:\n{simulated}`);
        }
        return results[0];
    }
    if (!freighterAccount) {
        throw new Error('Not connected to Freighter');
    }
    // is it possible for `auths` to be present but empty? Probably not, but let's be safe.
    const auths = simulated.results?.[0]?.auth;
    let auth_len = auths?.length ?? 0;
    if (auth_len > 1) {
        throw new NotImplementedError("Multiple auths not yet supported");
    }
    else if (auth_len == 1) {
        // TODO: figure out how to fix with new SorobanClient
        // const auth = SorobanClient.xdr.SorobanAuthorizationEntry.fromXDR(auths![0]!, 'base64')
        // if (auth.addressWithNonce() !== undefined) {
        //   throw new NotImplementedError(
        //     `This transaction needs to be signed by ${auth.addressWithNonce()
        //     }; Not yet supported`
        //   )
        // }
    }
    tx = await signTx(SorobanClient.assembleTransaction(tx, constants_js_1.NETWORK_PASSPHRASE, simulated));
    const raw = await sendTx(tx);
    return {
        ...raw,
        xdr: raw.resultXdr,
    };
}
exports.invoke = invoke;
/**
 * Sign a transaction with Freighter and return the fully-reconstructed
 * transaction ready to send with {@link sendTx}.
 *
 * If you need to construct a transaction yourself rather than using `invoke`
 * or one of the exported contract methods, you may want to use this function
 * to sign the transaction with Freighter.
 */
async function signTx(tx) {
    const signed = await signTransaction(tx.toXDR(), {
        networkPassphrase: constants_js_1.NETWORK_PASSPHRASE,
    });
    return SorobanClient.TransactionBuilder.fromXDR(signed, constants_js_1.NETWORK_PASSPHRASE);
}
exports.signTx = signTx;
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
async function sendTx(tx, secondsToWait = 10) {
    const sendTransactionResponse = await server_js_1.Server.sendTransaction(tx);
    let getTransactionResponse = await server_js_1.Server.getTransaction(sendTransactionResponse.hash);
    const waitUntil = new Date((Date.now() + secondsToWait * 1000)).valueOf();
    let waitTime = 1000;
    let exponentialFactor = 1.5;
    while ((Date.now() < waitUntil) && getTransactionResponse.status === "NOT_FOUND") {
        // Wait a beat
        await new Promise(resolve => setTimeout(resolve, waitTime));
        /// Exponential backoff
        waitTime = waitTime * exponentialFactor;
        // See if the transaction is complete
        getTransactionResponse = await server_js_1.Server.getTransaction(sendTransactionResponse.hash);
    }
    if (getTransactionResponse.status === "NOT_FOUND") {
        console.log(`Waited ${secondsToWait} seconds for transaction to complete, but it did not. Returning anyway. Check the transaction status manually. Info: ${JSON.stringify(sendTransactionResponse, null, 2)}`);
    }
    return getTransactionResponse;
}
exports.sendTx = sendTx;
