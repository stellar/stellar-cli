import { Buffer } from "buffer";
export * from './constants.js';
export * from './server.js';
export * from './invoke.js';
export type u32 = number;
export type i32 = number;
export type u64 = bigint;
export type i64 = bigint;
export type u128 = bigint;
export type i128 = bigint;
export type u256 = bigint;
export type i256 = bigint;
export type Address = string;
export type Option<T> = T | undefined;
export type Typepoint = bigint;
export type Duration = bigint;
export interface Error_ {
    message: string;
}
export interface Result<T, E = Error_> {
    unwrap(): T;
    unwrapErr(): E;
    isOk(): boolean;
    isErr(): boolean;
}
export declare class Ok<T> implements Result<T> {
    readonly value: T;
    constructor(value: T);
    unwrapErr(): Error_;
    unwrap(): T;
    isOk(): boolean;
    isErr(): boolean;
}
export declare class Err<T> implements Result<T> {
    readonly error: Error_;
    constructor(error: Error_);
    unwrapErr(): Error_;
    unwrap(): never;
    isOk(): boolean;
    isErr(): boolean;
}
/**
 * This is from the rust doc above the struct Test
 */
export interface Test {
    a: u32;
    b: boolean;
    c: string;
}
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
export declare enum RoyalCard {
    Jack = 11,
    Queen = 12,
    King = 13
}
export type TupleStruct = [Test, SimpleEnum];
export type ComplexEnum = {
    tag: "Struct";
    values: [Test];
} | {
    tag: "Tuple";
    values: [TupleStruct];
} | {
    tag: "Enum";
    values: [SimpleEnum];
} | {
    tag: "Void";
    values: void;
};
export declare function hello({ hello }: {
    hello: string;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<string>;
export declare function woid({ signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<void>;
export declare function val({ signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<any>;
export declare function u32_fail_on_even({ u32_ }: {
    u32_: u32;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Result<u32>>;
export declare function u32_({ u32_ }: {
    u32_: u32;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<u32>;
export declare function i32_({ i32_ }: {
    i32_: i32;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<i32>;
export declare function i64_({ i64_ }: {
    i64_: i64;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<i64>;
/**
 * Example contract method which takes a struct
 */
export declare function strukt_hel({ strukt }: {
    strukt: Test;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Array<string>>;
export declare function strukt({ strukt }: {
    strukt: Test;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Test>;
export declare function simple({ simple }: {
    simple: SimpleEnum;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<SimpleEnum>;
export declare function complex({ complex }: {
    complex: ComplexEnum;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<ComplexEnum>;
export declare function addresse({ addresse }: {
    addresse: Address;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Address>;
export declare function bytes({ bytes }: {
    bytes: Buffer;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Buffer>;
export declare function bytes_n({ bytes_n }: {
    bytes_n: Buffer;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Buffer>;
export declare function card({ card }: {
    card: RoyalCard;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<RoyalCard>;
export declare function boolean({ boolean }: {
    boolean: boolean;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<boolean>;
/**
 * Negates a boolean value
 */
export declare function not({ boolean }: {
    boolean: boolean;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<boolean>;
export declare function i128({ i128 }: {
    i128: i128;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<i128>;
export declare function u128({ u128 }: {
    u128: u128;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<u128>;
export declare function multi_args({ a, b }: {
    a: u32;
    b: boolean;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<u32>;
export declare function map({ map }: {
    map: Map<u32, boolean>;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Map<u32, boolean>>;
export declare function vec({ vec }: {
    vec: Array<u32>;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Array<u32>>;
export declare function tuple({ tuple }: {
    tuple: [string, u32];
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<[string, u32]>;
/**
 * Example of an optional argument
 */
export declare function option({ option }: {
    option: Option<u32>;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<Option<u32>>;
export declare function u256({ u256 }: {
    u256: u256;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<u256>;
export declare function i256({ i256 }: {
    i256: i256;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<i256>;
export declare function string({ string }: {
    string: string;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<string>;
export declare function tuple_strukt({ tuple_strukt }: {
    tuple_strukt: TupleStruct;
}, { signAndSend, fee }?: {
    signAndSend?: boolean;
    fee?: number;
}): Promise<TupleStruct>;
