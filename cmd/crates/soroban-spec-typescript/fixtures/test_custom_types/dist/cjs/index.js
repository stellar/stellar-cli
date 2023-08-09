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
exports.Contract = exports.RoyalCard = exports.Err = exports.Ok = void 0;
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
        case "Asset":
            res.push(((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))("Asset"));
            res.push(((i) => (0, convert_js_1.addressToScVal)(i))(complexEnum.values[0]));
            res.push(((i) => (0, convert_js_1.i128ToScVal)(i))(complexEnum.values[1]));
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
    { message: "Unknown error has occurred" }
];
class Contract {
    options;
    constructor(options) {
        this.options = options;
    }
    async hello({ hello }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'hello',
            args: [((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))(hello)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async woid(options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'woid',
            ...options,
            ...this.options,
            parseResultXdr: () => { },
        });
    }
    async val(options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'val',
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async u32FailOnEven({ u32_ }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'u32_fail_on_even',
            args: [((i) => soroban_client_1.xdr.ScVal.scvU32(i))(u32_)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                try {
                    return new Ok((0, convert_js_1.scValStrToJs)(xdr));
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
    async u32({ u32_ }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'u32_',
            args: [((i) => soroban_client_1.xdr.ScVal.scvU32(i))(u32_)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async i32({ i32_ }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'i32_',
            args: [((i) => soroban_client_1.xdr.ScVal.scvI32(i))(i32_)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async i64({ i64_ }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'i64_',
            args: [((i) => soroban_client_1.xdr.ScVal.scvI64(soroban_client_1.xdr.Int64.fromString(i.toString())))(i64_)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    /**
 * Example contract method which takes a struct
 */
    async struktHel({ strukt }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'strukt_hel',
            args: [((i) => TestToXdr(i))(strukt)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async strukt({ strukt }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'strukt',
            args: [((i) => TestToXdr(i))(strukt)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return TestFromXdr(xdr);
            },
        });
    }
    async simple({ simple }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'simple',
            args: [((i) => SimpleEnumToXdr(i))(simple)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return SimpleEnumFromXdr(xdr);
            },
        });
    }
    async complex({ complex }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'complex',
            args: [((i) => ComplexEnumToXdr(i))(complex)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return ComplexEnumFromXdr(xdr);
            },
        });
    }
    async addresse({ addresse }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'addresse',
            args: [((i) => (0, convert_js_1.addressToScVal)(i))(addresse)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async bytes({ bytes }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'bytes',
            args: [((i) => soroban_client_1.xdr.ScVal.scvBytes(i))(bytes)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async bytesN({ bytes_n }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'bytes_n',
            args: [((i) => soroban_client_1.xdr.ScVal.scvBytes(i))(bytes_n)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async card({ card }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'card',
            args: [((i) => RoyalCardToXdr(i))(card)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return RoyalCardFromXdr(xdr);
            },
        });
    }
    async booleanMethod({ boolean }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'boolean',
            args: [((i) => soroban_client_1.xdr.ScVal.scvBool(i))(boolean)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    /**
 * Negates a boolean value
 */
    async not({ boolean }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'not',
            args: [((i) => soroban_client_1.xdr.ScVal.scvBool(i))(boolean)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async i128({ i128 }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'i128',
            args: [((i) => (0, convert_js_1.i128ToScVal)(i))(i128)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async u128({ u128 }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'u128',
            args: [((i) => (0, convert_js_1.u128ToScVal)(i))(u128)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async multiArgs({ a, b }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'multi_args',
            args: [((i) => soroban_client_1.xdr.ScVal.scvU32(i))(a),
                ((i) => soroban_client_1.xdr.ScVal.scvBool(i))(b)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async map({ map }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'map',
            args: [((i) => soroban_client_1.xdr.ScVal.scvMap(Array.from(i.entries()).map(([key, value]) => {
                    return new soroban_client_1.xdr.ScMapEntry({
                        key: ((i) => soroban_client_1.xdr.ScVal.scvU32(i))(key),
                        val: ((i) => soroban_client_1.xdr.ScVal.scvBool(i))(value)
                    });
                })))(map)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async vec({ vec }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'vec',
            args: [((i) => soroban_client_1.xdr.ScVal.scvVec(i.map((i) => soroban_client_1.xdr.ScVal.scvU32(i))))(vec)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async tuple({ tuple }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'tuple',
            args: [((i) => soroban_client_1.xdr.ScVal.scvVec([((i) => soroban_client_1.xdr.ScVal.scvSymbol(i))(i[0]),
                    ((i) => soroban_client_1.xdr.ScVal.scvU32(i))(i[1])]))(tuple)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    /**
 * Example of an optional argument
 */
    async option({ option }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'option',
            args: [((i) => (!i) ? soroban_client_1.xdr.ScVal.scvVoid() : soroban_client_1.xdr.ScVal.scvU32(i))(option)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async u256({ u256 }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'u256',
            args: [((i) => i)(u256)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async i256({ i256 }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'i256',
            args: [((i) => i)(i256)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async string({ string }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'string',
            args: [((i) => soroban_client_1.xdr.ScVal.scvString(i))(string)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return (0, convert_js_1.scValStrToJs)(xdr);
            },
        });
    }
    async tupleStrukt({ tuple_strukt }, options = {}) {
        return await (0, invoke_js_1.invoke)({
            method: 'tuple_strukt',
            args: [((i) => TupleStructToXdr(i))(tuple_strukt)],
            ...options,
            ...this.options,
            parseResultXdr: (xdr) => {
                return TupleStructFromXdr(xdr);
            },
        });
    }
}
exports.Contract = Contract;
