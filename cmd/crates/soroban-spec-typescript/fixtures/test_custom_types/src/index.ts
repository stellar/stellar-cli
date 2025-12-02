import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  AssembledTransactionOptions,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}





/**
 * This is from the rust doc above the struct Test
 */
export interface Test {
  a: u32;
  b: boolean;
  c: string;
}

export const Errors = {
  /**
   * Please provide an odd number
   */
  1: {message:"NumberMustBeOdd"}
}

export const Erroneous = {
  /**
   * Some contract libraries contain extra #[contracterror] definitions that end up compiled
   * into the main contract types. We need to make sure tooling deals with this properly.
   */
  100: {message:"HowCouldYou"}
}

export enum RoyalCard {
  Jack = 11,
  Queen = 12,
  King = 13,
}

export type SimpleEnum = {tag: "First", values: void} | {tag: "Second", values: void} | {tag: "Third", values: void};

export type ComplexEnum = {tag: "Struct", values: readonly [Test]} | {tag: "Tuple", values: readonly [TupleStruct]} | {tag: "Enum", values: readonly [SimpleEnum]} | {tag: "Asset", values: readonly [string, i128]} | {tag: "Void", values: void};

export type TupleStruct = readonly [Test,  SimpleEnum];

export interface Client {
  /**
   * Construct and simulate a map transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  map: ({map}: {map: Map<u32, boolean>}, options?: AssembledTransactionOptions<Map<u32, boolean>>) => Promise<AssembledTransaction<Map<u32, boolean>>>

  /**
   * Construct and simulate a not transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Negates a boolean value
   */
  not: ({boolean}: {boolean: boolean}, options?: AssembledTransactionOptions<boolean>) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a val transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  val: (options?: AssembledTransactionOptions<any>) => Promise<AssembledTransaction<any>>

  /**
   * Construct and simulate a vec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  vec: ({vec}: {vec: Array<u32>}, options?: AssembledTransactionOptions<Array<u32>>) => Promise<AssembledTransaction<Array<u32>>>

  /**
   * Construct and simulate a card transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  card: ({card}: {card: RoyalCard}, options?: AssembledTransactionOptions<RoyalCard>) => Promise<AssembledTransaction<RoyalCard>>

  /**
   * Construct and simulate a i128 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  i128: ({i128}: {i128: i128}, options?: AssembledTransactionOptions<i128>) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a i256 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  i256: ({i256}: {i256: i256}, options?: AssembledTransactionOptions<i256>) => Promise<AssembledTransaction<i256>>

  /**
   * Construct and simulate a i32_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  i32_: ({i32_}: {i32_: i32}, options?: AssembledTransactionOptions<i32>) => Promise<AssembledTransaction<i32>>

  /**
   * Construct and simulate a i64_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  i64_: ({i64_}: {i64_: i64}, options?: AssembledTransactionOptions<i64>) => Promise<AssembledTransaction<i64>>

  /**
   * Construct and simulate a u128 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  u128: ({u128}: {u128: u128}, options?: AssembledTransactionOptions<u128>) => Promise<AssembledTransaction<u128>>

  /**
   * Construct and simulate a u256 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  u256: ({u256}: {u256: u256}, options?: AssembledTransactionOptions<u256>) => Promise<AssembledTransaction<u256>>

  /**
   * Construct and simulate a u32_ transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  u32_: ({u32_}: {u32_: u32}, options?: AssembledTransactionOptions<u32>) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a woid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  woid: (options?: AssembledTransactionOptions<null>) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a bytes transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bytes: ({bytes}: {bytes: Buffer}, options?: AssembledTransactionOptions<Buffer>) => Promise<AssembledTransaction<Buffer>>

  /**
   * Construct and simulate a hello transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  hello: ({hello}: {hello: string}, options?: AssembledTransactionOptions<string>) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a tuple transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  tuple: ({tuple}: {tuple: readonly [string, u32]}, options?: AssembledTransactionOptions<readonly [string, u32]>) => Promise<AssembledTransaction<readonly [string, u32]>>

  /**
   * Construct and simulate a option transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Example of an optional argument
   */
  option: ({option}: {option: Option<u32>}, options?: AssembledTransactionOptions<Option<u32>>) => Promise<AssembledTransaction<Option<u32>>>

