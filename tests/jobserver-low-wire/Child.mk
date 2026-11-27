# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

child:
	@$(file >child-auth,$(filter --jobserver-auth=%,$(MAKEFLAGS)))/usr/bin/python3 witness.py inspect
