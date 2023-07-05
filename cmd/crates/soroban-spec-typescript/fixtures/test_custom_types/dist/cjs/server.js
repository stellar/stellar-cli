"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Server = void 0;
const SorobanClient = require("soroban-client");
const constants_js_1 = require("./constants.js");
/**
 * SorobanClient.Server instance, initialized using {@link RPC_URL} used to
 * initialize this library.
 */
exports.Server = new SorobanClient.Server(constants_js_1.RPC_URL, { allowHttp: constants_js_1.RPC_URL.startsWith('http://') });
