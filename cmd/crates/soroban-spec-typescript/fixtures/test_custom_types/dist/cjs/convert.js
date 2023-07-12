"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.u128ToScVal = exports.i128ToScVal = exports.addressToScVal = exports.scValToJs = exports.scValStrToJs = exports.strToScVal = exports.scvalToBigInt = void 0;
const soroban_client_1 = require("soroban-client");
const buffer_1 = require("buffer");
const bigint_conversion_1 = require("bigint-conversion");
function scvalToBigInt(scval) {
    switch (scval?.switch()) {
        case undefined: {
            return BigInt(0);
        }
        case soroban_client_1.xdr.ScValType.scvU64(): {
            const { high, low } = scval.u64();
            return (0, bigint_conversion_1.bufToBigint)(new Uint32Array([high, low]));
        }
        case soroban_client_1.xdr.ScValType.scvI64(): {
            const { high, low } = scval.i64();
            return (0, bigint_conversion_1.bufToBigint)(new Int32Array([high, low]));
        }
        case soroban_client_1.xdr.ScValType.scvU128(): {
            const parts = scval.u128();
            const a = parts.hi();
            const b = parts.lo();
            return (0, bigint_conversion_1.bufToBigint)(new Uint32Array([a.high, a.low, b.high, b.low]));
        }
        case soroban_client_1.xdr.ScValType.scvI128(): {
            const parts = scval.i128();
            const a = parts.hi();
            const b = parts.lo();
            return (0, bigint_conversion_1.bufToBigint)(new Int32Array([a.high, a.low, b.high, b.low]));
        }
        default: {
            throw new Error(`Invalid type for scvalToBigInt: ${scval?.switch().name}`);
        }
    }
    ;
}
exports.scvalToBigInt = scvalToBigInt;
function strToScVal(base64Xdr) {
    return soroban_client_1.xdr.ScVal.fromXDR(buffer_1.Buffer.from(base64Xdr, 'base64'));
}
exports.strToScVal = strToScVal;
function scValStrToJs(base64Xdr) {
    return scValToJs(strToScVal(base64Xdr));
}
exports.scValStrToJs = scValStrToJs;
function scValToJs(val) {
    switch (val?.switch()) {
        case soroban_client_1.xdr.ScValType.scvBool(): {
            return val.b();
        }
        case soroban_client_1.xdr.ScValType.scvVoid():
        case undefined: {
            return 0;
        }
        case soroban_client_1.xdr.ScValType.scvU32(): {
            return val.u32();
        }
        case soroban_client_1.xdr.ScValType.scvI32(): {
            return val.i32();
        }
        case soroban_client_1.xdr.ScValType.scvU64():
        case soroban_client_1.xdr.ScValType.scvI64():
        case soroban_client_1.xdr.ScValType.scvU128():
        case soroban_client_1.xdr.ScValType.scvI128():
        case soroban_client_1.xdr.ScValType.scvU256():
        case soroban_client_1.xdr.ScValType.scvI256(): {
            return scvalToBigInt(val);
        }
        case soroban_client_1.xdr.ScValType.scvAddress(): {
            return soroban_client_1.Address.fromScVal(val).toString();
        }
        case soroban_client_1.xdr.ScValType.scvString(): {
            return val.str().toString();
        }
        case soroban_client_1.xdr.ScValType.scvSymbol(): {
            return val.sym().toString();
        }
        case soroban_client_1.xdr.ScValType.scvBytes(): {
            return val.bytes();
        }
        case soroban_client_1.xdr.ScValType.scvVec(): {
            return val.vec().map(v => scValToJs(v));
        }
        case soroban_client_1.xdr.ScValType.scvMap(): {
            let res = {};
            val.map().forEach((e) => {
                let key = scValToJs(e.key());
                let value;
                let v = e.val();
                // For now we assume second level maps are real maps. Not perfect but better.
                switch (v?.switch()) {
                    case soroban_client_1.xdr.ScValType.scvMap(): {
                        let inner_map = new Map();
                        v.map().forEach((e) => {
                            let key = scValToJs(e.key());
                            let value = scValToJs(e.val());
                            inner_map.set(key, value);
                        });
                        value = inner_map;
                        break;
                    }
                    default: {
                        value = scValToJs(e.val());
                    }
                }
                //@ts-ignore
                res[key] = value;
            });
            return res;
        }
        case soroban_client_1.xdr.ScValType.scvContractInstance():
            return val.instance();
        case soroban_client_1.xdr.ScValType.scvLedgerKeyNonce():
            return val.nonceKey();
        case soroban_client_1.xdr.ScValType.scvTimepoint():
            return val.timepoint();
        case soroban_client_1.xdr.ScValType.scvDuration():
            return val.duration();
        // TODO: Add this case when merged
        // case xdr.ScValType.scvError():
        default: {
            throw new Error(`type not implemented yet: ${val?.switch().name}`);
        }
    }
    ;
}
exports.scValToJs = scValToJs;
function addressToScVal(addr) {
    let addrObj = soroban_client_1.Address.fromString(addr);
    return addrObj.toScVal();
}
exports.addressToScVal = addressToScVal;
function i128ToScVal(i) {
    return soroban_client_1.xdr.ScVal.scvI128(new soroban_client_1.xdr.Int128Parts({
        lo: soroban_client_1.xdr.Uint64.fromString((i & BigInt(0xffffffffffffffffn)).toString()),
        hi: soroban_client_1.xdr.Int64.fromString(((i >> BigInt(64)) & BigInt(0xffffffffffffffffn)).toString()),
    }));
}
exports.i128ToScVal = i128ToScVal;
function u128ToScVal(i) {
    return soroban_client_1.xdr.ScVal.scvU128(new soroban_client_1.xdr.UInt128Parts({
        lo: soroban_client_1.xdr.Uint64.fromString((i & BigInt(0xffffffffffffffffn)).toString()),
        hi: soroban_client_1.xdr.Int64.fromString(((i >> BigInt(64)) & BigInt(0xffffffffffffffffn)).toString()),
    }));
}
exports.u128ToScVal = u128ToScVal;
