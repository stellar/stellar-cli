import { xdr } from 'soroban-client';
export declare function strToScVal(base64Xdr: string): xdr.ScVal;
export declare function scValStrToJs<T>(base64Xdr: string): T;
export declare function scValToJs<T>(val: xdr.ScVal): T;
export declare function addressToScVal(addr: string): xdr.ScVal;
export declare function i128ToScVal(i: bigint): xdr.ScVal;
export declare function u128ToScVal(i: bigint): xdr.ScVal;
