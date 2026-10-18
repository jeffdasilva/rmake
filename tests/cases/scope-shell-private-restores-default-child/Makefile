# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all: private SHELL = /bin/bash
all: child
all child:
	@if [ -n "$${BASH_VERSION-}" ]; then printf '%s=bash\n' '$@'; else printf '%s=sh\n' '$@'; fi
