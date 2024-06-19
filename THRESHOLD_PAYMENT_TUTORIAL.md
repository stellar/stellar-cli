# Threshold Payment Tutorial

This tutorial is an example of a 2-of-2 threshold payment with the following two accounts:

```txt
keys1: {
  pk: "GCA2EYGVKZWKATWEABRKAW366IVF23BXHDXYVID7NBW2VMDFRDLNNYFZ",
  sk: "SAOEL36GXF774FX65CPUTXL4MRZRR4JCNLHJVFVYXWNX6F2UTDJGFKVO"
},
keys2: {
  pk: "GAD2NYO3IZ2FOMSRPVWYZI3TSRAYMIU2GCKMYHTOLNBZPHMDDSHX5X3R",
  sk: "SBYHNZBOVDL2AMJX3JQ73BHQPI6UVO4C3D3Q6TZMUOQWX42ZX2CHJ64J"
}
```

1. Create the “contributor_secret_key.json” file with the secret key of the contributor in base 64. This can be one of the recicipients or not. For this example we will use the secret key of the first recipient, so the content of “contributor_secret_key.json” is: ```"SAOEL36GXF774FX65CPUTXL4MRZRR4JCNLHJVFVYXWNX6F2UTDJGFKVO"```

2. Create the “recipients.json” file with the public keys of the recipients in base 64. The content is the following:

```txt
[
  "GCA2EYGVKZWKATWEABRKAW366IVF23BXHDXYVID7NBW2VMDFRDLNNYFZ",
  "GAD2NYO3IZ2FOMSRPVWYZI3TSRAYMIU2GCKMYHTOLNBZPHMDDSHX5X3R"
]
```

The files_path is the path to the folder where we stored the previous files and the threshold is 2 in this example, so we run the following command to execute round 1 of the threshold account generation:

**Input**:
```txt
soroban keys generate-threshold-round1 --threshold 2 --files cmd/soroban-cli/src/commands/keys/threshold_account
```

**Output**:
This produces the file "all_messages.json" with one message intended to all the recipients.

3. We need to run the same command again (using the same contributor or a different one, and in the same machine or a different one), because we need the same number of messages as the number of recipients for the round 2. Add all messages (2, in this case) to “all_messages.json”. The content of the file is the following:

```txt
[
  [
    129, 162, 96, 213, 86, 108, 160, 78, 196, 0, 98, 160, 91, 126, 242, 42, 93,
    108, 55, 56, 239, 138, 160, 127, 104, 109, 170, 176, 101, 136, 214, 214,
    251, 140, 41, 61, 209, 54, 173, 250, 139, 224, 12, 130, 2, 0, 2, 0, 75, 225,
    27, 225, 105, 175, 154, 113, 116, 178, 67, 173, 118, 242, 52, 219, 58, 143,
    37, 228, 61, 44, 207, 214, 119, 41, 42, 0, 15, 152, 32, 13, 231, 209, 100,
    158, 115, 66, 171, 204, 17, 14, 105, 136, 91, 107, 72, 31, 254, 59, 89, 199,
    20, 229, 19, 224, 252, 209, 112, 63, 87, 45, 138, 207, 250, 195, 81, 125,
    69, 22, 98, 169, 80, 211, 20, 4, 141, 35, 22, 210, 241, 13, 54, 238, 116,
    234, 75, 191, 143, 139, 120, 60, 104, 186, 91, 108, 114, 120, 190, 229, 31,
    204, 208, 4, 93, 255, 137, 155, 169, 164, 87, 6, 183, 151, 219, 61, 101,
    236, 0, 93, 190, 126, 228, 109, 150, 13, 8, 228, 100, 175, 196, 2, 101, 133,
    207, 184, 22, 46, 169, 144, 130, 11, 228, 7, 229, 219, 62, 74, 116, 246, 87,
    136, 61, 248, 160, 97, 128, 95, 132, 217, 148, 42, 76, 169, 229, 120, 62,
    225, 5, 254, 161, 139, 191, 40, 57, 42, 28, 236, 50, 247, 95, 86, 99, 213,
    27, 193, 109, 201, 89, 133, 163, 142, 234, 89, 221, 108, 137, 65, 255, 11,
    26, 148, 243, 20, 212, 53, 154, 3, 130, 18, 22, 72, 125, 179, 253, 50, 75,
    138, 16, 204, 39, 157, 48, 44, 132, 137, 93, 78, 133, 184, 246, 108, 104,
    236, 51, 11, 236, 171, 119, 14
  ],
  [
    129, 162, 96, 213, 86, 108, 160, 78, 196, 0, 98, 160, 91, 126, 242, 42, 93,
    108, 55, 56, 239, 138, 160, 127, 104, 109, 170, 176, 101, 136, 214, 214,
    142, 214, 164, 5, 219, 73, 47, 208, 105, 31, 195, 183, 2, 0, 2, 0, 75, 225,
    27, 225, 105, 175, 154, 113, 116, 178, 67, 173, 118, 242, 52, 219, 121, 167,
    107, 202, 190, 154, 31, 111, 195, 209, 132, 123, 83, 199, 26, 52, 128, 41,
    179, 143, 160, 187, 49, 63, 249, 168, 15, 103, 225, 36, 49, 18, 235, 22,
    180, 18, 71, 106, 153, 191, 0, 164, 100, 70, 78, 98, 153, 253, 83, 148, 129,
    237, 63, 223, 153, 98, 198, 134, 201, 204, 41, 213, 143, 23, 213, 165, 188,
    228, 117, 16, 57, 85, 121, 88, 110, 110, 16, 255, 36, 238, 206, 30, 136, 85,
    199, 225, 57, 171, 25, 143, 125, 34, 144, 138, 98, 15, 160, 101, 141, 119,
    183, 237, 23, 130, 219, 200, 205, 61, 237, 2, 242, 136, 40, 118, 224, 198,
    195, 130, 12, 95, 212, 200, 212, 131, 45, 152, 196, 1, 200, 40, 125, 193,
    85, 85, 186, 191, 145, 179, 193, 73, 60, 92, 19, 215, 143, 208, 81, 12, 179,
    145, 28, 83, 127, 189, 52, 144, 211, 53, 69, 144, 69, 175, 14, 192, 215, 85,
    90, 160, 15, 69, 205, 81, 126, 128, 112, 127, 143, 34, 0, 185, 60, 88, 100,
    199, 143, 224, 236, 129, 251, 151, 142, 59, 77, 253, 181, 199, 67, 29, 129,
    28, 27, 132, 144, 155, 163, 50, 63, 105, 60, 108, 219, 170, 113, 101, 240,
    36, 88, 241, 47, 27, 227, 239, 4, 14
  ]
]
```

