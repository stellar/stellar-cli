"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __exportStar = (this && this.__exportStar) || function(m, exports) {
    for (var p in m) if (p !== "default" && !Object.prototype.hasOwnProperty.call(exports, p)) __createBinding(exports, m, p);
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.tuple_strukt = exports.string = exports.i256 = exports.u256 = exports.option = exports.tuple = exports.vec = exports.map = exports.multi_args = exports.u128 = exports.i128 = exports.not = exports.boolean = exports.card = exports.bytes_n = exports.bytes = exports.addresse = exports.complex = exports.simple = exports.strukt = exports.strukt_hel = exports.i64_ = exports.i32_ = exports.u32_ = exports.u32_fail_on_even = exports.val = exports.woid = exports.hello = exports.RoyalCard = exports.Err = exports.Ok = void 0;
const soroban_client_1 = require("soroban-client");
const buffer_1 = require("buffer");
const convert_js_1 = require("./convert.js");
const invoke_js_1 = require("./invoke.js");
__exportStar(require("./constants.js"), exports);
__exportStar(require("./server.js"), exports);
__exportStar(require("./invoke.js"), exports);
;
;
class Ok {
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
exports.Ok = Ok;
class Err {
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
exports.Err = Err;
if (typeof window !== 'undefined') {
    //@ts-ignore Buffer exists
    window.Buffer = window.Buffer || buffer_1.Buffer;
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
        return soroban_client_1.xdr.ScVal.scvVoid();
    }
    let arr = [
        new soroban_client_1.xdr.ScMapEntry({ key: ((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("a"), val: ((i) => soroban_client_1.xdr.ScVal.scvU32(i))(test["a"]) }),
        new soroban_client_1.xdr.ScMapEntry({ key: ((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("b"), val: ((i) => soroban_client_1.xdr.ScVal.scvBool(i))(test["b"]) }),
        new soroban_client_1.xdr.ScMapEntry({ key: ((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("c"), val: ((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))(test["c"]) })
    ];
    return soroban_client_1.xdr.ScVal.scvMap(arr);
}
function TestFromXdr(base64Xdr) {
    let scVal = (0, convert_js_1.strToScVal)(base64Xdr);
    let obj = scVal.map().map(e => [e.key().str(), e.val()]);
    let map = new Map(obj);
    if (!obj) {
        throw new Error('Invalid XDR');
    }
    return {
        a: (0, convert_js_1.scValToJs)(map.get("a")),
        b: (0, convert_js_1.scValToJs)(map.get("b")),
        c: (0, convert_js_1.scValToJs)(map.get("c"))
    };
}
function SimpleEnumToXdr(simpleEnum) {
    if (!simpleEnum) {
        return soroban_client_1.xdr.ScVal.scvVoid();
    }
    let res = [];
    switch (simpleEnum.tag) {
        case "First":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("First"));
            break;
        case "Second":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Second"));
            break;
        case "Third":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Third"));
            break;
    }
    return soroban_client_1.xdr.ScVal.scvVec(res);
}
function SimpleEnumFromXdr(base64Xdr) {
    let [tag, values] = (0, convert_js_1.strToScVal)(base64Xdr).vec().map(convert_js_1.scValToJs);
    if (!tag) {
        throw new Error('Missing enum tag when decoding SimpleEnum from XDR');
    }
    return { tag, values };
}
var RoyalCard;
(function (RoyalCard) {
    RoyalCard[RoyalCard["Jack"] = 11] = "Jack";
    RoyalCard[RoyalCard["Queen"] = 12] = "Queen";
    RoyalCard[RoyalCard["King"] = 13] = "King";
})(RoyalCard || (exports.RoyalCard = RoyalCard = {}));
function RoyalCardFromXdr(base64Xdr) {
    return (0, convert_js_1.scValStrToJs)(base64Xdr);
}
function RoyalCardToXdr(val) {
    return soroban_client_1.xdr.ScVal.scvI32(val);
}
function TupleStructToXdr(tupleStruct) {
    if (!tupleStruct) {
        return soroban_client_1.xdr.ScVal.scvVoid();
    }
    let arr = [
        (i => TestToXdr(i))(tupleStruct[0]),
        (i => SimpleEnumToXdr(i))(tupleStruct[1])
    ];
    return soroban_client_1.xdr.ScVal.scvVec(arr);
}
function TupleStructFromXdr(base64Xdr) {
    return (0, convert_js_1.scValStrToJs)(base64Xdr);
}
function ComplexEnumToXdr(complexEnum) {
    if (!complexEnum) {
        return soroban_client_1.xdr.ScVal.scvVoid();
    }
    let res = [];
    switch (complexEnum.tag) {
        case "Struct":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Struct"));
            res.push(((i) => TestToXdr(i))(complexEnum.values[0]));
            break;
        case "Tuple":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Tuple"));
            res.push(((i) => TupleStructToXdr(i))(complexEnum.values[0]));
            break;
        case "Enum":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Enum"));
            res.push(((i) => SimpleEnumToXdr(i))(complexEnum.values[0]));
            break;
        case "Void":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Void"));
            break;
    }
    return soroban_client_1.xdr.ScVal.scvVec(res);
}
function ComplexEnumFromXdr(base64Xdr) {
    let [tag, values] = (0, convert_js_1.strToScVal)(base64Xdr).vec().map(convert_js_1.scValToJs);
    if (!tag) {
        throw new Error('Missing enum tag when decoding ComplexEnum from XDR');
    }
    return { tag, values };
}
const Errors = [
    { message: "Unknown error has occured" }
];
async function hello({ hello }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'hello',
        args: [((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))(hello)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.hello = hello;
async function woid({ signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'woid',
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return;
}
exports.woid = woid;
async function val({ signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'val',
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.val = val;
async function u32_fail_on_even({ u32_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u32_fail_on_even',
        args: [((i) => soroban_client_1.xdr.ScVal.scvU32(i))(u32_)],
    };
    try {
        // @ts-ignore Type does exist
        const response = await (0, invoke_js_1.invoke)(invokeArgs);
        return new Ok((0, convert_js_1.scValStrToJs)(response.xdr));
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
exports.u32_fail_on_even = u32_fail_on_even;
async function u32_({ u32_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u32_',
        args: [((i) => soroban_client_1.xdr.ScVal.scvU32(i))(u32_)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.u32_ = u32_;
async function i32_({ i32_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i32_',
        args: [((i) => soroban_client_1.xdr.ScVal.scvI32(i))(i32_)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.i32_ = i32_;
async function i64_({ i64_ }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i64_',
        args: [((i) => soroban_client_1.xdr.ScVal.scvI64(soroban_client_1.xdr.Int64.fromString(i.toString())))(i64_)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.i64_ = i64_;
/**
 * Example contract method which takes a struct
 */
async function strukt_hel({ strukt }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'strukt_hel',
        args: [((i) => TestToXdr(i))(strukt)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.strukt_hel = strukt_hel;
async function strukt({ strukt }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'strukt',
        args: [((i) => TestToXdr(i))(strukt)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return TestFromXdr(response.xdr);
}
exports.strukt = strukt;
async function simple({ simple }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'simple',
        args: [((i) => SimpleEnumToXdr(i))(simple)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return SimpleEnumFromXdr(response.xdr);
}
exports.simple = simple;
async function complex({ complex }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'complex',
        args: [((i) => ComplexEnumToXdr(i))(complex)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return ComplexEnumFromXdr(response.xdr);
}
exports.complex = complex;
async function addresse({ addresse }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'addresse',
        args: [((i) => (0, convert_js_1.addressToScVal)(i))(addresse)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.addresse = addresse;
async function bytes({ bytes }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'bytes',
        args: [((i) => soroban_client_1.xdr.ScVal.scvBytes(i))(bytes)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.bytes = bytes;
async function bytes_n({ bytes_n }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'bytes_n',
        args: [((i) => soroban_client_1.xdr.ScVal.scvBytes(i))(bytes_n)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.bytes_n = bytes_n;
async function card({ card }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'card',
        args: [((i) => RoyalCardToXdr(i))(card)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return RoyalCardFromXdr(response.xdr);
}
exports.card = card;
async function boolean({ boolean }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'boolean',
        args: [((i) => soroban_client_1.xdr.ScVal.scvBool(i))(boolean)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.boolean = boolean;
/**
 * Negates a boolean value
 */
async function not({ boolean }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'not',
        args: [((i) => soroban_client_1.xdr.ScVal.scvBool(i))(boolean)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.not = not;
async function i128({ i128 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i128',
        args: [((i) => (0, convert_js_1.i128ToScVal)(i))(i128)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.i128 = i128;
async function u128({ u128 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u128',
        args: [((i) => (0, convert_js_1.u128ToScVal)(i))(u128)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.u128 = u128;
async function multi_args({ a, b }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'multi_args',
        args: [((i) => soroban_client_1.xdr.ScVal.scvU32(i))(a),
            ((i) => soroban_client_1.xdr.ScVal.scvBool(i))(b)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.multi_args = multi_args;
async function map({ map }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'map',
        args: [((i) => soroban_client_1.xdr.ScVal.scvMap(Array.from(i.entries()).map(([key, value]) => {
                return new soroban_client_1.xdr.ScMapEntry({
                    key: ((i) => soroban_client_1.xdr.ScVal.scvU32(i))(key),
                    val: ((i) => soroban_client_1.xdr.ScVal.scvBool(i))(value)
                });
            })))(map)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.map = map;
async function vec({ vec }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'vec',
        args: [((i) => soroban_client_1.xdr.ScVal.scvVec(i.map((i) => soroban_client_1.xdr.ScVal.scvU32(i))))(vec)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.vec = vec;
async function tuple({ tuple }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'tuple',
        args: [((i) => soroban_client_1.xdr.ScVal.scvVec([((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))(i[0]),
                ((i) => soroban_client_1.xdr.ScVal.scvU32(i))(i[1])]))(tuple)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.tuple = tuple;
/**
 * Example of an optional argument
 */
async function option({ option }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'option',
        args: [((i) => (!i) ? soroban_client_1.xdr.ScVal.scvVoid() : soroban_client_1.xdr.ScVal.scvU32(i))(option)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.option = option;
async function u256({ u256 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'u256',
        args: [((i) => i)(u256)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.u256 = u256;
async function i256({ i256 }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'i256',
        args: [((i) => i)(i256)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.i256 = i256;
async function string({ string }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'string',
        args: [((i) => soroban_client_1.xdr.ScVal.scvString(i))(string)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return (0, convert_js_1.scValStrToJs)(response.xdr);
}
exports.string = string;
async function tuple_strukt({ tuple_strukt }, { signAndSend, fee } = { signAndSend: false, fee: 100 }) {
    let invokeArgs = {
        signAndSend,
        fee,
        method: 'tuple_strukt',
        args: [((i) => TupleStructToXdr(i))(tuple_strukt)],
    };
    // @ts-ignore Type does exist
    const response = await (0, invoke_js_1.invoke)(invokeArgs);
    return TupleStructFromXdr(response.xdr);
}
exports.tuple_strukt = tuple_strukt;
