import { ContractSpec, Address } from '@stellar/stellar-sdk';
import { Buffer } from "buffer";
import { AssembledTransaction, Ok, Err } from './assembled-tx.js';
export * from './assembled-tx.js';
export * from './method-options.js';
if (typeof window !== 'undefined') {
    //@ts-ignore Buffer exists
    window.Buffer = window.Buffer || Buffer;
}
export const networks = {
    futurenet: {
        networkPassphrase: "Test SDF Future Network ; October 2022",
        contractId: "CBYMYMSDF6FBDNCFJCRC7KMO4REYFPOH2U4N7FXI3GJO6YXNCQ43CDSK",
    }
};
/**
    
    */
export var RoyalCard;
(function (RoyalCard) {
    RoyalCard[RoyalCard["Jack"] = 11] = "Jack";
    RoyalCard[RoyalCard["Queen"] = 12] = "Queen";
    RoyalCard[RoyalCard["King"] = 13] = "King";
})(RoyalCard || (RoyalCard = {}));
/**
    
    */
export const Errors = {
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
    parsers = {
        hello: (result) => this.spec.funcResToNative("hello", result),
        woid: () => { },
        val: (result) => this.spec.funcResToNative("val", result),
        u32FailOnEven: (result) => {
            if (result instanceof Err)
                return result;
            return new Ok(this.spec.funcResToNative("u32_fail_on_even", result));
        },
        u32: (result) => this.spec.funcResToNative("u32_", result),
        i32: (result) => this.spec.funcResToNative("i32_", result),
        i64: (result) => this.spec.funcResToNative("i64_", result),
        struktHel: (result) => this.spec.funcResToNative("strukt_hel", result),
        strukt: (result) => this.spec.funcResToNative("strukt", result),
        simple: (result) => this.spec.funcResToNative("simple", result),
        complex: (result) => this.spec.funcResToNative("complex", result),
        addresse: (result) => this.spec.funcResToNative("addresse", result),
        bytes: (result) => this.spec.funcResToNative("bytes", result),
        bytesN: (result) => this.spec.funcResToNative("bytes_n", result),
        card: (result) => this.spec.funcResToNative("card", result),
        boolean: (result) => this.spec.funcResToNative("boolean", result),
        not: (result) => this.spec.funcResToNative("not", result),
        i128: (result) => this.spec.funcResToNative("i128", result),
        u128: (result) => this.spec.funcResToNative("u128", result),
        multiArgs: (result) => this.spec.funcResToNative("multi_args", result),
        map: (result) => this.spec.funcResToNative("map", result),
        vec: (result) => this.spec.funcResToNative("vec", result),
        tuple: (result) => this.spec.funcResToNative("tuple", result),
        option: (result) => this.spec.funcResToNative("option", result),
        u256: (result) => this.spec.funcResToNative("u256", result),
        i256: (result) => this.spec.funcResToNative("i256", result),
        string: (result) => this.spec.funcResToNative("string", result),
        tupleStrukt: (result) => this.spec.funcResToNative("tuple_strukt", result)
    };
    txFromJSON = (json) => {
        const { method, ...tx } = JSON.parse(json);
        return AssembledTransaction.fromJSON({
            ...this.options,
            method,
            parseResultXdr: this.parsers[method],
        }, tx);
    };
    fromJSON = {
        hello: (this.txFromJSON),
        woid: (this.txFromJSON),
        val: (this.txFromJSON),
        u32FailOnEven: (this.txFromJSON),
        u32: (this.txFromJSON),
        i32: (this.txFromJSON),
        i64: (this.txFromJSON),
        struktHel: (this.txFromJSON),
        strukt: (this.txFromJSON),
        simple: (this.txFromJSON),
        complex: (this.txFromJSON),
        addresse: (this.txFromJSON),
        bytes: (this.txFromJSON),
        bytesN: (this.txFromJSON),
        card: (this.txFromJSON),
        boolean: (this.txFromJSON),
        not: (this.txFromJSON),
        i128: (this.txFromJSON),
        u128: (this.txFromJSON),
        multiArgs: (this.txFromJSON),
        map: (this.txFromJSON),
        vec: (this.txFromJSON),
        tuple: (this.txFromJSON),
        option: (this.txFromJSON),
        u256: (this.txFromJSON),
        i256: (this.txFromJSON),
        string: (this.txFromJSON),
        tupleStrukt: (this.txFromJSON)
    };
    /**
* Construct and simulate a hello transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    hello = async ({ hello }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'hello',
            args: this.spec.funcArgsToScVals("hello", { hello }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['hello'],
        });
    };
    /**
* Construct and simulate a woid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    woid = async (options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'woid',
            args: this.spec.funcArgsToScVals("woid", {}),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['woid'],
        });
    };
    /**
* Construct and simulate a val transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    val = async (options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'val',
            args: this.spec.funcArgsToScVals("val", {}),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['val'],
        });
    };
    /**
* Construct and simulate a u32_fail_on_even transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u32FailOnEven = async ({ u32_ }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'u32_fail_on_even',
            args: this.spec.funcArgsToScVals("u32_fail_on_even", { u32_ }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['u32FailOnEven'],
        });
    };
    /**
* Construct and simulate a u32_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u32 = async ({ u32_ }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'u32_',
            args: this.spec.funcArgsToScVals("u32_", { u32_ }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['u32'],
        });
    };
    /**
* Construct and simulate a i32_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i32 = async ({ i32_ }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'i32_',
            args: this.spec.funcArgsToScVals("i32_", { i32_ }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['i32'],
        });
    };
    /**
* Construct and simulate a i64_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i64 = async ({ i64_ }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'i64_',
            args: this.spec.funcArgsToScVals("i64_", { i64_ }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['i64'],
        });
    };
    /**
* Construct and simulate a strukt_hel transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.Example contract method which takes a struct
*/
    struktHel = async ({ strukt }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'strukt_hel',
            args: this.spec.funcArgsToScVals("strukt_hel", { strukt }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['struktHel'],
        });
    };
    /**
* Construct and simulate a strukt transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    strukt = async ({ strukt }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'strukt',
            args: this.spec.funcArgsToScVals("strukt", { strukt }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['strukt'],
        });
    };
    /**
* Construct and simulate a simple transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    simple = async ({ simple }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'simple',
            args: this.spec.funcArgsToScVals("simple", { simple }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['simple'],
        });
    };
    /**
* Construct and simulate a complex transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    complex = async ({ complex }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'complex',
            args: this.spec.funcArgsToScVals("complex", { complex }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['complex'],
        });
    };
    /**
* Construct and simulate a addresse transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    addresse = async ({ addresse }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'addresse',
            args: this.spec.funcArgsToScVals("addresse", { addresse: new Address(addresse) }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['addresse'],
        });
    };
    /**
* Construct and simulate a bytes transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    bytes = async ({ bytes }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'bytes',
            args: this.spec.funcArgsToScVals("bytes", { bytes }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['bytes'],
        });
    };
    /**
* Construct and simulate a bytes_n transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    bytesN = async ({ bytes_n }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'bytes_n',
            args: this.spec.funcArgsToScVals("bytes_n", { bytes_n }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['bytesN'],
        });
    };
    /**
* Construct and simulate a card transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    card = async ({ card }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'card',
            args: this.spec.funcArgsToScVals("card", { card }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['card'],
        });
    };
    /**
* Construct and simulate a boolean transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    boolean = async ({ boolean }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'boolean',
            args: this.spec.funcArgsToScVals("boolean", { boolean }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['boolean'],
        });
    };
    /**
* Construct and simulate a not transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.Negates a boolean value
*/
    not = async ({ boolean }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'not',
            args: this.spec.funcArgsToScVals("not", { boolean }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['not'],
        });
    };
    /**
* Construct and simulate a i128 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i128 = async ({ i128 }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'i128',
            args: this.spec.funcArgsToScVals("i128", { i128 }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['i128'],
        });
    };
    /**
* Construct and simulate a u128 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u128 = async ({ u128 }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'u128',
            args: this.spec.funcArgsToScVals("u128", { u128 }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['u128'],
        });
    };
    /**
* Construct and simulate a multi_args transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    multiArgs = async ({ a, b }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'multi_args',
            args: this.spec.funcArgsToScVals("multi_args", { a, b }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['multiArgs'],
        });
    };
    /**
* Construct and simulate a map transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    map = async ({ map }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'map',
            args: this.spec.funcArgsToScVals("map", { map }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['map'],
        });
    };
    /**
* Construct and simulate a vec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    vec = async ({ vec }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'vec',
            args: this.spec.funcArgsToScVals("vec", { vec }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['vec'],
        });
    };
    /**
* Construct and simulate a tuple transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    tuple = async ({ tuple }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'tuple',
            args: this.spec.funcArgsToScVals("tuple", { tuple }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['tuple'],
        });
    };
    /**
* Construct and simulate a option transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.Example of an optional argument
*/
    option = async ({ option }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'option',
            args: this.spec.funcArgsToScVals("option", { option }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['option'],
        });
    };
    /**
* Construct and simulate a u256 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u256 = async ({ u256 }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'u256',
            args: this.spec.funcArgsToScVals("u256", { u256 }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['u256'],
        });
    };
    /**
* Construct and simulate a i256 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i256 = async ({ i256 }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'i256',
            args: this.spec.funcArgsToScVals("i256", { i256 }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['i256'],
        });
    };
    /**
* Construct and simulate a string transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    string = async ({ string }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'string',
            args: this.spec.funcArgsToScVals("string", { string }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['string'],
        });
    };
    /**
* Construct and simulate a tuple_strukt transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    tupleStrukt = async ({ tuple_strukt }, options = {}) => {
        return await AssembledTransaction.fromSimulation({
            method: 'tuple_strukt',
            args: this.spec.funcArgsToScVals("tuple_strukt", { tuple_strukt }),
            ...options,
            ...this.options,
            errorTypes: Errors,
            parseResultXdr: this.parsers['tupleStrukt'],
        });
    };
}
