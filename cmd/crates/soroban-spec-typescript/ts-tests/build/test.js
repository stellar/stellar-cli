import test from 'ava';
const networkPassphrases = {
    standalone: 'Standalone Network ; February 2017',
    futurenet: 'Test SDF Future Network ; October 2022',
};
function getWallet(network) {
    return {
        isConnected: () => Promise.resolve(true),
        isAllowed: () => Promise.resolve(true),
        getUserInfo: () => Promise.resolve({ publicKey: 'GABC' }),
        signTransaction: (tx, opts) => Promise.resolve(''),
    };
}
test('create wallet', async (t) => {
    const wallet = getWallet('standalone');
    t.true(await wallet.isConnected());
});
