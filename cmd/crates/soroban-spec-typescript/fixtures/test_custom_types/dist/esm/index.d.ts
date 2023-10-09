import { ContractSpec } from '@stellar/stellar-sdk';
import { Buffer } from "buffer";
import { AssembledTransaction, Ok, Err } from './assembled-tx.js';
import type { u32, i32, i64, i128, Option, Error_ } from './assembled-tx.js';
import type { ClassOptions } from './method-options.js';
export * from './assembled-tx.js';
export * from './method-options.js';
export declare const networks: {
    readonly futurenet: {
        readonly networkPassphrase: "Test SDF Future Network ; October 2022";
        readonly contractId: "CBYMYMSDF6FBDNCFJCRC7KMO4REYFPOH2U4N7FXI3GJO6YXNCQ43CDSK";
    };
};
/**
    This is from the rust doc above the struct Test
    */
export interface Test {
    /**
      
      */
    a: u32;
    /**
      
      */
    b: boolean;
    /**
      
      */
    c: string;
}
/**
    
    */
export type SimpleEnum = {
    tag: "First";
    values: void;
} | {
    tag: "Second";
    values: void;
} | {
    tag: "Third";
    values: void;
};
/**
    
    */
export declare enum RoyalCard {
    Jack = 11,
    Queen = 12,
    King = 13
}
/**
    
    */
export type TupleStruct = readonly [Test, SimpleEnum];
/**
    
    */
export type ComplexEnum = {
    tag: "Struct";
    values: readonly [Test];
} | {
    tag: "Tuple";
    values: readonly [TupleStruct];
} | {
    tag: "Enum";
    values: readonly [SimpleEnum];
} | {
    tag: "Asset";
    values: readonly [string, i128];
} | {
    tag: "Void";
    values: void;
};
/**
    
    */
export declare const Errors: {
    1: {
        message: string;
    };
};
export declare class Contract {
    readonly options: ClassOptions;
    spec: ContractSpec;
    constructor(options: ClassOptions);
    private readonly parsers;
    private txFromJSON;
    readonly fromJSON: {
        hello: (json: string) => AssembledTransaction<string>;
        woid: (json: string) => AssembledTransaction<void>;
        val: (json: string) => AssembledTransaction<any>;
        u32FailOnEven: (json: string) => AssembledTransaction<Err<Error_> | Ok<number, Error_>>;
        u32: (json: string) => AssembledTransaction<number>;
        i32: (json: string) => AssembledTransaction<number>;
        i64: (json: string) => AssembledTransaction<bigint>;
        struktHel: (json: string) => AssembledTransaction<string[]>;
        strukt: (json: string) => AssembledTransaction<Test>;
        simple: (json: string) => AssembledTransaction<SimpleEnum>;
        complex: (json: string) => AssembledTransaction<ComplexEnum>;
        addresse: (json: string) => AssembledTransaction<string>;
        bytes: (json: string) => AssembledTransaction<Buffer>;
        bytesN: (json: string) => AssembledTransaction<Buffer>;
        card: (json: string) => AssembledTransaction<RoyalCard>;
        boolean: (json: string) => AssembledTransaction<boolean>;
        not: (json: string) => AssembledTransaction<boolean>;
        i128: (json: string) => AssembledTransaction<bigint>;
        u128: (json: string) => AssembledTransaction<bigint>;
        multiArgs: (json: string) => AssembledTransaction<number>;
        map: (json: string) => AssembledTransaction<Map<number, boolean>>;
        vec: (json: string) => AssembledTransaction<number[]>;
        tuple: (json: string) => AssembledTransaction<readonly [string, number]>;
        option: (json: string) => AssembledTransaction<Option<number>>;
        u256: (json: string) => AssembledTransaction<bigint>;
        i256: (json: string) => AssembledTransaction<bigint>;
        string: (json: string) => AssembledTransaction<string>;
        tupleStrukt: (json: string) => AssembledTransaction<TupleStruct>;
    };
    /**
* Construct and simulate a hello transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    hello: ({ hello }: {
        hello: string;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<string>>;
    /**
* Construct and simulate a woid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    woid: (options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<void>>;
    /**
* Construct and simulate a val transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    val: (options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<any>>;
    /**
* Construct and simulate a u32_fail_on_even transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u32FailOnEven: ({ u32_ }: {
        u32_: u32;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<Err<Error_> | Ok<number, Error_>>>;
    /**
* Construct and simulate a u32_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u32: ({ u32_ }: {
        u32_: u32;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<number>>;
    /**
* Construct and simulate a i32_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i32: ({ i32_ }: {
        i32_: i32;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<number>>;
    /**
* Construct and simulate a i64_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i64: ({ i64_ }: {
        i64_: i64;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<bigint>>;
    /**
* Construct and simulate a strukt_hel transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.Example contract method which takes a struct
*/
    struktHel: ({ strukt }: {
        strukt: Test;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<string[]>>;
    /**
* Construct and simulate a strukt transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    strukt: ({ strukt }: {
        strukt: Test;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<Test>>;
    /**
* Construct and simulate a simple transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    simple: ({ simple }: {
        simple: SimpleEnum;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<SimpleEnum>>;
    /**
* Construct and simulate a complex transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    complex: ({ complex }: {
        complex: ComplexEnum;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<ComplexEnum>>;
    /**
* Construct and simulate a addresse transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    addresse: ({ addresse }: {
        addresse: string;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<string>>;
    /**
* Construct and simulate a bytes transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    bytes: ({ bytes }: {
        bytes: Buffer;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<Buffer>>;
    /**
* Construct and simulate a bytes_n transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    bytesN: ({ bytes_n }: {
        bytes_n: Buffer;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<Buffer>>;
    /**
* Construct and simulate a card transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    card: ({ card }: {
        card: RoyalCard;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<RoyalCard>>;
    /**
* Construct and simulate a boolean transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    boolean: ({ boolean }: {
        boolean: boolean;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<boolean>>;
    /**
* Construct and simulate a not transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.Negates a boolean value
*/
    not: ({ boolean }: {
        boolean: boolean;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<boolean>>;
    /**
* Construct and simulate a i128 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i128: ({ i128 }: {
        i128: bigint;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<bigint>>;
    /**
* Construct and simulate a u128 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u128: ({ u128 }: {
        u128: bigint;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<bigint>>;
    /**
* Construct and simulate a multi_args transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    multiArgs: ({ a, b }: {
        a: u32;
        b: boolean;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<number>>;
    /**
* Construct and simulate a map transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    map: ({ map }: {
        map: Map<u32, boolean>;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<Map<number, boolean>>>;
    /**
* Construct and simulate a vec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    vec: ({ vec }: {
        vec: Array<u32>;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<number[]>>;
    /**
* Construct and simulate a tuple transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    tuple: ({ tuple }: {
        tuple: readonly [string, u32];
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<readonly [string, number]>>;
    /**
* Construct and simulate a option transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.Example of an optional argument
*/
    option: ({ option }: {
        option: Option<u32>;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<Option<number>>>;
    /**
* Construct and simulate a u256 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    u256: ({ u256 }: {
        u256: bigint;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<bigint>>;
    /**
* Construct and simulate a i256 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    i256: ({ i256 }: {
        i256: bigint;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<bigint>>;
    /**
* Construct and simulate a string transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    string: ({ string }: {
        string: string;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<string>>;
    /**
* Construct and simulate a tuple_strukt transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
*/
    tupleStrukt: ({ tuple_strukt }: {
        tuple_strukt: TupleStruct;
    }, options?: {
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number;
    }) => Promise<AssembledTransaction<TupleStruct>>;
}
