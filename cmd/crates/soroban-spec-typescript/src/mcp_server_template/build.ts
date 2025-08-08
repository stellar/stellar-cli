import * as esbuild from 'esbuild';
import { readFile } from 'fs/promises';
import { resolve } from 'path';

async function build() {
  try {
    // Read sac-sdk source
    const sacSdkPath = resolve('./node_modules/sac-sdk/src/index.ts');
    const sacSdkContent = await readFile(sacSdkPath, 'utf8');

    // First bundle sac-sdk
    await esbuild.build({
      stdin: {
        contents: sacSdkContent,
        loader: 'ts',
        resolveDir: resolve('./node_modules/sac-sdk/src'),
      },
      bundle: true,
      outfile: './build/sac-sdk.js',
      format: 'esm',
      platform: 'node',
      target: 'node18',
      external: ['@stellar/stellar-sdk'],
    });

    // Then bundle our app
    await esbuild.build({
      entryPoints: ['src/index.ts'],
      bundle: true,
      outfile: './build/index.js',
      format: 'esm',
      platform: 'node',
      target: 'node18',
      external: [
        '@stellar/stellar-sdk',
        '@modelcontextprotocol/sdk',
        'zod',
        'dotenv'
      ],
      plugins: [{
        name: 'resolve-sac-sdk',
        setup(build) {
          build.onResolve({ filter: /^sac-sdk$/ }, () => {
            return { path: resolve('./build/sac-sdk.js') };
          });
        },
      }],
    });

    console.log('Build completed successfully!');
  } catch (error) {
    console.error('Build failed:', error);
    process.exit(1);
  }
}

build(); 