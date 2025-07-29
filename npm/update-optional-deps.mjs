#!/usr/bin/env node

import { execSync } from 'child_process';
import { readFileSync, writeFileSync } from 'fs';
import path from 'path';

const __dirname = new URL('.', import.meta.url).pathname;
process.chdir(path.join(__dirname, '..'));

function main() {
  // Check if we're on the changeset-release/main branch
  try {
    const currentBranch = execSync('git rev-parse --abbrev-ref HEAD', { encoding: 'utf8' }).trim();
    if (currentBranch !== 'changeset-release/main') {
      console.log(`Not on changeset-release/main branch (current: ${currentBranch}), skipping optionalDependencies update`);
      return;
    }
  } catch (error) {
    console.error('Error checking current branch:', error.message);
    return;
  }

  console.log('On changeset-release/main branch, updating optionalDependencies');

  try {
    // Get version from package.json instead of using changeset
    const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
    const newVersion = pkg.version;

    if (!newVersion) {
      console.log('Could not determine version from package.json');
      process.exit(1);
    }

    console.log('Found version:', newVersion);
    
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

    // Commit and push the changes
    console.log('Committing and pushing changes...');
    execSync('git add package.json', { stdio: 'inherit' });
    execSync(`git commit -m "Update optionalDependencies to ${newVersion}"`, { stdio: 'inherit' });
    execSync('git push origin HEAD:changeset-release/main', { stdio: 'inherit' });
    console.log('Successfully committed and pushed changes');

  } catch (error) {
    console.error('Error updating optionalDependencies:', error.message);
    process.exit(1);
  }
}

main();