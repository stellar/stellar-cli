import { xdr, Address, nativeToScVal, scValToBigInt, ScInt } from 'soroban-client';
export function strToScVal(base64Xdr) {
    return xdr.ScVal.fromXDR(base64Xdr, 'base64');
}
export function scValStrToJs(base64Xdr) {
    return scValToJs(strToScVal(base64Xdr));
}
export function scValToJs(val) {
    switch (val?.switch()) {
        case xdr.ScValType.scvBool(): {
            return val.b();
        }
        case xdr.ScValType.scvVoid():
        case undefined: {
            return 0;
        }
        case xdr.ScValType.scvU32(): {
            return val.u32();
        }
        case xdr.ScValType.scvI32(): {
            return val.i32();
        }
        case xdr.ScValType.scvU64():
        case xdr.ScValType.scvI64():
        case xdr.ScValType.scvU128():
        case xdr.ScValType.scvI128():
        case xdr.ScValType.scvU256():
        case xdr.ScValType.scvI256(): {
            return scValToBigInt(val);
        }
        case xdr.ScValType.scvAddress(): {
            return Address.fromScVal(val).toString();
        }
        case xdr.ScValType.scvString(): {
            return val.str().toString();
        }
        case xdr.ScValType.scvSymbol(): {
            return val.sym().toString();
        }
        case xdr.ScValType.scvBytes(): {
            return val.bytes();
        }
        case xdr.ScValType.scvVec(): {
            return val.vec().map(v => scValToJs(v));
        }
        case xdr.ScValType.scvMap(): {
            let res = {};
            val.map().forEach((e) => {
                let key = scValToJs(e.key());
                let value;
                let v = e.val();
                // For now we assume second level maps are real maps. Not perfect but better.
                switch (v?.switch()) {
                    case xdr.ScValType.scvMap(): {
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
        case xdr.ScValType.scvContractInstance():
        case xdr.ScValType.scvLedgerKeyNonce():
        case xdr.ScValType.scvTimepoint():
        case xdr.ScValType.scvDuration():
            return val.value();
        // TODO: Add this case when merged
        // case xdr.ScValType.scvError():
        default: {
            throw new Error(`type not implemented yet: ${val?.switch().name}`);
        }
    }
    ;
}
export function addressToScVal(addr) {
    return nativeToScVal(addr, { type: 'address' } /* bug workaround */);
}
export function i128ToScVal(i) {
    return new ScInt(i).toI128();
}
export function u128ToScVal(i) {
    return new ScInt(i).toU128();
}
