#!/usr/bin/env node

const { platform, arch } = require('os');
const { existsSync } = require('fs');
const { join } = require('path');

// Platform mapping
function getPlatformPackage() {
  const platformKey = `${platform()} ${arch()}`;
  
  const packageMap = {
    'darwin arm64': '@gql-safeguard/darwin-arm64',
    'darwin x64': '@gql-safeguard/darwin-x64', 
    'linux arm64': '@gql-safeguard/linux-arm64',
    'linux x64': '@gql-safeguard/linux-x64',
    'win32 x64': '@gql-safeguard/win32-x64'
  };
  
  return packageMap[platformKey];
}

function checkInstallation() {
  const pkg = getPlatformPackage();
  
  if (!pkg) {
    console.warn(`[gql-safeguard] Unsupported platform: ${platform()} ${arch()}`);
    console.warn('[gql-safeguard] The binary may not work on this platform');
    return;
  }
  
  const binaryName = platform() === 'win32' ? 'gql-safeguard.exe' : 'gql-safeguard';
  
  try {
    const binaryPath = require.resolve(`${pkg}/bin/${binaryName}`);
    if (existsSync(binaryPath)) {
      console.log(`[gql-safeguard] Successfully installed binary for ${platform()} ${arch()}`);
    }
  } catch (error) {
    console.warn(`[gql-safeguard] Platform-specific package ${pkg} not found`);
    console.warn('[gql-safeguard] This may happen if optional dependencies were skipped');
    console.warn('[gql-safeguard] Try reinstalling with: npm install --include=optional');
  }
}

// Only run check if this is the main module (not during require)
if (require.main === module) {
  checkInstallation();
}