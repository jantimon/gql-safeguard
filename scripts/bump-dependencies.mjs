#!/usr/bin/env node

import { execSync } from 'child_process';
import { readFileSync, writeFileSync, existsSync, readdirSync } from 'fs';
import { join } from 'path';

function hasChangesetFiles() {
  if (!existsSync('.changeset')) {
    return false;
  }
  
  const files = readdirSync('.changeset');
  const changesetFiles = files.filter(file => file.endsWith('.md') && file !== 'README.md');
  return changesetFiles.length > 0;
}

function main() {
  // Check if there are changeset files (indicates PR creation mode)
  if (!hasChangesetFiles()) {
    console.log('No changeset files found, skipping optionalDependencies update (likely publishing mode)');
    return;
  }

  console.log('Changeset files found, updating optionalDependencies for PR creation');

  try {
    // Get the new version that changesets will use
    const changesetStatus = JSON.parse(execSync('npx changeset status --output=json', { encoding: 'utf8' }));
    const newVersion = changesetStatus.releases?.[0]?.newVersion;

    if (!newVersion) {
      console.log('Could not determine new version from changesets');
      process.exit(1);
    }

    console.log('Found new version:', newVersion);

    // Update package.json optionalDependencies
    const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
    
    if (!pkg.optionalDependencies) {
      console.log('No optionalDependencies found in package.json');
      return;
    }

    Object.keys(pkg.optionalDependencies).forEach(dep => {
      pkg.optionalDependencies[dep] = newVersion;
    });

    writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
    console.log('Updated optionalDependencies to version', newVersion + ':');
    console.log(JSON.stringify(pkg.optionalDependencies, null, 2));

  } catch (error) {
    console.error('Error updating optionalDependencies:', error.message);
    process.exit(1);
  }
}

main();