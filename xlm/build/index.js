#!/usr/bin/env node

// src/index.ts
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { Contract, rpc as SorobanRpc2 } from "@stellar/stellar-sdk";
import { z } from "zod";
import { config as dotenvConfig } from "dotenv";

// src/helper.ts
import { Address, nativeToScVal, xdr, TransactionBuilder, contract } from "@stellar/stellar-sdk";

// build/sac-sdk.js
import { Buffer } from "buffer";
import {
  Client as ContractClient,
  Spec as ContractSpec
} from "@stellar/stellar-sdk/minimal/contract";
if (typeof window !== "undefined") {
  window.Buffer = window.Buffer || Buffer;
}
var Client = class extends ContractClient {
  constructor(options) {
    super(
      new ContractSpec([
        "AAAAAAAAAYpSZXR1cm5zIHRoZSBhbGxvd2FuY2UgZm9yIGBzcGVuZGVyYCB0byB0cmFuc2ZlciBmcm9tIGBmcm9tYC4KClRoZSBhbW91bnQgcmV0dXJuZWQgaXMgdGhlIGFtb3VudCB0aGF0IHNwZW5kZXIgaXMgYWxsb3dlZCB0byB0cmFuc2ZlcgpvdXQgb2YgZnJvbSdzIGJhbGFuY2UuIFdoZW4gdGhlIHNwZW5kZXIgdHJhbnNmZXJzIGFtb3VudHMsIHRoZSBhbGxvd2FuY2UKd2lsbCBiZSByZWR1Y2VkIGJ5IHRoZSBhbW91bnQgdHJhbnNmZXJyZWQuCgojIEFyZ3VtZW50cwoKKiBgZnJvbWAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSBiYWxhbmNlIG9mIHRva2VucyB0byBiZSBkcmF3biBmcm9tLgoqIGBzcGVuZGVyYCAtIFRoZSBhZGRyZXNzIHNwZW5kaW5nIHRoZSB0b2tlbnMgaGVsZCBieSBgZnJvbWAuAAAAAAAJYWxsb3dhbmNlAAAAAAAAAgAAAAAAAAAEZnJvbQAAABMAAAAAAAAAB3NwZW5kZXIAAAAAEwAAAAEAAAAL",
        "AAAAAAAAAIlSZXR1cm5zIHRydWUgaWYgYGlkYCBpcyBhdXRob3JpemVkIHRvIHVzZSBpdHMgYmFsYW5jZS4KCiMgQXJndW1lbnRzCgoqIGBpZGAgLSBUaGUgYWRkcmVzcyBmb3Igd2hpY2ggdG9rZW4gYXV0aG9yaXphdGlvbiBpcyBiZWluZyBjaGVja2VkLgAAAAAAAAphdXRob3JpemVkAAAAAAABAAAAAAAAAAJpZAAAAAAAEwAAAAEAAAAB",
        "AAAAAAAAA59TZXQgdGhlIGFsbG93YW5jZSBieSBgYW1vdW50YCBmb3IgYHNwZW5kZXJgIHRvIHRyYW5zZmVyL2J1cm4gZnJvbQpgZnJvbWAuCgpUaGUgYW1vdW50IHNldCBpcyB0aGUgYW1vdW50IHRoYXQgc3BlbmRlciBpcyBhcHByb3ZlZCB0byB0cmFuc2ZlciBvdXQgb2YKZnJvbSdzIGJhbGFuY2UuIFRoZSBzcGVuZGVyIHdpbGwgYmUgYWxsb3dlZCB0byB0cmFuc2ZlciBhbW91bnRzLCBhbmQKd2hlbiBhbiBhbW91bnQgaXMgdHJhbnNmZXJyZWQgdGhlIGFsbG93YW5jZSB3aWxsIGJlIHJlZHVjZWQgYnkgdGhlCmFtb3VudCB0cmFuc2ZlcnJlZC4KCiMgQXJndW1lbnRzCgoqIGBmcm9tYCAtIFRoZSBhZGRyZXNzIGhvbGRpbmcgdGhlIGJhbGFuY2Ugb2YgdG9rZW5zIHRvIGJlIGRyYXduIGZyb20uCiogYHNwZW5kZXJgIC0gVGhlIGFkZHJlc3MgYmVpbmcgYXV0aG9yaXplZCB0byBzcGVuZCB0aGUgdG9rZW5zIGhlbGQgYnkKYGZyb21gLgoqIGBhbW91bnRgIC0gVGhlIHRva2VucyB0byBiZSBtYWRlIGF2YWlsYWJsZSB0byBgc3BlbmRlcmAuCiogYGV4cGlyYXRpb25fbGVkZ2VyYCAtIFRoZSBsZWRnZXIgbnVtYmVyIHdoZXJlIHRoaXMgYWxsb3dhbmNlIGV4cGlyZXMuIENhbm5vdApiZSBsZXNzIHRoYW4gdGhlIGN1cnJlbnQgbGVkZ2VyIG51bWJlciB1bmxlc3MgdGhlIGFtb3VudCBpcyBiZWluZyBzZXQgdG8gMC4KQW4gZXhwaXJlZCBlbnRyeSAod2hlcmUgZXhwaXJhdGlvbl9sZWRnZXIgPCB0aGUgY3VycmVudCBsZWRnZXIgbnVtYmVyKQpzaG91bGQgYmUgdHJlYXRlZCBhcyBhIDAgYW1vdW50IGFsbG93YW5jZS4KCiMgRXZlbnRzCgpFbWl0cyBhbiBldmVudCB3aXRoIHRvcGljcyBgWyJhcHByb3ZlIiwgZnJvbTogQWRkcmVzcywKc3BlbmRlcjogQWRkcmVzc10sIGRhdGEgPSBbYW1vdW50OiBpMTI4LCBleHBpcmF0aW9uX2xlZGdlcjogdTMyXWAAAAAAB2FwcHJvdmUAAAAABAAAAAAAAAAEZnJvbQAAABMAAAAAAAAAB3NwZW5kZXIAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAABFleHBpcmF0aW9uX2xlZGdlcgAAAAAAAAQAAAAA",
        "AAAAAAAAAJhSZXR1cm5zIHRoZSBiYWxhbmNlIG9mIGBpZGAuCgojIEFyZ3VtZW50cwoKKiBgaWRgIC0gVGhlIGFkZHJlc3MgZm9yIHdoaWNoIGEgYmFsYW5jZSBpcyBiZWluZyBxdWVyaWVkLiBJZiB0aGUKYWRkcmVzcyBoYXMgbm8gZXhpc3RpbmcgYmFsYW5jZSwgcmV0dXJucyAwLgAAAAdiYWxhbmNlAAAAAAEAAAAAAAAAAmlkAAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAWJCdXJuIGBhbW91bnRgIGZyb20gYGZyb21gLgoKUmVkdWNlcyBmcm9tJ3MgYmFsYW5jZSBieSB0aGUgYW1vdW50LCB3aXRob3V0IHRyYW5zZmVycmluZyB0aGUgYmFsYW5jZQp0byBhbm90aGVyIGhvbGRlcidzIGJhbGFuY2UuCgojIEFyZ3VtZW50cwoKKiBgZnJvbWAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSBiYWxhbmNlIG9mIHRva2VucyB3aGljaCB3aWxsIGJlCmJ1cm5lZCBmcm9tLgoqIGBhbW91bnRgIC0gVGhlIGFtb3VudCBvZiB0b2tlbnMgdG8gYmUgYnVybmVkLgoKIyBFdmVudHMKCkVtaXRzIGFuIGV2ZW50IHdpdGggdG9waWNzIGBbImJ1cm4iLCBmcm9tOiBBZGRyZXNzXSwgZGF0YSA9IGFtb3VudDoKaTEyOGAAAAAAAARidXJuAAAAAgAAAAAAAAAEZnJvbQAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAtpCdXJuIGBhbW91bnRgIGZyb20gYGZyb21gLCBjb25zdW1pbmcgdGhlIGFsbG93YW5jZSBvZiBgc3BlbmRlcmAuCgpSZWR1Y2VzIGZyb20ncyBiYWxhbmNlIGJ5IHRoZSBhbW91bnQsIHdpdGhvdXQgdHJhbnNmZXJyaW5nIHRoZSBiYWxhbmNlCnRvIGFub3RoZXIgaG9sZGVyJ3MgYmFsYW5jZS4KClRoZSBzcGVuZGVyIHdpbGwgYmUgYWxsb3dlZCB0byBidXJuIHRoZSBhbW91bnQgZnJvbSBmcm9tJ3MgYmFsYW5jZSwgaWYKdGhlIGFtb3VudCBpcyBsZXNzIHRoYW4gb3IgZXF1YWwgdG8gdGhlIGFsbG93YW5jZSB0aGF0IHRoZSBzcGVuZGVyIGhhcwpvbiB0aGUgZnJvbSdzIGJhbGFuY2UuIFRoZSBzcGVuZGVyJ3MgYWxsb3dhbmNlIG9uIGZyb20ncyBiYWxhbmNlIHdpbGwgYmUKcmVkdWNlZCBieSB0aGUgYW1vdW50LgoKIyBBcmd1bWVudHMKCiogYHNwZW5kZXJgIC0gVGhlIGFkZHJlc3MgYXV0aG9yaXppbmcgdGhlIGJ1cm4sIGFuZCBoYXZpbmcgaXRzIGFsbG93YW5jZQpjb25zdW1lZCBkdXJpbmcgdGhlIGJ1cm4uCiogYGZyb21gIC0gVGhlIGFkZHJlc3MgaG9sZGluZyB0aGUgYmFsYW5jZSBvZiB0b2tlbnMgd2hpY2ggd2lsbCBiZQpidXJuZWQgZnJvbS4KKiBgYW1vdW50YCAtIFRoZSBhbW91bnQgb2YgdG9rZW5zIHRvIGJlIGJ1cm5lZC4KCiMgRXZlbnRzCgpFbWl0cyBhbiBldmVudCB3aXRoIHRvcGljcyBgWyJidXJuIiwgZnJvbTogQWRkcmVzc10sIGRhdGEgPSBhbW91bnQ6CmkxMjhgAAAAAAAJYnVybl9mcm9tAAAAAAAAAwAAAAAAAAAHc3BlbmRlcgAAAAATAAAAAAAAAARmcm9tAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAVFDbGF3YmFjayBgYW1vdW50YCBmcm9tIGBmcm9tYCBhY2NvdW50LiBgYW1vdW50YCBpcyBidXJuZWQgaW4gdGhlCmNsYXdiYWNrIHByb2Nlc3MuCgojIEFyZ3VtZW50cwoKKiBgZnJvbWAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSBiYWxhbmNlIGZyb20gd2hpY2ggdGhlIGNsYXdiYWNrIHdpbGwKdGFrZSB0b2tlbnMuCiogYGFtb3VudGAgLSBUaGUgYW1vdW50IG9mIHRva2VucyB0byBiZSBjbGF3ZWQgYmFjay4KCiMgRXZlbnRzCgpFbWl0cyBhbiBldmVudCB3aXRoIHRvcGljcyBgWyJjbGF3YmFjayIsIGFkbWluOiBBZGRyZXNzLCB0bzogQWRkcmVzc10sCmRhdGEgPSBhbW91bnQ6IGkxMjhgAAAAAAAACGNsYXdiYWNrAAAAAgAAAAAAAAAEZnJvbQAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAIBSZXR1cm5zIHRoZSBudW1iZXIgb2YgZGVjaW1hbHMgdXNlZCB0byByZXByZXNlbnQgYW1vdW50cyBvZiB0aGlzIHRva2VuLgoKIyBQYW5pY3MKCklmIHRoZSBjb250cmFjdCBoYXMgbm90IHlldCBiZWVuIGluaXRpYWxpemVkLgAAAAhkZWNpbWFscwAAAAAAAAABAAAABA==",
        "AAAAAAAAAPNNaW50cyBgYW1vdW50YCB0byBgdG9gLgoKIyBBcmd1bWVudHMKCiogYHRvYCAtIFRoZSBhZGRyZXNzIHdoaWNoIHdpbGwgcmVjZWl2ZSB0aGUgbWludGVkIHRva2Vucy4KKiBgYW1vdW50YCAtIFRoZSBhbW91bnQgb2YgdG9rZW5zIHRvIGJlIG1pbnRlZC4KCiMgRXZlbnRzCgpFbWl0cyBhbiBldmVudCB3aXRoIHRvcGljcyBgWyJtaW50IiwgYWRtaW46IEFkZHJlc3MsIHRvOiBBZGRyZXNzXSwgZGF0YQo9IGFtb3VudDogaTEyOGAAAAAABG1pbnQAAAACAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAFlSZXR1cm5zIHRoZSBuYW1lIGZvciB0aGlzIHRva2VuLgoKIyBQYW5pY3MKCklmIHRoZSBjb250cmFjdCBoYXMgbm90IHlldCBiZWVuIGluaXRpYWxpemVkLgAAAAAAAARuYW1lAAAAAAAAAAEAAAAQ",
        "AAAAAAAAAQxTZXRzIHRoZSBhZG1pbmlzdHJhdG9yIHRvIHRoZSBzcGVjaWZpZWQgYWRkcmVzcyBgbmV3X2FkbWluYC4KCiMgQXJndW1lbnRzCgoqIGBuZXdfYWRtaW5gIC0gVGhlIGFkZHJlc3Mgd2hpY2ggd2lsbCBoZW5jZWZvcnRoIGJlIHRoZSBhZG1pbmlzdHJhdG9yCm9mIHRoaXMgdG9rZW4gY29udHJhY3QuCgojIEV2ZW50cwoKRW1pdHMgYW4gZXZlbnQgd2l0aCB0b3BpY3MgYFsic2V0X2FkbWluIiwgYWRtaW46IEFkZHJlc3NdLCBkYXRhID0KW25ld19hZG1pbjogQWRkcmVzc11gAAAACXNldF9hZG1pbgAAAAAAAAEAAAAAAAAACW5ld19hZG1pbgAAAAAAABMAAAAA",
        "AAAAAAAAAEZSZXR1cm5zIHRoZSBhZG1pbiBvZiB0aGUgY29udHJhY3QuCgojIFBhbmljcwoKSWYgdGhlIGFkbWluIGlzIG5vdCBzZXQuAAAAAAAFYWRtaW4AAAAAAAAAAAAAAQAAABM=",
        "AAAAAAAAAVBTZXRzIHdoZXRoZXIgdGhlIGFjY291bnQgaXMgYXV0aG9yaXplZCB0byB1c2UgaXRzIGJhbGFuY2UuIElmCmBhdXRob3JpemVkYCBpcyB0cnVlLCBgaWRgIHNob3VsZCBiZSBhYmxlIHRvIHVzZSBpdHMgYmFsYW5jZS4KCiMgQXJndW1lbnRzCgoqIGBpZGAgLSBUaGUgYWRkcmVzcyBiZWluZyAoZGUtKWF1dGhvcml6ZWQuCiogYGF1dGhvcml6ZWAgLSBXaGV0aGVyIG9yIG5vdCBgaWRgIGNhbiB1c2UgaXRzIGJhbGFuY2UuCgojIEV2ZW50cwoKRW1pdHMgYW4gZXZlbnQgd2l0aCB0b3BpY3MgYFsic2V0X2F1dGhvcml6ZWQiLCBpZDogQWRkcmVzc10sIGRhdGEgPQpbYXV0aG9yaXplOiBib29sXWAAAAAOc2V0X2F1dGhvcml6ZWQAAAAAAAIAAAAAAAAAAmlkAAAAAAATAAAAAAAAAAlhdXRob3JpemUAAAAAAAABAAAAAA==",
        "AAAAAAAAAFtSZXR1cm5zIHRoZSBzeW1ib2wgZm9yIHRoaXMgdG9rZW4uCgojIFBhbmljcwoKSWYgdGhlIGNvbnRyYWN0IGhhcyBub3QgeWV0IGJlZW4gaW5pdGlhbGl6ZWQuAAAAAAZzeW1ib2wAAAAAAAAAAAABAAAAEA==",
        "AAAAAAAAAWJUcmFuc2ZlciBgYW1vdW50YCBmcm9tIGBmcm9tYCB0byBgdG9gLgoKIyBBcmd1bWVudHMKCiogYGZyb21gIC0gVGhlIGFkZHJlc3MgaG9sZGluZyB0aGUgYmFsYW5jZSBvZiB0b2tlbnMgd2hpY2ggd2lsbCBiZQp3aXRoZHJhd24gZnJvbS4KKiBgdG9gIC0gVGhlIGFkZHJlc3Mgd2hpY2ggd2lsbCByZWNlaXZlIHRoZSB0cmFuc2ZlcnJlZCB0b2tlbnMuCiogYGFtb3VudGAgLSBUaGUgYW1vdW50IG9mIHRva2VucyB0byBiZSB0cmFuc2ZlcnJlZC4KCiMgRXZlbnRzCgpFbWl0cyBhbiBldmVudCB3aXRoIHRvcGljcyBgWyJ0cmFuc2ZlciIsIGZyb206IEFkZHJlc3MsIHRvOiBBZGRyZXNzXSwKZGF0YSA9IGFtb3VudDogaTEyOGAAAAAAAAh0cmFuc2ZlcgAAAAMAAAAAAAAABGZyb20AAAATAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAzFUcmFuc2ZlciBgYW1vdW50YCBmcm9tIGBmcm9tYCB0byBgdG9gLCBjb25zdW1pbmcgdGhlIGFsbG93YW5jZSB0aGF0CmBzcGVuZGVyYCBoYXMgb24gYGZyb21gJ3MgYmFsYW5jZS4gQXV0aG9yaXplZCBieSBzcGVuZGVyCihgc3BlbmRlci5yZXF1aXJlX2F1dGgoKWApLgoKVGhlIHNwZW5kZXIgd2lsbCBiZSBhbGxvd2VkIHRvIHRyYW5zZmVyIHRoZSBhbW91bnQgZnJvbSBmcm9tJ3MgYmFsYW5jZQppZiB0aGUgYW1vdW50IGlzIGxlc3MgdGhhbiBvciBlcXVhbCB0byB0aGUgYWxsb3dhbmNlIHRoYXQgdGhlIHNwZW5kZXIKaGFzIG9uIHRoZSBmcm9tJ3MgYmFsYW5jZS4gVGhlIHNwZW5kZXIncyBhbGxvd2FuY2Ugb24gZnJvbSdzIGJhbGFuY2UKd2lsbCBiZSByZWR1Y2VkIGJ5IHRoZSBhbW91bnQuCgojIEFyZ3VtZW50cwoKKiBgc3BlbmRlcmAgLSBUaGUgYWRkcmVzcyBhdXRob3JpemluZyB0aGUgdHJhbnNmZXIsIGFuZCBoYXZpbmcgaXRzCmFsbG93YW5jZSBjb25zdW1lZCBkdXJpbmcgdGhlIHRyYW5zZmVyLgoqIGBmcm9tYCAtIFRoZSBhZGRyZXNzIGhvbGRpbmcgdGhlIGJhbGFuY2Ugb2YgdG9rZW5zIHdoaWNoIHdpbGwgYmUKd2l0aGRyYXduIGZyb20uCiogYHRvYCAtIFRoZSBhZGRyZXNzIHdoaWNoIHdpbGwgcmVjZWl2ZSB0aGUgdHJhbnNmZXJyZWQgdG9rZW5zLgoqIGBhbW91bnRgIC0gVGhlIGFtb3VudCBvZiB0b2tlbnMgdG8gYmUgdHJhbnNmZXJyZWQuCgojIEV2ZW50cwoKRW1pdHMgYW4gZXZlbnQgd2l0aCB0b3BpY3MgYFsidHJhbnNmZXIiLCBmcm9tOiBBZGRyZXNzLCB0bzogQWRkcmVzc10sCmRhdGEgPSBhbW91bnQ6IGkxMjhgAAAAAAAADXRyYW5zZmVyX2Zyb20AAAAAAAAEAAAAAAAAAAdzcGVuZGVyAAAAABMAAAAAAAAABGZyb20AAAATAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA=="
      ]),
      options
    );
    this.options = options;
  }
  static async deploy(options) {
    return ContractClient.deploy(null, options);
  }
  fromJSON = {
    allowance: this.txFromJSON,
    authorized: this.txFromJSON,
    approve: this.txFromJSON,
    balance: this.txFromJSON,
    burn: this.txFromJSON,
    burn_from: this.txFromJSON,
    clawback: this.txFromJSON,
    decimals: this.txFromJSON,
    mint: this.txFromJSON,
    name: this.txFromJSON,
    set_admin: this.txFromJSON,
    admin: this.txFromJSON,
    set_authorized: this.txFromJSON,
    symbol: this.txFromJSON,
    transfer: this.txFromJSON,
    transfer_from: this.txFromJSON
  };
};

