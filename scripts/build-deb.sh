#!/usr/bin/env sh
set -e

chmod 755 packaging/DEBIAN/postinst packaging/DEBIAN/postrm

cargo deb

DEB_FILE=$(find target/debian -maxdepth 1 -name '*.deb' | sort | tail -n1)
lintian --profile debian "$DEB_FILE" || true

echo "Built ${DEB_FILE}"