4. Create the “recipient_secret_key.json” file with one of the recipients' secret key (e.g.: "SBYHNZBOVDL2AMJX3JQ73BHQPI6UVO4C3D3Q6TZMUOQWX42ZX2CHJ64J") and run the following command to execute round 2 of threshold account generation protocol:

**Input**:
```txt
soroban keys generate-threshold-round2 --files cmd/soroban-cli/src/commands/keys/threshold_account
```

**Output**:
The files “threshold_public_key.json”, "spp_output.json" and "signing_share" are created. If you run the same command for the other recipient you should get the same results, except for the signing share, which is unique (and secret) to each recipient/signer. The threshold public key is the shared public key between the recipients/signers. No one knows its corresponding secret key, but jointly they can sign with it. The spp_output contains other needed information for the execution of the signing protocol.

5. Fund the threshold account in the “threshold_public_key.json” file: txt```"GA63R6HSIYS6FQCY3HF5LJTEAVW2J5MGFE6ZF26K6MDGXYPAXU4ODNCP```

6. Run the following command for each signer with the corresponding "signing_share.json" for executing the round 1 of the threshold signing:

```txt
soroban payment sign-threshold-round1 --files cmd/soroban-cli/src/commands/keys/threshold_account
```

7. Add the content of “signing_commitments.json” of all signers (2, in this case) to “signing_commitments.json”, so that it contains one set of commitments from each signer, just like we did for the “all_messages.json”. Run the following command for each signer with the corresponding "signing_nonces.json" and "signing_share.json" for executing the round 2 of the threshold signing:

```txt
soroban payment threshold-sign-round2 \
    --source GA63R6HSIYS6FQCY3HF5LJTEAVW2J5MGFE6ZF26K6MDGXYPAXU4ODNCP \
    --network test_network \
    --destination GDEGC7Q3ZI4IVNNZX7WY7JC7AN32FF4ZJN5JS2CGVBGOPQMHSW2TLJTX \
  --amount 1000 --asset native --files cmd/soroban-cli/src/commands/keys/threshold_account
```

8. Add the content of “signing_packages.json” of all signers (2, in this case) to “signing_packages.json”, so that it contains one package from each signer, just like we did for the “all_messages.json”, and run the final command to submit the payment transaction:

```txt
soroban payment aggregate-sign --files cmd/soroban-cli/src/commands/keys/threshold_account
```
