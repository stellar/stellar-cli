import { ContractSpec, Address } from 'soroban-client';
import { Buffer } from "buffer";
import { invoke } from './invoke.js';
export * from './invoke.js';
export * from './method-options.js';
export { Address };
;
;
export class Ok {
    value;
    constructor(value) {
        this.value = value;
    }
    unwrapErr() {
        throw new Error('No error');
    }
    unwrap() {
        return this.value;
    }
    isOk() {
        return true;
    }
    isErr() {
        return !this.isOk();
    }
}
export class Err {
    error;
    constructor(error) {
        this.error = error;
    }
    unwrapErr() {
        return this.error;
    }
    unwrap() {
        throw new Error(this.error.message);
    }
    isOk() {
        return false;
    }
    isErr() {
        return !this.isOk();
    }
}
if (typeof window !== 'undefined') {
    //@ts-ignore Buffer exists
    window.Buffer = window.Buffer || Buffer;
}
const regex = /Error\(Contract, #(\d+)\)/;
function parseError(message) {
    const match = message.match(regex);
    if (!match) {
        return undefined;
    }
    if (Errors === undefined) {
        return undefined;
    }
    let i = parseInt(match[1], 10);
    let err = Errors[i];
    if (err) {
        return new Err(err);
    }
    return undefined;
}
export const networks = {
    futurenet: {
        networkPassphrase: "Test SDF Future Network ; October 2022",
        contractId: "CBYMYMSDF6FBDNCFJCRC7KMO4REYFPOH2U4N7FXI3GJO6YXNCQ43CDSK",
    }
};
export var RoyalCard;
(function (RoyalCard) {
    RoyalCard[RoyalCard["Jack"] = 11] = "Jack";
    RoyalCard[RoyalCard["Queen"] = 12] = "Queen";
    RoyalCard[RoyalCard["King"] = 13] = "King";
})(RoyalCard || (RoyalCard = {}));
const Errors = {
    1: { message: "Please provide an odd number" }
};
export class Contract {
    options;
    spec;
    constructor(options) {
        this.options = options;
        this.spec = new ContractSpec([
            "AAAAAQAAAC9UaGlzIGlzIGZyb20gdGhlIHJ1c3QgZG9jIGFib3ZlIHRoZSBzdHJ1Y3QgVGVzdAAAAAAAAAAABFRlc3QAAAADAAAAAAAAAAFhAAAAAAAABAAAAAAAAAABYgAAAAAAAAEAAAAAAAAAAWMAAAAAAAAR",
            "AAAAAgAAAAAAAAAAAAAAClNpbXBsZUVudW0AAAAAAAMAAAAAAAAAAAAAAAVGaXJzdAAAAAAAAAAAAAAAAAAABlNlY29uZAAAAAAAAAAAAAAAAAAFVGhpcmQAAAA=",
            "AAAAAwAAAAAAAAAAAAAACVJveWFsQ2FyZAAAAAAAAAMAAAAAAAAABEphY2sAAAALAAAAAAAAAAVRdWVlbgAAAAAAAAwAAAAAAAAABEtpbmcAAAAN",
            "AAAAAQAAAAAAAAAAAAAAC1R1cGxlU3RydWN0AAAAAAIAAAAAAAAAATAAAAAAAAfQAAAABFRlc3QAAAAAAAAAATEAAAAAAAfQAAAAClNpbXBsZUVudW0AAA==",
            "AAAAAgAAAAAAAAAAAAAAC0NvbXBsZXhFbnVtAAAAAAUAAAABAAAAAAAAAAZTdHJ1Y3QAAAAAAAEAAAfQAAAABFRlc3QAAAABAAAAAAAAAAVUdXBsZQAAAAAAAAEAAAfQAAAAC1R1cGxlU3RydWN0AAAAAAEAAAAAAAAABEVudW0AAAABAAAH0AAAAApTaW1wbGVFbnVtAAAAAAABAAAAAAAAAAVBc3NldAAAAAAAAAIAAAATAAAACwAAAAAAAAAAAAAABFZvaWQ=",
            "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAAAQAAABxQbGVhc2UgcHJvdmlkZSBhbiBvZGQgbnVtYmVyAAAAD051bWJlck11c3RCZU9kZAAAAAAB",
            "AAAAAAAAAAAAAAAFaGVsbG8AAAAAAAABAAAAAAAAAAVoZWxsbwAAAAAAABEAAAABAAAAEQ==",
            "AAAAAAAAAAAAAAAEd29pZAAAAAAAAAAA",
            "AAAAAAAAAAAAAAADdmFsAAAAAAAAAAABAAAAAA==",
            "AAAAAAAAAAAAAAAQdTMyX2ZhaWxfb25fZXZlbgAAAAEAAAAAAAAABHUzMl8AAAAEAAAAAQAAA+kAAAAEAAAAAw==",
            "AAAAAAAAAAAAAAAEdTMyXwAAAAEAAAAAAAAABHUzMl8AAAAEAAAAAQAAAAQ=",
            "AAAAAAAAAAAAAAAEaTMyXwAAAAEAAAAAAAAABGkzMl8AAAAFAAAAAQAAAAU=",
            "AAAAAAAAAAAAAAAEaTY0XwAAAAEAAAAAAAAABGk2NF8AAAAHAAAAAQAAAAc=",
            "AAAAAAAAACxFeGFtcGxlIGNvbnRyYWN0IG1ldGhvZCB3aGljaCB0YWtlcyBhIHN0cnVjdAAAAApzdHJ1a3RfaGVsAAAAAAABAAAAAAAAAAZzdHJ1a3QAAAAAB9AAAAAEVGVzdAAAAAEAAAPqAAAAEQ==",
            "AAAAAAAAAAAAAAAGc3RydWt0AAAAAAABAAAAAAAAAAZzdHJ1a3QAAAAAB9AAAAAEVGVzdAAAAAEAAAfQAAAABFRlc3Q=",
            "AAAAAAAAAAAAAAAGc2ltcGxlAAAAAAABAAAAAAAAAAZzaW1wbGUAAAAAB9AAAAAKU2ltcGxlRW51bQAAAAAAAQAAB9AAAAAKU2ltcGxlRW51bQAA",
            "AAAAAAAAAAAAAAAHY29tcGxleAAAAAABAAAAAAAAAAdjb21wbGV4AAAAB9AAAAALQ29tcGxleEVudW0AAAAAAQAAB9AAAAALQ29tcGxleEVudW0A",
            "AAAAAAAAAAAAAAAIYWRkcmVzc2UAAAABAAAAAAAAAAhhZGRyZXNzZQAAABMAAAABAAAAEw==",
            "AAAAAAAAAAAAAAAFYnl0ZXMAAAAAAAABAAAAAAAAAAVieXRlcwAAAAAAAA4AAAABAAAADg==",
            "AAAAAAAAAAAAAAAHYnl0ZXNfbgAAAAABAAAAAAAAAAdieXRlc19uAAAAA+4AAAAJAAAAAQAAA+4AAAAJ",
            "AAAAAAAAAAAAAAAEY2FyZAAAAAEAAAAAAAAABGNhcmQAAAfQAAAACVJveWFsQ2FyZAAAAAAAAAEAAAfQAAAACVJveWFsQ2FyZAAAAA==",
            "AAAAAAAAAAAAAAAHYm9vbGVhbgAAAAABAAAAAAAAAAdib29sZWFuAAAAAAEAAAABAAAAAQ==",
            "AAAAAAAAABdOZWdhdGVzIGEgYm9vbGVhbiB2YWx1ZQAAAAADbm90AAAAAAEAAAAAAAAAB2Jvb2xlYW4AAAAAAQAAAAEAAAAB",
            "AAAAAAAAAAAAAAAEaTEyOAAAAAEAAAAAAAAABGkxMjgAAAALAAAAAQAAAAs=",
            "AAAAAAAAAAAAAAAEdTEyOAAAAAEAAAAAAAAABHUxMjgAAAAKAAAAAQAAAAo=",
            "AAAAAAAAAAAAAAAKbXVsdGlfYXJncwAAAAAAAgAAAAAAAAABYQAAAAAAAAQAAAAAAAAAAWIAAAAAAAABAAAAAQAAAAQ=",
            "AAAAAAAAAAAAAAADbWFwAAAAAAEAAAAAAAAAA21hcAAAAAPsAAAABAAAAAEAAAABAAAD7AAAAAQAAAAB",
            "AAAAAAAAAAAAAAADdmVjAAAAAAEAAAAAAAAAA3ZlYwAAAAPqAAAABAAAAAEAAAPqAAAABA==",
            "AAAAAAAAAAAAAAAFdHVwbGUAAAAAAAABAAAAAAAAAAV0dXBsZQAAAAAAA+0AAAACAAAAEQAAAAQAAAABAAAD7QAAAAIAAAARAAAABA==",
            "AAAAAAAAAB9FeGFtcGxlIG9mIGFuIG9wdGlvbmFsIGFyZ3VtZW50AAAAAAZvcHRpb24AAAAAAAEAAAAAAAAABm9wdGlvbgAAAAAD6AAAAAQAAAABAAAD6AAAAAQ=",
            "AAAAAAAAAAAAAAAEdTI1NgAAAAEAAAAAAAAABHUyNTYAAAAMAAAAAQAAAAw=",
            "AAAAAAAAAAAAAAAEaTI1NgAAAAEAAAAAAAAABGkyNTYAAAANAAAAAQAAAA0=",
            "AAAAAAAAAAAAAAAGc3RyaW5nAAAAAAABAAAAAAAAAAZzdHJpbmcAAAAAABAAAAABAAAAEA==",
            "AAAAAAAAAAAAAAAMdHVwbGVfc3RydWt0AAAAAQAAAAAAAAAMdHVwbGVfc3RydWt0AAAH0AAAAAtUdXBsZVN0cnVjdAAAAAABAAAH0AAAAAtUdXBsZVN0cnVjdAA="
        ]);
    }
    hello = async ({ hello }, options = {}) => {
        return await invoke({
            method: 'hello',
            args: this.spec.funcArgsToScVals("hello", { hello }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("hello", xdr);
            },
        });
    };
    woid = async (options = {}) => {
        return await invoke({
            method: 'woid',
            args: this.spec.funcArgsToScVals("woid", {}),
            ...options,
            ...this.options,
            parseResultXdr: () => { },
        });
    };
    val = async (options = {}) => {
        return await invoke({
            method: 'val',
            args: this.spec.funcArgsToScVals("val", {}),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("val", xdr);
            },
        });
    };
    u32FailOnEven = async ({ u32_ }, options = {}) => {
        try {
            return await invoke({
                method: 'u32_fail_on_even',
                args: this.spec.funcArgsToScVals("u32_fail_on_even", { u32_ }),
                ...options,
                ...this.options,
                parseResultXdr: (xdr) => {
                    return new Ok(this.spec.funcResToNative("u32_fail_on_even", xdr));
                },
            });
        }
        catch (e) {
            if (typeof e === 'string') {
                let err = parseError(e);
                if (err)
                    return err;
            }
            throw e;
        }
    };
    u32 = async ({ u32_ }, options = {}) => {
        return await invoke({
            method: 'u32_',
            args: this.spec.funcArgsToScVals("u32_", { u32_ }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("u32_", xdr);
            },
        });
    };
    i32 = async ({ i32_ }, options = {}) => {
        return await invoke({
            method: 'i32_',
            args: this.spec.funcArgsToScVals("i32_", { i32_ }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("i32_", xdr);
            },
        });
    };
    i64 = async ({ i64_ }, options = {}) => {
        return await invoke({
            method: 'i64_',
            args: this.spec.funcArgsToScVals("i64_", { i64_ }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("i64_", xdr);
            },
        });
    };
    /**
     * Example contract method which takes a struct
     */
    struktHel = async ({ strukt }, options = {}) => {
        return await invoke({
            method: 'strukt_hel',
            args: this.spec.funcArgsToScVals("strukt_hel", { strukt }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("strukt_hel", xdr);
            },
        });
    };
    strukt = async ({ strukt }, options = {}) => {
        return await invoke({
            method: 'strukt',
            args: this.spec.funcArgsToScVals("strukt", { strukt }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("strukt", xdr);
            },
        });
    };
    simple = async ({ simple }, options = {}) => {
        return await invoke({
            method: 'simple',
            args: this.spec.funcArgsToScVals("simple", { simple }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("simple", xdr);
            },
        });
    };
    complex = async ({ complex }, options = {}) => {
        return await invoke({
            method: 'complex',
            args: this.spec.funcArgsToScVals("complex", { complex }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("complex", xdr);
            },
        });
    };
    addresse = async ({ addresse }, options = {}) => {
        return await invoke({
            method: 'addresse',
            args: this.spec.funcArgsToScVals("addresse", { addresse }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("addresse", xdr);
            },
        });
    };
    bytes = async ({ bytes }, options = {}) => {
        return await invoke({
            method: 'bytes',
            args: this.spec.funcArgsToScVals("bytes", { bytes }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("bytes", xdr);
            },
        });
    };
    bytesN = async ({ bytes_n }, options = {}) => {
        return await invoke({
            method: 'bytes_n',
            args: this.spec.funcArgsToScVals("bytes_n", { bytes_n }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("bytes_n", xdr);
            },
        });
    };
    card = async ({ card }, options = {}) => {
        return await invoke({
            method: 'card',
            args: this.spec.funcArgsToScVals("card", { card }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("card", xdr);
            },
        });
    };
    boolean = async ({ boolean }, options = {}) => {
        return await invoke({
            method: 'boolean',
            args: this.spec.funcArgsToScVals("boolean", { boolean }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("boolean", xdr);
            },
        });
    };
    /**
     * Negates a boolean value
     */
    not = async ({ boolean }, options = {}) => {
        return await invoke({
            method: 'not',
            args: this.spec.funcArgsToScVals("not", { boolean }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("not", xdr);
            },
        });
    };
    i128 = async ({ i128 }, options = {}) => {
        return await invoke({
            method: 'i128',
            args: this.spec.funcArgsToScVals("i128", { i128 }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("i128", xdr);
            },
        });
    };
    u128 = async ({ u128 }, options = {}) => {
        return await invoke({
            method: 'u128',
            args: this.spec.funcArgsToScVals("u128", { u128 }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("u128", xdr);
            },
        });
    };
    multiArgs = async ({ a, b }, options = {}) => {
        return await invoke({
            method: 'multi_args',
            args: this.spec.funcArgsToScVals("multi_args", { a, b }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("multi_args", xdr);
            },
        });
    };
    map = async ({ map }, options = {}) => {
        return await invoke({
            method: 'map',
            args: this.spec.funcArgsToScVals("map", { map }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("map", xdr);
            },
        });
    };
    vec = async ({ vec }, options = {}) => {
        return await invoke({
            method: 'vec',
            args: this.spec.funcArgsToScVals("vec", { vec }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("vec", xdr);
            },
        });
    };
    tuple = async ({ tuple }, options = {}) => {
        return await invoke({
            method: 'tuple',
            args: this.spec.funcArgsToScVals("tuple", { tuple }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("tuple", xdr);
            },
        });
    };
    /**
     * Example of an optional argument
     */
    option = async ({ option }, options = {}) => {
        return await invoke({
            method: 'option',
            args: this.spec.funcArgsToScVals("option", { option }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("option", xdr);
            },
        });
    };
    u256 = async ({ u256 }, options = {}) => {
        return await invoke({
            method: 'u256',
            args: this.spec.funcArgsToScVals("u256", { u256 }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("u256", xdr);
            },
        });
    };
    i256 = async ({ i256 }, options = {}) => {
        return await invoke({
            method: 'i256',
            args: this.spec.funcArgsToScVals("i256", { i256 }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("i256", xdr);
            },
        });
    };
    string = async ({ string }, options = {}) => {
        return await invoke({
            method: 'string',
            args: this.spec.funcArgsToScVals("string", { string }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("string", xdr);
            },
        });
    };
    tupleStrukt = async ({ tuple_strukt }, options = {}) => {
        return await invoke({
            method: 'tuple_strukt',
            args: this.spec.funcArgsToScVals("tuple_strukt", { tuple_strukt }),
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return this.spec.funcResToNative("tuple_strukt", xdr);
            },
        });
    };
}
