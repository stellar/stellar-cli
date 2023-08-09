import test from 'ava';
import { Wallet } from '../../fixtures/test_custom_types/dist/esm/index.js'

function getWallet(): Wallet {
  return {
    isConnected: () => Promise.resolve(true),
    isAllowed: () => Promise.resolve(true),
    getUserInfo: () => Promise.resolve({ publicKey: 'GABC' }),
    signTransaction: (tx: string, opts?: {
      network?: string,
      networkPassphrase?: string,
      accountToSign?: string,
    }) => Promise.resolve(''),
  }
}

test('create wallet', async t => {
  const wallet = getWallet();
  t.true(await wallet.isConnected());
});