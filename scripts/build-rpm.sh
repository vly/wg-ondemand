#!/bin/bash
# Script to build RPM package locally

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building wg-ondemand RPM package${NC}"

# Get version from spec file
VERSION=$(grep '^Version:' wg-ondemand.spec | awk '{print $2}')
echo -e "${YELLOW}Version: ${VERSION}${NC}"

# Check if running on Fedora/RHEL
if ! command -v rpmbuild &> /dev/null; then
    echo -e "${RED}Error: rpmbuild not found${NC}"
    echo "Install with: sudo dnf install @development-tools @rpm-development-tools"
    exit 1
fi

# Check dependencies
echo "Checking build dependencies..."
MISSING_DEPS=()

for dep in rust cargo clang llvm; do
    if ! command -v $dep &> /dev/null; then
        MISSING_DEPS+=($dep)
    fi
done

if [ ${#MISSING_DEPS[@]} -ne 0 ]; then
    echo -e "${RED}Missing dependencies: ${MISSING_DEPS[*]}${NC}"
    echo "Install with: sudo dnf install rust cargo clang llvm elfutils-libelf-devel kernel-devel libbpf-devel"
    exit 1
fi

# Set up RPM build tree
echo "Setting up RPM build tree..."
mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Create source tarball
echo "Creating source tarball..."
git archive --format=tar.gz --prefix=wg-ondemand-${VERSION}/ HEAD > ~/rpmbuild/SOURCES/wg-ondemand-${VERSION}.tar.gz

# Copy spec file
cp wg-ondemand.spec ~/rpmbuild/SPECS/

# Build source RPM
echo "Building source RPM..."
cd ~/rpmbuild/SPECS
rpmbuild -bs wg-ondemand.spec

# Build binary RPM
echo "Building binary RPM..."
rpmbuild -bb wg-ondemand.spec

# List built packages
echo -e "${GREEN}Build complete!${NC}"
echo ""
echo "Source RPM:"
ls -lh ~/rpmbuild/SRPMS/wg-ondemand-*.src.rpm
echo ""
echo "Binary RPM:"
ls -lh ~/rpmbuild/RPMS/x86_64/wg-ondemand-*.rpm
echo ""
echo -e "${YELLOW}To install:${NC}"
echo "  sudo dnf install ~/rpmbuild/RPMS/x86_64/wg-ondemand-${VERSION}-*.rpm"
echo ""
echo -e "${YELLOW}To test in container:${NC}"
echo "  podman run -it --rm -v ~/rpmbuild:/rpmbuild:ro fedora:latest bash"
echo "  dnf install /rpmbuild/RPMS/x86_64/wg-ondemand-${VERSION}-*.rpm"
