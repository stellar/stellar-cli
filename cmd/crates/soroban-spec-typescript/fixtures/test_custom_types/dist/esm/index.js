import { xdr } from 'soroban-client';
import { Buffer } from "buffer";
import { scValStrToJs, scValToJs, addressToScVal, u128ToScVal, i128ToScVal, strToScVal } from './convert.js';
import { invoke } from './invoke.js';
export * from './constants.js';
export * from './server.js';
export * from './invoke.js';
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
const regex = /ContractError\((\d+)\)/;
function getError(err) {
    const match = err.match(regex);
    if (!match) {
        return undefined;
    }
    if (Errors == undefined) {
        return undefined;
    }
    // @ts-ignore
    let i = parseInt(match[1], 10);
    if (i < Errors.length) {
        return new Err(Errors[i]);
    }
    return undefined;
}
function TestToXdr(test) {
    if (!test) {
        return xdr.ScVal.scvVoid();
    }
    let arr = [
        new xdr.ScMapEntry({ key: ((i) => xdr.ScVal.scvSymbol(i))("a"), val: ((i) => xdr.ScVal.scvU32(i))(test["a"]) }),
        new xdr.ScMapEntry({ key: ((i) => xdr.ScVal.scvSymbol(i))("b"), val: ((i) => xdr.ScVal.scvBool(i))(test["b"]) }),
        new xdr.ScMapEntry({ key: ((i) => xdr.ScVal.scvSymbol(i))("c"), val: ((i) => xdr.ScVal.scvSymbol(i))(test["c"]) })
    ];
    return xdr.ScVal.scvMap(arr);
}
function TestFromXdr(base64Xdr) {
    let scVal = strToScVal(base64Xdr);
    let obj = scVal.map().map(e => [e.key().str(), e.val()]);
    let map = new Map(obj);
    if (!obj) {
        throw new Error('Invalid XDR');
    }
    return {
        a: scValToJs(map.get("a")),
        b: scValToJs(map.get("b")),
        c: scValToJs(map.get("c"))
    };
}
function SimpleEnumToXdr(simpleEnum) {
    if (!simpleEnum) {
        return xdr.ScVal.scvVoid();
    }
    let res = [];
    switch (simpleEnum.tag) {
        case "First":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("First"));
            break;
        case "Second":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("Second"));
            break;
        case "Third":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("Third"));
            break;
    }
    return xdr.ScVal.scvVec(res);
}
function SimpleEnumFromXdr(base64Xdr) {
    let [tag, values] = strToScVal(base64Xdr).vec().map(scValToJs);
    if (!tag) {
        throw new Error('Missing enum tag when decoding SimpleEnum from XDR');
    }
    return { tag, values };
}
export var RoyalCard;
(function (RoyalCard) {
    RoyalCard[RoyalCard["Jack"] = 11] = "Jack";
    RoyalCard[RoyalCard["Queen"] = 12] = "Queen";
    RoyalCard[RoyalCard["King"] = 13] = "King";
})(RoyalCard || (RoyalCard = {}));
function RoyalCardFromXdr(base64Xdr) {
    return scValStrToJs(base64Xdr);
}
function RoyalCardToXdr(val) {
    return xdr.ScVal.scvI32(val);
}
function TupleStructToXdr(tupleStruct) {
    if (!tupleStruct) {
        return xdr.ScVal.scvVoid();
    }
    let arr = [
        (i => TestToXdr(i))(tupleStruct[0]),
        (i => SimpleEnumToXdr(i))(tupleStruct[1])
    ];
    return xdr.ScVal.scvVec(arr);
}
function TupleStructFromXdr(base64Xdr) {
    return scValStrToJs(base64Xdr);
}
function ComplexEnumToXdr(complexEnum) {
    if (!complexEnum) {
        return xdr.ScVal.scvVoid();
    }
    let res = [];
    switch (complexEnum.tag) {
        case "Struct":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("Struct"));
            res.push(((i) => TestToXdr(i))(complexEnum.values[0]));
            break;
        case "Tuple":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("Tuple"));
            res.push(((i) => TupleStructToXdr(i))(complexEnum.values[0]));
            break;
        case "Enum":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("Enum"));
            res.push(((i) => SimpleEnumToXdr(i))(complexEnum.values[0]));
            break;
        case "Void":
            res.push(((i) => xdr.ScVal.scvSymbol(i))("Void"));
            break;
    }
    return xdr.ScVal.scvVec(res);
}
function ComplexEnumFromXdr(base64Xdr) {
    let [tag, values] = strToScVal(base64Xdr).vec().map(scValToJs);
    if (!tag) {
        throw new Error('Missing enum tag when decoding ComplexEnum from XDR');
    }
    return { tag, values };
}
const Errors = [
    { message: "Unknown error has occured" }
];
export async function hello({ hello }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'hello',
        args: [((i) => xdr.ScVal.scvSymbol(i))(hello)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function woid({ signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'woid',
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return;
}
export async function val({ signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'val',
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function u32_fail_on_even({ u32_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u32_fail_on_even',
        args: [((i) => xdr.ScVal.scvU32(i))(u32_)],
    };
    try {
        // @ts-ignore Type does exist
        const response = await invoke(invokeArgs);
        return new Ok(scValStrToJs(response.xdr));
    }
    catch (e) {
        //@ts-ignore
        let err = getError(e.message);
        if (err) {
            return err;
        }
        else {
            throw e;
        }
    }
}
export async function u32_({ u32_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u32_',
        args: [((i) => xdr.ScVal.scvU32(i))(u32_)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function i32_({ i32_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i32_',
        args: [((i) => xdr.ScVal.scvI32(i))(i32_)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function i64_({ i64_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i64_',
        args: [((i) => xdr.ScVal.scvI64(xdr.Int64.fromString(i.toString())))(i64_)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
/**
 * Example contract method which takes a struct
 */
export async function strukt_hel({ strukt }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'strukt_hel',
        args: [((i) => TestToXdr(i))(strukt)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function strukt({ strukt }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'strukt',
        args: [((i) => TestToXdr(i))(strukt)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return TestFromXdr(response.xdr);
}
export async function simple({ simple }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'simple',
        args: [((i) => SimpleEnumToXdr(i))(simple)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return SimpleEnumFromXdr(response.xdr);
}
export async function complex({ complex }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'complex',
        args: [((i) => ComplexEnumToXdr(i))(complex)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return ComplexEnumFromXdr(response.xdr);
}
export async function addresse({ addresse }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'addresse',
        args: [((i) => addressToScVal(i))(addresse)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function bytes({ bytes }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'bytes',
        args: [((i) => xdr.ScVal.scvBytes(i))(bytes)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function bytes_n({ bytes_n }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'bytes_n',
        args: [((i) => xdr.ScVal.scvBytes(i))(bytes_n)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function card({ card }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'card',
        args: [((i) => RoyalCardToXdr(i))(card)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return RoyalCardFromXdr(response.xdr);
}
export async function boolean({ boolean }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'boolean',
        args: [((i) => xdr.ScVal.scvBool(i))(boolean)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
/**
 * Negates a boolean value
 */
export async function not({ boolean }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'not',
        args: [((i) => xdr.ScVal.scvBool(i))(boolean)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function i128({ i128 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i128',
        args: [((i) => i128ToScVal(i))(i128)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function u128({ u128 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u128',
        args: [((i) => u128ToScVal(i))(u128)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function multi_args({ a, b }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'multi_args',
        args: [((i) => xdr.ScVal.scvU32(i))(a),
            ((i) => xdr.ScVal.scvBool(i))(b)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function map({ map }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'map',
        args: [((i) => xdr.ScVal.scvMap(Array.from(i.entries()).map(([key, value]) => {
                return new xdr.ScMapEntry({
                    key: ((i) => xdr.ScVal.scvU32(i))(key),
                    val: ((i) => xdr.ScVal.scvBool(i))(value)
                });
            })))(map)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function vec({ vec }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'vec',
        args: [((i) => xdr.ScVal.scvVec(i.map((i) => xdr.ScVal.scvU32(i))))(vec)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function tuple({ tuple }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'tuple',
        args: [((i) => xdr.ScVal.scvVec([((i) => xdr.ScVal.scvSymbol(i))(i[0]),
                ((i) => xdr.ScVal.scvU32(i))(i[1])]))(tuple)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
/**
 * Example of an optional argument
 */
export async function option({ option }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'option',
        args: [((i) => (!i) ? xdr.ScVal.scvVoid() : xdr.ScVal.scvU32(i))(option)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function u256({ u256 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u256',
        args: [((i) => i)(u256)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function i256({ i256 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i256',
        args: [((i) => i)(i256)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function string({ string }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'string',
        args: [((i) => xdr.ScVal.scvString(i))(string)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return scValStrToJs(response.xdr);
}
export async function tuple_strukt({ tuple_strukt }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'tuple_strukt',
        args: [((i) => TupleStructToXdr(i))(tuple_strukt)],
    };
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return TupleStructFromXdr(response.xdr);
}
