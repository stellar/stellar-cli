#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m'
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function testAssetManagementRecipes() {
  log('🧪 Testing Asset Management Recipes...\n', 'blue');

  const assetManagementFiles = [
    'stellar-asset-operations.mdx',
    'trustlines-management.mdx', 
    'clawback-operations.mdx',
    'asset-authorization.mdx',
    'contract-metadata.mdx'
  ];

  let allTestsPassed = true;
  let testResults = [];

  assetManagementFiles.forEach(fileName => {
    log(`📋 Testing ${fileName}...`, 'cyan');
    
    const filePath = path.join(__dirname, '..', fileName);
    const content = fs.readFileSync(filePath, 'utf8');
    
    const tests = [
      {
        name: 'File exists and readable',
        test: () => content.length > 0,
        critical: true
      },
      {
        name: 'Has proper frontmatter',
        test: () => content.startsWith('---') && content.includes('---', 3),
        critical: true
      },
      {
        name: 'Has title field',
        test: () => content.includes('title:'),
        critical: true
      },
      {
        name: 'Has description field',
        test: () => content.includes('description:'),
        critical: true
      },
      {
        name: 'Has hide_table_of_contents field',
        test: () => content.includes('hide_table_of_contents:'),
        critical: false
      },
      {
        name: 'Has custom_edit_url field',
        test: () => content.includes('custom_edit_url:'),
        critical: false
      },
      {
        name: 'Contains section headers',
        test: () => content.includes('##'),
        critical: false
      },
      {
        name: 'Contains code blocks',
        test: () => content.includes('```'),
        critical: false
      },
      {
        name: 'Contains Stellar CLI commands',
        test: () => content.includes('stellar '),
        critical: false
      },
      {
        name: 'Contains practical examples',
        test: () => content.includes('```bash') || content.includes('```rust'),
        critical: false
      },
      {
        name: 'Has reasonable file size',
        test: () => content.length > 2000, // At least 2KB
        critical: false
      }
    ];

    const fileResults = {
      fileName,
      tests: [],
      passed: 0,
      failed: 0,
      warnings: 0
    };

    tests.forEach(test => {
      try {
        const passed = test.test();
        if (passed) {
          fileResults.passed++;
          fileResults.tests.push({ name: test.name, status: 'PASS', critical: test.critical });
        } else {
          fileResults.failed++;
          if (test.critical) {
            allTestsPassed = false;
          }
          fileResults.tests.push({ name: test.name, status: test.critical ? 'FAIL' : 'WARN', critical: test.critical });
        }
      } catch (error) {
        fileResults.failed++;
        if (test.critical) {
          allTestsPassed = false;
        }
        fileResults.tests.push({ name: test.name, status: test.critical ? 'FAIL' : 'WARN', critical: test.critical, error: error.message });
      }
    });

    // Display test results for this file
    fileResults.tests.forEach(test => {
      const status = test.status === 'PASS' ? '✅' : test.status === 'FAIL' ? '❌' : '⚠️';
      const color = test.status === 'PASS' ? 'green' : test.status === 'FAIL' ? 'red' : 'yellow';
      log(`   ${status} ${test.name}`, color);
    });

    testResults.push(fileResults);
    console.log('');
  });

  // Summary
  log('📊 Asset Management Recipes Test Summary:', 'blue');
  testResults.forEach(result => {
    const status = result.failed === 0 ? '✅' : result.failed > 0 && result.tests.some(t => t.status === 'FAIL') ? '❌' : '⚠️';
    const color = result.failed === 0 ? 'green' : result.failed > 0 && result.tests.some(t => t.status === 'FAIL') ? 'red' : 'yellow';
    log(`${status} ${result.fileName}: ${result.passed} passed, ${result.failed} failed`, color);
  });

  console.log('');
  
  const totalTests = testResults.reduce((sum, r) => sum + r.tests.length, 0);
  const totalPassed = testResults.reduce((sum, r) => sum + r.passed, 0);
  const totalFailed = testResults.reduce((sum, r) => sum + r.failed, 0);

  log(`Overall Results: ${totalPassed}/${totalTests} tests passed`, totalFailed === 0 ? 'green' : 'red');

  if (allTestsPassed) {
    log('\n🎉 All Asset Management recipes passed validation!', 'green');
  } else {
    log('\n❌ Some Asset Management recipes have critical issues', 'red');
  }

  return allTestsPassed;
}

function testCLICommands() {
  log('\n🔧 Testing CLI Commands in Recipes...', 'blue');
  
  const commands = [
    'stellar keys generate',
    'stellar keys fund',
    'stellar keys address',
    'stellar tx new change_trust',
    'stellar tx new payment',
    'stellar contract invoke',
    'stellar contract asset deploy',
    'stellar contract id asset',
    'stellar tx new set_options'
  ];

  let foundCommands = 0;
  
  commands.forEach(cmd => {
    // Check if this command appears in any of our recipes
    const assetManagementFiles = [
      'stellar-asset-operations.mdx',
      'trustlines-management.mdx', 
      'clawback-operations.mdx',
      'asset-authorization.mdx',
      'contract-metadata.mdx'
    ];

    let found = false;
    assetManagementFiles.forEach(fileName => {
      const filePath = path.join(__dirname, '..', fileName);
      const content = fs.readFileSync(filePath, 'utf8');
      if (content.includes(cmd)) {
        found = true;
      }
    });

    if (found) {
      log(`   ✅ ${cmd}`, 'green');
      foundCommands++;
    } else {
      log(`   ❌ ${cmd}`, 'red');
    }
  });

  log(`\n📊 CLI Commands Coverage: ${foundCommands}/${commands.length} commands documented`, foundCommands === commands.length ? 'green' : 'yellow');
  
  return foundCommands === commands.length;
}

function main() {
  log('🚀 Stellar CLI Asset Management Recipes Integration Test\n', 'blue');
  
  const recipesPassed = testAssetManagementRecipes();
  const cliCommandsPassed = testCLICommands();
  
  console.log('\n' + '='.repeat(60));
  
  if (recipesPassed && cliCommandsPassed) {
    log('\n🎉 All tests passed! Asset Management recipes are ready for use.', 'green');
    process.exit(0);
  } else {
    log('\n❌ Some tests failed. Please review the issues above.', 'red');
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = { testAssetManagementRecipes, testCLICommands };
