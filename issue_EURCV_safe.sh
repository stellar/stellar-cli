#!/bin/sh

cargo install stellar-cli \
    --debug \
    --git https://github.com/stellar/stellar-cli \
    --branch feat/tx_add_op \
    --locked

set -e

export STELLAR_NETWORK=testnet

stellar keys generate --fund sg-issuer || true
stellar keys generate --fund sg-operator || true

ISSUER_PK=$(stellar keys address sg-issuer)
OPERATOR_PK=$(stellar keys address sg-operator)

ASSET="EURCV:$ISSUER_PK"
LIMIT=1000000000

# Example of issuing an asset
stellar tx new set-options --fee 1000 --source sg-issuer --set-clawback-enabled --set-revocable --build-only \
| stellar tx op add change-trust --op-source sg-operator --line $ASSET \
| stellar tx op add payment --destination $OPERATOR_PK --asset $ASSET --amount 1 \
| stellar tx op add set-trustline-flags --asset $ASSET --trustor $OPERATOR_PK --clear-authorize \
| stellar tx sign --sign-with-key sg-issuer \
| stellar tx sign --sign-with-key sg-operator \
| stellar tx send

# Example of sending a payment with sandwich transaction
stellar tx new set-trustline-flags --fee 1000 --build-only --source sg-issuer --asset $ASSET --trustor $OPERATOR_PK --set-authorize \
| stellar tx op add payment --destination $OPERATOR_PK --asset $ASSET --amount 1000000000000 \
| stellar tx op add set-trustline-flags --asset $ASSET --trustor $OPERATOR_PK --clear-authorize \
| stellar tx sign --sign-with-key sg-issuer \
| stellar tx send

stellar keys generate --fund sg-user || true
USER_PK=$(stellar keys address sg-user)

# User adds trustline
stellar tx new change-trust --source sg-user --line $ASSET

# Send user funds from operator
stellar tx new set-trustline-flags --fee 1000 --build-only --source sg-issuer --asset $ASSET --trustor $OPERATOR_PK --set-authorize \
| stellar tx op add payment --op-source sg-operator --destination $USER_PK --asset $ASSET --amount 10000000000 \
| stellar tx op add set-trustline-flags --asset $ASSET --trustor $OPERATOR_PK --clear-authorize \
| stellar tx sign --sign-with-key sg-issuer \
| stellar tx sign --sign-with-key sg-operator \
| stellar tx send

# Next user creates a claimable balance, currently only supports unconditional claim predicate
stellar tx new create-claimable-balance --source sg-user --asset $ASSET --amount 500 \
                                        --cliamants $OPERATOR_PK \
                                        --claimant-amount $USER_PK

# Need a way to look up the th balance id like
# https://horizon-testnet.stellar.org/claimable_balances/?sponsor=GCT2HCTMAPCRP33MAKFY5OISQV52CHYTDFYXKUNG3I7IZIECLV5BMTUQ&claimant=GAE3ZGWQF3WJLFPRDCSSIOWKN4IZFTWGCQAQPR5PVOEVS3TIP752NNPZ

# Then can claim the balance with the following transaction
# stellar tx new set-trustline-flags --fee 1000 --build-only --source sg-issuer --asset $ASSET --trustor $OPERATOR_PK --set-authorize \
# | stellar tx op add claim-claimable-balance --op-source sg-operator --balance-id bce769414798ff660c9535945febbecaf4995a2d9a045900404d5ad82ddc24fa  \
# | stellar tx op add set-trustline-flags --asset $ASSET --trustor $OPERATOR_PK --clear-authorize \
# | stellar tx sign --sign-with-key sg-issuer \
# | stellar tx sign --sign-with-key sg-operator \
# | stellar tx send




stellar