  /**
   * Construct and simulate a simple transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  simple: ({simple}: {simple: SimpleEnum}, options?: AssembledTransactionOptions<SimpleEnum>) => Promise<AssembledTransaction<SimpleEnum>>

  /**
   * Construct and simulate a string transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  string: ({string}: {string: string}, options?: AssembledTransactionOptions<string>) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a strukt transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  strukt: ({strukt}: {strukt: Test}, options?: AssembledTransactionOptions<Test>) => Promise<AssembledTransaction<Test>>

  /**
   * Construct and simulate a boolean transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  boolean: ({boolean}: {boolean: boolean}, options?: AssembledTransactionOptions<boolean>) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a bytes_n transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bytes_n: ({bytes_n}: {bytes_n: Buffer}, options?: AssembledTransactionOptions<Buffer>) => Promise<AssembledTransaction<Buffer>>

  /**
   * Construct and simulate a complex transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  complex: ({complex}: {complex: ComplexEnum}, options?: AssembledTransactionOptions<ComplexEnum>) => Promise<AssembledTransaction<ComplexEnum>>

  /**
   * Construct and simulate a addresse transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  addresse: ({addresse}: {addresse: string}, options?: AssembledTransactionOptions<string>) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a duration transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  duration: ({duration}: {duration: Duration}, options?: AssembledTransactionOptions<Duration>) => Promise<AssembledTransaction<Duration>>

  /**
   * Construct and simulate a timepoint transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  timepoint: ({timepoint}: {timepoint: Timepoint}, options?: AssembledTransactionOptions<Timepoint>) => Promise<AssembledTransaction<Timepoint>>

  /**
   * Construct and simulate a multi_args transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  multi_args: ({a, b}: {a: u32, b: boolean}, options?: AssembledTransactionOptions<u32>) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a strukt_hel transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Example contract method which takes a struct
   */
  strukt_hel: ({strukt}: {strukt: Test}, options?: AssembledTransactionOptions<Array<string>>) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a tuple_strukt transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  tuple_strukt: ({tuple_strukt}: {tuple_strukt: TupleStruct}, options?: AssembledTransactionOptions<TupleStruct>) => Promise<AssembledTransaction<TupleStruct>>

