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
export async function hello({ hello }, options = {}) {
    return await invoke({
        method: 'hello',
        args: [((i) => xdr.ScVal.scvSymbol(i))(hello)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function woid(options = {}) {
    return await invoke({
        method: 'woid',
        ...options,
        parseResultXdr: () => { },
    });
}
export async function val(options = {}) {
    return await invoke({
        method: 'val',
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function u32FailOnEven({ u32_ }, options = {}) {
    return await invoke({
        method: 'u32_fail_on_even',
        args: [((i) => xdr.ScVal.scvU32(i))(u32_)],
        ...options,
        parseResultXdr: (xdr) => {
            try {
                return new Ok(scValStrToJs(xdr));
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
        },
    });
}
export async function u32({ u32_ }, options = {}) {
    return await invoke({
        method: 'u32_',
        args: [((i) => xdr.ScVal.scvU32(i))(u32_)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function i32({ i32_ }, options = {}) {
    return await invoke({
        method: 'i32_',
        args: [((i) => xdr.ScVal.scvI32(i))(i32_)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function i64({ i64_ }, options = {}) {
    return await invoke({
        method: 'i64_',
        args: [((i) => xdr.ScVal.scvI64(xdr.Int64.fromString(i.toString())))(i64_)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
/**
 * Example contract method which takes a struct
 */
export async function struktHel({ strukt }, options = {}) {
    return await invoke({
        method: 'strukt_hel',
        args: [((i) => TestToXdr(i))(strukt)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function strukt({ strukt }, options = {}) {
    return await invoke({
        method: 'strukt',
        args: [((i) => TestToXdr(i))(strukt)],
        ...options,
        parseResultXdr: (xdr) => {
            return TestFromXdr(xdr);
        },
    });
}
export async function simple({ simple }, options = {}) {
    return await invoke({
        method: 'simple',
        args: [((i) => SimpleEnumToXdr(i))(simple)],
        ...options,
        parseResultXdr: (xdr) => {
            return SimpleEnumFromXdr(xdr);
        },
    });
}
export async function complex({ complex }, options = {}) {
    return await invoke({
        method: 'complex',
        args: [((i) => ComplexEnumToXdr(i))(complex)],
        ...options,
        parseResultXdr: (xdr) => {
            return ComplexEnumFromXdr(xdr);
        },
    });
}
export async function addresse({ addresse }, options = {}) {
    return await invoke({
        method: 'addresse',
        args: [((i) => addressToScVal(i))(addresse)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function bytes({ bytes }, options = {}) {
    return await invoke({
        method: 'bytes',
        args: [((i) => xdr.ScVal.scvBytes(i))(bytes)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function bytesN({ bytes_n }, options = {}) {
    return await invoke({
        method: 'bytes_n',
        args: [((i) => xdr.ScVal.scvBytes(i))(bytes_n)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function card({ card }, options = {}) {
    return await invoke({
        method: 'card',
        args: [((i) => RoyalCardToXdr(i))(card)],
        ...options,
        parseResultXdr: (xdr) => {
            return RoyalCardFromXdr(xdr);
        },
    });
}
export async function booleanMethod({ boolean }, options = {}) {
    return await invoke({
        method: 'boolean',
        args: [((i) => xdr.ScVal.scvBool(i))(boolean)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
/**
 * Negates a boolean value
 */
export async function not({ boolean }, options = {}) {
    return await invoke({
        method: 'not',
        args: [((i) => xdr.ScVal.scvBool(i))(boolean)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function i128({ i128 }, options = {}) {
    return await invoke({
        method: 'i128',
        args: [((i) => i128ToScVal(i))(i128)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function u128({ u128 }, options = {}) {
    return await invoke({
        method: 'u128',
        args: [((i) => u128ToScVal(i))(u128)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function multiArgs({ a, b }, options = {}) {
    return await invoke({
        method: 'multi_args',
        args: [((i) => xdr.ScVal.scvU32(i))(a),
            ((i) => xdr.ScVal.scvBool(i))(b)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function map({ map }, options = {}) {
    return await invoke({
        method: 'map',
        args: [((i) => xdr.ScVal.scvMap(Array.from(i.entries()).map(([key, value]) => {
                return new xdr.ScMapEntry({
                    key: ((i) => xdr.ScVal.scvU32(i))(key),
                    val: ((i) => xdr.ScVal.scvBool(i))(value)
                });
            })))(map)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function vec({ vec }, options = {}) {
    return await invoke({
        method: 'vec',
        args: [((i) => xdr.ScVal.scvVec(i.map((i) => xdr.ScVal.scvU32(i))))(vec)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function tuple({ tuple }, options = {}) {
    return await invoke({
        method: 'tuple',
        args: [((i) => xdr.ScVal.scvVec([((i) => xdr.ScVal.scvSymbol(i))(i[0]),
                ((i) => xdr.ScVal.scvU32(i))(i[1])]))(tuple)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
/**
 * Example of an optional argument
 */
export async function option({ option }, options = {}) {
    return await invoke({
        method: 'option',
        args: [((i) => (!i) ? xdr.ScVal.scvVoid() : xdr.ScVal.scvU32(i))(option)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function u256({ u256 }, options = {}) {
    return await invoke({
        method: 'u256',
        args: [((i) => i)(u256)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function i256({ i256 }, options = {}) {
    return await invoke({
        method: 'i256',
        args: [((i) => i)(i256)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function string({ string }, options = {}) {
    return await invoke({
        method: 'string',
        args: [((i) => xdr.ScVal.scvString(i))(string)],
        ...options,
        parseResultXdr: (xdr) => {
            return scValStrToJs(xdr);
        },
    });
}
export async function tupleStrukt({ tuple_strukt }, options = {}) {
    return await invoke({
        method: 'tuple_strukt',
        args: [((i) => TupleStructToXdr(i))(tuple_strukt)],
        ...options,
        parseResultXdr: (xdr) => {
            return TupleStructFromXdr(xdr);
        },
    });
}
