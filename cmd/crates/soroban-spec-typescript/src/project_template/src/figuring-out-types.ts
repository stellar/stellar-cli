let responseTypes: 'parsed' | 'simulated' | 'full'
type ResponseTypes = typeof responseTypes

type Options<R extends ResponseTypes> = {
  fee?: number
  responseType?: R
  secondsToWait?: number
}

type InvokeArgs<R extends ResponseTypes, T = string> = Options<R> & {
  method: string,
  args?: any[],
  parseResultXdr?: (xdr: string) => T,
};

function invoke<R extends ResponseTypes, T = string>(args: InvokeArgs<R, T>): R extends "simulated" ? "sim" : R extends "full" ? "rpc" : T;
function invoke<undefined, T>(args: InvokeArgs<'parsed', T>): T;
function invoke<R extends ResponseTypes, T = string>(args: InvokeArgs<R, T>): T | string | "sim" | "rpc" {
  // stub implementation with correct return types
  switch(args.responseType) {
    case 'simulated':
      return 'sim';
    case 'full':
      return 'rpc';
    case 'parsed':
    case undefined:
    default:
      const parse = args.parseResultXdr ?? (xdr => xdr)
      return parse(JSON.stringify({ method: args.method, args: args.args }));
  }
}

const invokeSimulate = invoke({ method: 'balance', args: ['GAAAA'], responseType: 'simulated' }) // typeof invokeSimulate === "sim"
const invokeRpc = invoke({ method: 'balance', args: ['GAAAA'], responseType: 'full' }) // typeof invokeRpc === "rpc"
const invokeParsed = invoke({ method: 'balance', args: ['GAAAA'], responseType: 'parsed' }) // typeof invokeParsed === string
const invokeSimple = invoke({ method: 'balance', args: ['GAAAA'] }) // typeof invokeSimple === string
const invokeParsedXdr = invoke({ method: 'balance', args: ['GAAAA'], parseResultXdr: () => 1 }) // typeof invokeParsed === number | "sim" | "rpc"

/**
 * Smart contract author comments go here.
 * 
 * Available `options`:
 * - fee: The fee to pay for the transaction. Default: 100.
 * - secondsToWait: If the simulation shows that this invocation requires auth/signing, `invoke` will wait `secondsToWait` seconds for the transaction to complete before giving up and returning the incomplete {@link SorobanClient.SorobanRpc.GetTransactionResponse} results (or attempting to parse their probably-missing XDR with `parseResultXdr`, depending on `responseType`). Set this to `0` to skip waiting altogether, which will return you {@link SorobanClient.SorobanRpc.SendTransactionResponse} more quickly, before the transaction has time to be included in the ledger. Default: 10.
 * - parseResultXdr: If `responseType` is `parsed`, this function will be used to parse the XDR returned by either the simulation or the sent transaction. If not provided, the raw XDR will be returned; this can be inspected manually at https://laboratory.stellar.org/#xdr-viewer?network=futurenet
 * - responseType: What type of response to return. 
 *   - `'parsed'` parses the returned XDR as `{{i128}}`. This is the default. Runs preflight, checks to see if auth/signing is required, and sends the transaction if so. If there's no error and `secondsToWait` is positive, awaits the finalized transaction.
 *   - `'simulated'` will only simulate/preflight the transaction, even if it's a change/set method that requires auth/signing. Returns full preflight info. 
 *   - `'full'` return the full RPC response, meaning either 1. the preflight info, if it's a view/read method that doesn't require auth/signing, or 2. the `sendTransaction` response, if there's a problem with sending the transaction or if you set `secondsToWait` to 0, or 3. the `getTransaction` response, if it's a change method with no `sendTransaction` errors and a positive `secondsToWait`.
 * 
 * @returns `{{i128}}`, by default, the parsed XDR from either the simulation or the full transaction. For `responseType` of `'simulated'`, returns the full preflight info. For `responseType` of `'full'`, returns either the simulation or the result of sending/getting the transaction to/from the ledger.
 */
function balance<R extends ResponseTypes>({ address }: { address: string }, options: Options<R> = {}) {
  return invoke({
    method: 'balance',
    args: [address],
    ...options,
    parseResultXdr: () => (parseInt(address, 62)),
  })
}

const balanceSimulate = balance({ address: "GAAAA" }, { responseType: 'simulated' })
const balanceRpc = balance({ address: "GAAAA" }, { responseType: 'full' })
const balanceParsed = balance({ address: "GAAAA" }, { responseType: 'parsed' })
const balanceSimple = balance({ address: "GAAAA" })