  /**
   * Construct and simulate a u32_fail_on_even transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  u32_fail_on_even: ({u32_}: {u32_: u32}, options?: AssembledTransactionOptions<Result<u32>>) => Promise<AssembledTransaction<Result<u32>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAQAAAC9UaGlzIGlzIGZyb20gdGhlIHJ1c3QgZG9jIGFib3ZlIHRoZSBzdHJ1Y3QgVGVzdAAAAAAAAAAABFRlc3QAAAADAAAAAAAAAAFhAAAAAAAABAAAAAAAAAABYgAAAAAAAAEAAAAAAAAAAWMAAAAAAAAR",
        "AAAAAAAAAAAAAAADbWFwAAAAAAEAAAAAAAAAA21hcAAAAAPsAAAABAAAAAEAAAABAAAD7AAAAAQAAAAB",
        "AAAAAAAAABdOZWdhdGVzIGEgYm9vbGVhbiB2YWx1ZQAAAAADbm90AAAAAAEAAAAAAAAAB2Jvb2xlYW4AAAAAAQAAAAEAAAAB",
        "AAAAAAAAAAAAAAADdmFsAAAAAAAAAAABAAAAAA==",
        "AAAAAAAAAAAAAAADdmVjAAAAAAEAAAAAAAAAA3ZlYwAAAAPqAAAABAAAAAEAAAPqAAAABA==",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAAAQAAABxQbGVhc2UgcHJvdmlkZSBhbiBvZGQgbnVtYmVyAAAAD051bWJlck11c3RCZU9kZAAAAAAB",
        "AAAAAAAAAAAAAAAEY2FyZAAAAAEAAAAAAAAABGNhcmQAAAfQAAAACVJveWFsQ2FyZAAAAAAAAAEAAAfQAAAACVJveWFsQ2FyZAAAAA==",
        "AAAAAAAAAAAAAAAEaTEyOAAAAAEAAAAAAAAABGkxMjgAAAALAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAEaTI1NgAAAAEAAAAAAAAABGkyNTYAAAANAAAAAQAAAA0=",
        "AAAAAAAAAAAAAAAEaTMyXwAAAAEAAAAAAAAABGkzMl8AAAAFAAAAAQAAAAU=",
        "AAAAAAAAAAAAAAAEaTY0XwAAAAEAAAAAAAAABGk2NF8AAAAHAAAAAQAAAAc=",
        "AAAAAAAAAAAAAAAEdTEyOAAAAAEAAAAAAAAABHUxMjgAAAAKAAAAAQAAAAo=",
        "AAAAAAAAAAAAAAAEdTI1NgAAAAEAAAAAAAAABHUyNTYAAAAMAAAAAQAAAAw=",
        "AAAAAAAAAAAAAAAEdTMyXwAAAAEAAAAAAAAABHUzMl8AAAAEAAAAAQAAAAQ=",
        "AAAAAAAAAAAAAAAEd29pZAAAAAAAAAAA",
        "AAAAAAAAAAAAAAAFYnl0ZXMAAAAAAAABAAAAAAAAAAVieXRlcwAAAAAAAA4AAAABAAAADg==",
        "AAAAAAAAAAAAAAAFaGVsbG8AAAAAAAABAAAAAAAAAAVoZWxsbwAAAAAAABEAAAABAAAAEQ==",
        "AAAAAAAAAAAAAAAFdHVwbGUAAAAAAAABAAAAAAAAAAV0dXBsZQAAAAAAA+0AAAACAAAAEQAAAAQAAAABAAAD7QAAAAIAAAARAAAABA==",
        "AAAAAAAAAB9FeGFtcGxlIG9mIGFuIG9wdGlvbmFsIGFyZ3VtZW50AAAAAAZvcHRpb24AAAAAAAEAAAAAAAAABm9wdGlvbgAAAAAD6AAAAAQAAAABAAAD6AAAAAQ=",
        "AAAAAAAAAAAAAAAGc2ltcGxlAAAAAAABAAAAAAAAAAZzaW1wbGUAAAAAB9AAAAAKU2ltcGxlRW51bQAAAAAAAQAAB9AAAAAKU2ltcGxlRW51bQAA",
        "AAAAAAAAAAAAAAAGc3RyaW5nAAAAAAABAAAAAAAAAAZzdHJpbmcAAAAAABAAAAABAAAAEA==",
        "AAAAAAAAAAAAAAAGc3RydWt0AAAAAAABAAAAAAAAAAZzdHJ1a3QAAAAAB9AAAAAEVGVzdAAAAAEAAAfQAAAABFRlc3Q=",
        "AAAAAAAAAAAAAAAHYm9vbGVhbgAAAAABAAAAAAAAAAdib29sZWFuAAAAAAEAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAAHYnl0ZXNfbgAAAAABAAAAAAAAAAdieXRlc19uAAAAA+4AAAAJAAAAAQAAA+4AAAAJ",
        "AAAAAAAAAAAAAAAHY29tcGxleAAAAAABAAAAAAAAAAdjb21wbGV4AAAAB9AAAAALQ29tcGxleEVudW0AAAAAAQAAB9AAAAALQ29tcGxleEVudW0A",
        "AAAABAAAAAAAAAAAAAAACUVycm9uZW91cwAAAAAAAAEAAACsU29tZSBjb250cmFjdCBsaWJyYXJpZXMgY29udGFpbiBleHRyYSAjW2NvbnRyYWN0ZXJyb3JdIGRlZmluaXRpb25zIHRoYXQgZW5kIHVwIGNvbXBpbGVkCmludG8gdGhlIG1haW4gY29udHJhY3QgdHlwZXMuIFdlIG5lZWQgdG8gbWFrZSBzdXJlIHRvb2xpbmcgZGVhbHMgd2l0aCB0aGlzIHByb3Blcmx5LgAAAAtIb3dDb3VsZFlvdQAAAABk",
        "AAAAAwAAAAAAAAAAAAAACVJveWFsQ2FyZAAAAAAAAAMAAAAAAAAABEphY2sAAAALAAAAAAAAAAVRdWVlbgAAAAAAAAwAAAAAAAAABEtpbmcAAAAN",
        "AAAAAAAAAAAAAAAIYWRkcmVzc2UAAAABAAAAAAAAAAhhZGRyZXNzZQAAABMAAAABAAAAEw==",
        "AAAAAAAAAAAAAAAIZHVyYXRpb24AAAABAAAAAAAAAAhkdXJhdGlvbgAAAAkAAAABAAAACQ==",
        "AAAAAgAAAAAAAAAAAAAAClNpbXBsZUVudW0AAAAAAAMAAAAAAAAAAAAAAAVGaXJzdAAAAAAAAAAAAAAAAAAABlNlY29uZAAAAAAAAAAAAAAAAAAFVGhpcmQAAAA=",
        "AAAAAAAAAAAAAAAJdGltZXBvaW50AAAAAAAAAQAAAAAAAAAJdGltZXBvaW50AAAAAAAACAAAAAEAAAAI",
        "AAAAAgAAAAAAAAAAAAAAC0NvbXBsZXhFbnVtAAAAAAUAAAABAAAAAAAAAAZTdHJ1Y3QAAAAAAAEAAAfQAAAABFRlc3QAAAABAAAAAAAAAAVUdXBsZQAAAAAAAAEAAAfQAAAAC1R1cGxlU3RydWN0AAAAAAEAAAAAAAAABEVudW0AAAABAAAH0AAAAApTaW1wbGVFbnVtAAAAAAABAAAAAAAAAAVBc3NldAAAAAAAAAIAAAATAAAACwAAAAAAAAAAAAAABFZvaWQ=",
        "AAAAAQAAAAAAAAAAAAAAC1R1cGxlU3RydWN0AAAAAAIAAAAAAAAAATAAAAAAAAfQAAAABFRlc3QAAAAAAAAAATEAAAAAAAfQAAAAClNpbXBsZUVudW0AAA==",
        "AAAAAAAAAAAAAAAKbXVsdGlfYXJncwAAAAAAAgAAAAAAAAABYQAAAAAAAAQAAAAAAAAAAWIAAAAAAAABAAAAAQAAAAQ=",
        "AAAAAAAAACxFeGFtcGxlIGNvbnRyYWN0IG1ldGhvZCB3aGljaCB0YWtlcyBhIHN0cnVjdAAAAApzdHJ1a3RfaGVsAAAAAAABAAAAAAAAAAZzdHJ1a3QAAAAAB9AAAAAEVGVzdAAAAAEAAAPqAAAAEQ==",
        "AAAAAAAAAAAAAAAMdHVwbGVfc3RydWt0AAAAAQAAAAAAAAAMdHVwbGVfc3RydWt0AAAH0AAAAAtUdXBsZVN0cnVjdAAAAAABAAAH0AAAAAtUdXBsZVN0cnVjdAA=",
        "AAAAAAAAAAAAAAAQdTMyX2ZhaWxfb25fZXZlbgAAAAEAAAAAAAAABHUzMl8AAAAEAAAAAQAAA+kAAAAEAAAAAw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    map: this.txFromJSON<Map<u32, boolean>>,
        not: this.txFromJSON<boolean>,
        val: this.txFromJSON<any>,
        vec: this.txFromJSON<Array<u32>>,
        card: this.txFromJSON<RoyalCard>,
        i128: this.txFromJSON<i128>,
        i256: this.txFromJSON<i256>,
        i32_: this.txFromJSON<i32>,
        i64_: this.txFromJSON<i64>,
        u128: this.txFromJSON<u128>,
        u256: this.txFromJSON<u256>,
        u32_: this.txFromJSON<u32>,
        woid: this.txFromJSON<null>,
        bytes: this.txFromJSON<Buffer>,
        hello: this.txFromJSON<string>,
        tuple: this.txFromJSON<readonly [string, u32]>,
        option: this.txFromJSON<Option<u32>>,
        simple: this.txFromJSON<SimpleEnum>,
        string: this.txFromJSON<string>,
        strukt: this.txFromJSON<Test>,
        boolean: this.txFromJSON<boolean>,
        bytes_n: this.txFromJSON<Buffer>,
        complex: this.txFromJSON<ComplexEnum>,
        addresse: this.txFromJSON<string>,
        duration: this.txFromJSON<Duration>,
        timepoint: this.txFromJSON<Timepoint>,
        multi_args: this.txFromJSON<u32>,
        strukt_hel: this.txFromJSON<Array<string>>,
        tuple_strukt: this.txFromJSON<TupleStruct>,
        u32_fail_on_even: this.txFromJSON<Result<u32>>
  }
}