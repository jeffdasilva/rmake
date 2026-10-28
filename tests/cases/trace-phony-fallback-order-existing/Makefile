# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
$(shell touch -t 200001010000.00 out)
$(file >missing,owned)
.PHONY: missing
out: | missing;@printf 'done\n'
missing:;
