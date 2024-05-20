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
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __exportStar = (this && this.__exportStar) || function(m, exports) {
    for (var p in m) if (p !== "default" && !Object.prototype.hasOwnProperty.call(exports, p)) __createBinding(exports, m, p);
};
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.Client = exports.Errors = exports.RoyalCard = exports.networks = exports.rpc = exports.contract = void 0;
const buffer_1 = require("buffer");
const contract_1 = require("@stellar/stellar-sdk/contract");
__exportStar(require("@stellar/stellar-sdk"), exports);
exports.contract = __importStar(require("@stellar/stellar-sdk/contract"));
exports.rpc = __importStar(require("@stellar/stellar-sdk/rpc"));
if (typeof window !== 'undefined') {
    //@ts-ignore Buffer exists
    window.Buffer = window.Buffer || buffer_1.Buffer;
}
exports.networks = {
    futurenet: {
        networkPassphrase: "Test SDF Future Network ; October 2022",
        contractId: "CBYMYMSDF6FBDNCFJCRC7KMO4REYFPOH2U4N7FXI3GJO6YXNCQ43CDSK",
    }
};
var RoyalCard;
(function (RoyalCard) {
    RoyalCard[RoyalCard["Jack"] = 11] = "Jack";
    RoyalCard[RoyalCard["Queen"] = 12] = "Queen";
    RoyalCard[RoyalCard["King"] = 13] = "King";
})(RoyalCard || (exports.RoyalCard = RoyalCard = {}));
exports.Errors = {
    1: { message: "Please provide an odd number" }
};
class Client extends contract_1.Client {
    options;
    constructor(options) {
        super(new contract_1.Spec(["AAAAAQAAAC9UaGlzIGlzIGZyb20gdGhlIHJ1c3QgZG9jIGFib3ZlIHRoZSBzdHJ1Y3QgVGVzdAAAAAAAAAAABFRlc3QAAAADAAAAAAAAAAFhAAAAAAAABAAAAAAAAAABYgAAAAAAAAEAAAAAAAAAAWMAAAAAAAAR",
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
            "AAAAAAAAAAAAAAAMdHVwbGVfc3RydWt0AAAAAQAAAAAAAAAMdHVwbGVfc3RydWt0AAAH0AAAAAtUdXBsZVN0cnVjdAAAAAABAAAH0AAAAAtUdXBsZVN0cnVjdAA="]), options);
        this.options = options;
    }
    fromJSON = {
        hello: (this.txFromJSON),
        woid: (this.txFromJSON),
        val: (this.txFromJSON),
        u32_fail_on_even: (this.txFromJSON),
        u32_: (this.txFromJSON),
        i32_: (this.txFromJSON),
        i64_: (this.txFromJSON),
        strukt_hel: (this.txFromJSON),
        strukt: (this.txFromJSON),
        simple: (this.txFromJSON),
        complex: (this.txFromJSON),
        addresse: (this.txFromJSON),
        bytes: (this.txFromJSON),
        bytes_n: (this.txFromJSON),
        card: (this.txFromJSON),
        boolean: (this.txFromJSON),
        not: (this.txFromJSON),
        i128: (this.txFromJSON),
        u128: (this.txFromJSON),
        multi_args: (this.txFromJSON),
        map: (this.txFromJSON),
        vec: (this.txFromJSON),
        tuple: (this.txFromJSON),
        option: (this.txFromJSON),
        u256: (this.txFromJSON),
        i256: (this.txFromJSON),
        string: (this.txFromJSON),
        tuple_strukt: (this.txFromJSON)
    };
}
exports.Client = Client;