// src/helper.ts
var createSACClient = async (contractId, networkPassphrase, rpcUrl) => {
  return new Client({
    contractId,
    rpcUrl,
    networkPassphrase
  });
};

// src/index.ts
dotenvConfig();
var config = {
  network: process.env.NETWORK || "testnet",
  networkPassphrase: process.env.NETWORK_PASSPHRASE || "Test SDF Network ; September 2015",
  rpcUrl: process.env.RPC_URL || "https://soroban-testnet.stellar.org",
  contractId: process.env.CONTRACT_ID || ""
};
if (!config.contractId) {
  throw new Error("CONTRACT_ID environment variable is required");
}
var server = new SorobanRpc2.Server(config.rpcUrl);
var contract2 = new Contract(config.contractId);
var mcpServer = new McpServer({
  name: "blend-mcp-server",
  version: "1.0.0",
  capabilities: {
    resources: {},
    tools: {}
  }
});
mcpServer.tool(
  "allowance",
  "Returns the allowance for `spender` to transfer from `from`.\n\nThe amount returned is the amount that spender is allowed to transfer out of from's balance. When the spender transfers amounts, the allowance will be reduced by the amount transferred.\n\n# Arguments\n\n* `from` - The address holding the balance of tokens to be drawn from. * `spender` - The address spending the tokens held by `from`.",
  {
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    spender: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "allowance";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        from: params.from,
        spender: params.spender
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing allowance: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "authorized",
  "Returns true if `id` is authorized to use its balance.\n\n# Arguments\n\n* `id` - The address for which token authorization is being checked.",
  {
    id: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "authorized";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        id: params.id
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing authorized: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "approve",
  'Set the allowance by `amount` for `spender` to transfer/burn from `from`.\n\nThe amount set is the amount that spender is approved to transfer out of from\'s balance. The spender will be allowed to transfer amounts, and when an amount is transferred the allowance will be reduced by the amount transferred.\n\n# Arguments\n\n* `from` - The address holding the balance of tokens to be drawn from. * `spender` - The address being authorized to spend the tokens held by `from`. * `amount` - The tokens to be made available to `spender`. * `expiration_ledger` - The ledger number where this allowance expires. Cannot be less than the current ledger number unless the amount is being set to 0. An expired entry (where expiration_ledger < the current ledger number) should be treated as a 0 amount allowance.\n\n# Events\n\nEmits an event with topics `["approve", from: Address, spender: Address], data = [amount: i128, expiration_ledger: u32]`',
  {
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    spender: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)"),
    expiration_ledger: z.number().describe("Unsigned 32-bit integer (0 to 4,294,967,295) - Converts to: xdr.ScVal.scvU32(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "approve";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        from: params.from,
        spender: params.spender,
        amount: BigInt(params.amount),
        expiration_ledger: params.expiration_ledger
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing approve: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "balance",
  "Returns the balance of `id`.\n\n# Arguments\n\n* `id` - The address for which a balance is being queried. If the address has no existing balance, returns 0.",
  {
    id: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "balance";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        id: params.id
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing balance: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "burn",
  "Burn `amount` from `from`.\n\nReduces from's balance by the amount, without transferring the balance to another holder's balance.\n\n# Arguments\n\n* `from` - The address holding the balance of tokens which will be burned from. * `amount` - The amount of tokens to be burned.\n\n# Events\n\nEmits an event with topics `[\"burn\", from: Address], data = amount: i128`",
  {
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "burn";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        from: params.from,
        amount: BigInt(params.amount)
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing burn: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "burn_from",
  "Burn `amount` from `from`, consuming the allowance of `spender`.\n\nReduces from's balance by the amount, without transferring the balance to another holder's balance.\n\nThe spender will be allowed to burn the amount from from's balance, if the amount is less than or equal to the allowance that the spender has on the from's balance. The spender's allowance on from's balance will be reduced by the amount.\n\n# Arguments\n\n* `spender` - The address authorizing the burn, and having its allowance consumed during the burn. * `from` - The address holding the balance of tokens which will be burned from. * `amount` - The amount of tokens to be burned.\n\n# Events\n\nEmits an event with topics `[\"burn\", from: Address], data = amount: i128`",
  {
    spender: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "burn_from";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        spender: params.spender,
        from: params.from,
        amount: BigInt(params.amount)
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing burn_from: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "clawback",
  'Clawback `amount` from `from` account. `amount` is burned in the clawback process.\n\n# Arguments\n\n* `from` - The address holding the balance from which the clawback will take tokens. * `amount` - The amount of tokens to be clawed back.\n\n# Events\n\nEmits an event with topics `["clawback", admin: Address, to: Address], data = amount: i128`',
  {
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "clawback";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        from: params.from,
        amount: BigInt(params.amount)
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing clawback: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "decimals",
  "Returns the number of decimals used to represent amounts of this token.\n\n# Panics\n\nIf the contract has not yet been initialized.",
  {},
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "decimals";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall();
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing decimals: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "mint",
  'Mints `amount` to `to`.\n\n# Arguments\n\n* `to` - The address which will receive the minted tokens. * `amount` - The amount of tokens to be minted.\n\n# Events\n\nEmits an event with topics `["mint", admin: Address, to: Address], data = amount: i128`',
  {
    to: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "mint";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        to: params.to,
        amount: BigInt(params.amount)
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing mint: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "name",
  "Returns the name for this token.\n\n# Panics\n\nIf the contract has not yet been initialized.",
  {},
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "name";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall();
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing name: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "set_admin",
  'Sets the administrator to the specified address `new_admin`.\n\n# Arguments\n\n* `new_admin` - The address which will henceforth be the administrator of this token contract.\n\n# Events\n\nEmits an event with topics `["set_admin", admin: Address], data = [new_admin: Address]`',
  {
    new_admin: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "set_admin";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        new_admin: params.new_admin
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing set_admin: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "admin",
  "Returns the admin of the contract.\n\n# Panics\n\nIf the admin is not set.",
  {},
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "admin";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall();
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing admin: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "set_authorized",
  'Sets whether the account is authorized to use its balance. If `authorized` is true, `id` should be able to use its balance.\n\n# Arguments\n\n* `id` - The address being (de-)authorized. * `authorize` - Whether or not `id` can use its balance.\n\n# Events\n\nEmits an event with topics `["set_authorized", id: Address], data = [authorize: bool]`',
  {
    id: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    authorize: z.boolean().describe("Boolean value (true/false) - Converts to: xdr.ScVal.scvBool(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "set_authorized";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        id: params.id,
        authorize: params.authorize
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing set_authorized: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "symbol",
  "Returns the symbol for this token.\n\n# Panics\n\nIf the contract has not yet been initialized.",
  {},
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "symbol";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall();
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing symbol: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "transfer",
  'Transfer `amount` from `from` to `to`.\n\n# Arguments\n\n* `from` - The address holding the balance of tokens which will be withdrawn from. * `to` - The address which will receive the transferred tokens. * `amount` - The amount of tokens to be transferred.\n\n# Events\n\nEmits an event with topics `["transfer", from: Address, to: Address], data = amount: i128`',
  {
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    to: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "transfer";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        from: params.from,
        to: params.to,
        amount: BigInt(params.amount)
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing transfer: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
mcpServer.tool(
  "transfer_from",
  "Transfer `amount` from `from` to `to`, consuming the allowance that `spender` has on `from`'s balance. Authorized by spender (`spender.require_auth()`).\n\nThe spender will be allowed to transfer the amount from from's balance if the amount is less than or equal to the allowance that the spender has on the from's balance. The spender's allowance on from's balance will be reduced by the amount.\n\n# Arguments\n\n* `spender` - The address authorizing the transfer, and having its allowance consumed during the transfer. * `from` - The address holding the balance of tokens which will be withdrawn from. * `to` - The address which will receive the transferred tokens. * `amount` - The amount of tokens to be transferred.\n\n# Events\n\nEmits an event with topics `[\"transfer\", from: Address, to: Address], data = amount: i128`",
  {
    spender: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    from: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    to: z.string().describe("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: addressToScVal(i)"),
    amount: z.string().describe("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: i128ToScVal(i)")
  },
  async (params) => {
    try {
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);
      let txXdr;
      const functionName = "transfer_from";
      const functionToCall = sacClient[functionName];
      const result = await functionToCall({
        spender: params.spender,
        from: params.from,
        to: params.to,
        amount: BigInt(params.amount)
      });
      txXdr = result.toXDR();
      return {
        content: [
          { type: "text", text: "Unsigned Transaction XDR:" },
          { type: "text", text: txXdr },
          { type: "text", text: `<SmartContractID>${config.contractId}</SmartContractID>` },
          { type: "text", text: "Next steps:)" },
          { type: "text", text: "1. Sign the Stellar transaction XDR\n2. Submit the signed transaction XDR to the Stellar network\n3. Remember to use the smart contract ID when submitting the transaction" }
        ]
      };
    } catch (error) {
      return {
        content: [{
          type: "text",
          text: `Error executing transfer_from: ${error.message}${error.cause ? `
Cause: ${error.cause}` : ""}`
        }]
      };
    }
  }
);
async function main() {
  const transport = new StdioServerTransport();
  await mcpServer.connect(transport);
  console.error("Soroban MCP Server running on stdio");
}
main().catch((error) => {
  console.error("Fatal error in main():", error);
  process.exit(1);
});
