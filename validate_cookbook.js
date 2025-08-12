#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m'
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function validateMdxFile(filePath) {
  try {
    const content = fs.readFileSync(filePath, 'utf8');
    const fileName = path.basename(filePath);
    let issues = [];
    let warnings = [];

    // Check if file has frontmatter
    if (!content.startsWith('---')) {
      issues.push('Missing frontmatter (should start with ---)');
    }

    // Check for required frontmatter fields
    const frontmatterMatch = content.match(/^---\s*\n([\s\S]*?)\n---/);
    if (frontmatterMatch) {
      const frontmatter = frontmatterMatch[1];
      
      // Check required fields
      if (!frontmatter.includes('title:')) {
        issues.push('Missing title field in frontmatter');
      }
      if (!frontmatter.includes('description:')) {
        issues.push('Missing description field in frontmatter');
      }
      if (!frontmatter.includes('hide_table_of_contents:')) {
        warnings.push('Missing hide_table_of_contents field in frontmatter');
      }
      if (!frontmatter.includes('custom_edit_url:')) {
        warnings.push('Missing custom_edit_url field in frontmatter');
      }
    } else {
      issues.push('Invalid frontmatter format');
    }

    // Check for basic content structure
    if (!content.includes('##')) {
      warnings.push('No section headers found (##)');
    }

    // Check for code blocks
    if (!content.includes('```')) {
      warnings.push('No code blocks found');
    }

    // Check for CLI commands
    if (!content.includes('stellar ')) {
      warnings.push('No Stellar CLI commands found');
    }

    // Check file size
    const sizeKB = (content.length / 1024).toFixed(1);
    if (parseFloat(sizeKB) < 1) {
      warnings.push(`File is very small (${sizeKB}KB) - might be incomplete`);
    }

    return {
      fileName,
      issues,
      warnings,
      sizeKB,
      isValid: issues.length === 0
    };
  } catch (error) {
    return {
      fileName: path.basename(filePath),
      issues: [`Error reading file: ${error.message}`],
      warnings: [],
      sizeKB: 0,
      isValid: false
    };
  }
}

function main() {
  log('🔍 Validating Stellar CLI Cookbook MDX Files...\n', 'blue');

  const cookbookDir = path.join(__dirname, 'cookbook');
  const files = fs.readdirSync(cookbookDir)
    .filter(file => file.endsWith('.mdx'))
    .map(file => path.join(cookbookDir, file));

  if (files.length === 0) {
    log('❌ No MDX files found in cookbook directory', 'red');
    process.exit(1);
  }

  log(`Found ${files.length} MDX files to validate:\n`, 'blue');

  let totalIssues = 0;
  let totalWarnings = 0;
  let validFiles = 0;

  files.forEach(filePath => {
    const result = validateMdxFile(filePath);
    
    if (result.isValid) {
      log(`✅ ${result.fileName} (${result.sizeKB}KB)`, 'green');
      validFiles++;
    } else {
      log(`❌ ${result.fileName} (${result.sizeKB}KB)`, 'red');
      result.issues.forEach(issue => {
        log(`   🚨 ${issue}`, 'red');
        totalIssues++;
      });
    }

    result.warnings.forEach(warning => {
      log(`   ⚠️  ${warning}`, 'yellow');
      totalWarnings++;
    });

    console.log('');
  });

  // Summary
  log('📊 Validation Summary:', 'blue');
  log(`   Files: ${files.length}`, 'reset');
  log(`   Valid: ${validFiles}`, 'green');
  log(`   Issues: ${totalIssues}`, totalIssues > 0 ? 'red' : 'green');
  log(`   Warnings: ${totalWarnings}`, totalWarnings > 0 ? 'yellow' : 'green');

  if (totalIssues > 0) {
    log('\n❌ Validation failed with issues', 'red');
    process.exit(1);
  } else if (totalWarnings > 0) {
    log('\n⚠️  Validation passed with warnings', 'yellow');
  } else {
    log('\n✅ All files validated successfully!', 'green');
  }
}

if (require.main === module) {
  main();
}

module.exports = { validateMdxFile };
